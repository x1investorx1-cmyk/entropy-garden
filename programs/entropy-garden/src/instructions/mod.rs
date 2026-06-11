use anchor_lang::prelude::*;
use anchor_lang::solana_program::sysvar::slot_hashes;

use crate::error::GardenError;
use crate::math::decay as m;
use crate::state::*;

const SHARE_SCALE: u128 = 1_000_000_000_000; // 1e12

// ---------------------------------------------------------------------------
// initialize_garden
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitializeGarden<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init, payer = authority, space = 8 + GardenConfig::INIT_SPACE,
        seeds = [b"config"], bump
    )]
    pub config: Account<'info, GardenConfig>,
    #[account(
        init, payer = authority, space = 8 + NutrientPool::INIT_SPACE,
        seeds = [b"compost"], bump
    )]
    pub pool: Box<Account<'info, NutrientPool>>,
    pub system_program: Program<'info, System>,
}

pub fn initialize_garden(ctx: Context<InitializeGarden>, genesis_nutrients: u64) -> Result<()> {
    let c = &mut ctx.accounts.config;
    c.authority = ctx.accounts.authority.key();
    c.regions = 0;
    c.base_decay_rate = 8;
    c.base_growth_rate = 5;
    c.compost_pool_bps = 7_000;
    c.compost_bounty_bps = 100;
    c.crank_reward_lamports = 5_000;
    c.tend_cooldown_slots = 750; // ~5 min
    c.genesis_soil_per_plot = genesis_nutrients / 1_000_000; // 0.0001%
    c.max_plots = 500; // beta cap; raise via set_params
    c.total_plots = 0;
    c.paused = false;
    c.bump = ctx.bumps.config;

    let p = &mut ctx.accounts.pool;
    p.balance = genesis_nutrients;
    p.genesis_total = genesis_nutrients;
    p.total_compost_shares = 0;
    p.acc_reward_per_share = 0;
    p.bump = ctx.bumps.pool;
    Ok(())
}

// ---------------------------------------------------------------------------
// create_region
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(region_id: u16)]
pub struct CreateRegion<'info> {
    #[account(mut, address = config.authority @ GardenError::Unauthorized)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(
        init, payer = authority, space = 8 + Region::INIT_SPACE,
        seeds = [b"region", region_id.to_le_bytes().as_ref()], bump
    )]
    pub region: Account<'info, Region>,
    #[account(
        init, payer = authority, space = 8 + WeatherFeed::INIT_SPACE,
        seeds = [b"weather", region_id.to_le_bytes().as_ref()], bump
    )]
    pub feed: Box<Account<'info, WeatherFeed>>,
    pub system_program: Program<'info, System>,
}

pub fn create_region(ctx: Context<CreateRegion>, region_id: u16, channel: WeatherChannel) -> Result<()> {
    let r = &mut ctx.accounts.region;
    r.region_id = region_id;
    r.channel = channel;
    r.current_weather_bps = 5_000; // neutral genesis weather
    r.last_weather_slot = 0; // genesis: allow immediate first sample
    r.plot_count = 0;
    r.open = true;
    r.bump = ctx.bumps.region;

    let f = &mut ctx.accounts.feed;
    f.region_id = region_id;
    f.bump = ctx.bumps.feed;
    ctx.accounts.config.regions += 1;
    Ok(())
}

// ---------------------------------------------------------------------------
// update_weather (permissionless crank)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct UpdateWeather<'info> {
    #[account(mut)]
    pub crank: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, seeds = [b"region", region.region_id.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
    #[account(mut, seeds = [b"weather", region.region_id.to_le_bytes().as_ref()], bump = feed.bump)]
    pub feed: Box<Account<'info, WeatherFeed>>,
    /// CHECK: validated by address constraint against the SlotHashes sysvar id.
    #[account(address = slot_hashes::id())]
    pub slot_hashes: UncheckedAccount<'info>,
}

pub fn update_weather(ctx: Context<UpdateWeather>, proposed_bps: u16, sampled_slot: u64) -> Result<()> {
    require!(!ctx.accounts.config.paused, GardenError::Paused);
    require!(proposed_bps <= 10_000, GardenError::BadSample);
    let now = Clock::get()?.slot;
    // Sample must be fresh and not from the future.
    require!(sampled_slot <= now && now - sampled_slot <= 150, GardenError::StaleSample);
    let region = &mut ctx.accounts.region;
    // Rate-limit: one accepted sample per region per ~25 slots.
    require!(region.last_weather_slot == 0 || now - region.last_weather_slot >= 25, GardenError::TooFrequent);

    let accepted = m::clamp_weather(region.current_weather_bps, proposed_bps);
    region.current_weather_bps = accepted;
    region.last_weather_slot = now;
    ctx.accounts.feed.push(accepted, now);

    // Crank reward paid from config account lamports (funded by fees).
    // v0.1: reward vault = config PDA lamport balance above rent.
    let reward = ctx.accounts.config.crank_reward_lamports;
    let config_info = ctx.accounts.config.to_account_info();
    let rent_min = Rent::get()?.minimum_balance(config_info.data_len());
    if config_info.lamports() >= rent_min + reward {
        **config_info.try_borrow_mut_lamports()? -= reward;
        **ctx.accounts.crank.to_account_info().try_borrow_mut_lamports()? += reward;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// claim_plot
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct ClaimPlot<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, seeds = [b"compost"], bump = pool.bump)]
    pub pool: Box<Account<'info, NutrientPool>>,
    #[account(mut, seeds = [b"region", region.region_id.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
    #[account(
        init, payer = owner, space = 8 + Plot::INIT_SPACE,
        seeds = [b"plot", owner.key().as_ref(), config.total_plots.to_le_bytes().as_ref()],
        bump
    )]
    pub plot: Box<Account<'info, Plot>>,
    pub system_program: Program<'info, System>,
}

pub fn claim_plot(ctx: Context<ClaimPlot>) -> Result<()> {
    let c = &mut ctx.accounts.config;
    require!(!c.paused, GardenError::Paused);
    require!(ctx.accounts.region.open, GardenError::RegionClosed);
    require!(c.total_plots < c.max_plots, GardenError::PlotCapReached);

    let pool = &mut ctx.accounts.pool;
    let soil = c.genesis_soil_per_plot;
    require!(pool.balance >= soil, GardenError::PoolExhausted);
    pool.balance -= soil; // conservation: pool -> plot soil

    let p = &mut ctx.accounts.plot;
    p.owner = ctx.accounts.owner.key();
    p.region = ctx.accounts.region.key();
    p.plot_index = c.total_plots;
    p.soil_nutrients = soil;
    p.plants = [None; PLANT_SLOTS];
    p.compost_shares = 0;
    p.reward_debt = 0;
    p.bump = ctx.bumps.plot;

    c.total_plots += 1;
    ctx.accounts.region.plot_count += 1;
    Ok(())
}

// ---------------------------------------------------------------------------
// plant_seed
// ---------------------------------------------------------------------------

#[derive(Accounts)]
#[instruction(slot_index: u8)]
pub struct PlantSeed<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, has_one = owner @ GardenError::Unauthorized)]
    pub plot: Box<Account<'info, Plot>>,
    #[account(
        init, payer = owner, space = 8 + Plant::INIT_SPACE,
        seeds = [b"plant", plot.key().as_ref(), slot_index.to_le_bytes().as_ref()],
        bump
    )]
    pub plant: Box<Account<'info, Plant>>,
    /// CHECK: validated by address constraint against the SlotHashes sysvar id.
    #[account(address = slot_hashes::id())]
    pub slot_hashes: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn plant_seed(ctx: Context<PlantSeed>, slot_index: u8, species: u16) -> Result<()> {
    require!(!ctx.accounts.config.paused, GardenError::Paused);
    require!((slot_index as usize) < PLANT_SLOTS, GardenError::BadSlotIndex);
    let plot = &mut ctx.accounts.plot;
    require!(plot.plants[slot_index as usize].is_none(), GardenError::SlotOccupied);

    // Seed cost: locks soil into the new plant as starting biomass.
    let seed_cost: u64 = 10;
    require!(plot.soil_nutrients >= seed_cost, GardenError::NotEnoughSoil);
    plot.soil_nutrients -= seed_cost;

    let now = Clock::get()?.slot;
    // Genome: hash(recent slot-hash bytes ++ planter ++ slot). NOTE: leader-
    // influenceable entropy — fine for v0.1 traits; commit-reveal for rares.
    let sh = ctx.accounts.slot_hashes.try_borrow_data()?;
    let take = sh.len().min(40);
    let mut material = Vec::with_capacity(take + 32 + 8 + 1);
    material.extend_from_slice(&sh[..take]);
    material.extend_from_slice(ctx.accounts.owner.key().as_ref());
    material.extend_from_slice(&now.to_le_bytes());
    material.push(slot_index);
    let genome = anchor_lang::solana_program::hash::hash(&material).to_bytes();

    let plant = &mut ctx.accounts.plant;
    plant.plot = plot.key();
    plant.slot_index = slot_index;
    plant.species = species;
    plant.genome = genome;
    // Resilience profile derived from genome: optimal weather in [1000, 9000].
    plant.optimal_bps = 1_000 + (u16::from_le_bytes([genome[0], genome[1]]) % 8_001);
    plant.planted_slot = now;
    plant.last_evaluated_slot = now;
    plant.health = HEALTH_MAX;
    plant.biomass = seed_cost; // conservation: soil -> biomass
    plant.growth_stage = 0;
    plant.bump = ctx.bumps.plant;

    plot.plants[slot_index as usize] = Some(plant.key());
    Ok(())
}

// ---------------------------------------------------------------------------
// tend — the core interaction; runs lazy evaluation then applies care bonus
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Tend<'info> {
    pub owner: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, has_one = owner @ GardenError::Unauthorized)]
    pub plot: Box<Account<'info, Plot>>,
    #[account(mut, constraint = plant.plot == plot.key() @ GardenError::WrongPlot)]
    pub plant: Box<Account<'info, Plant>>,
    #[account(
        seeds = [b"weather", region.region_id.to_le_bytes().as_ref()], bump = feed.bump,
        constraint = plot.region == region.key() @ GardenError::WrongRegion
    )]
    pub feed: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"region", region.region_id.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
}

pub fn tend(ctx: Context<Tend>) -> Result<()> {
    require!(!ctx.accounts.config.paused, GardenError::Paused);
    let now = Clock::get()?.slot;
    let c = &ctx.accounts.config;
    let plant = &mut ctx.accounts.plant;
    require!(!plant.is_dead(), GardenError::PlantDead);
    let elapsed = now.saturating_sub(plant.last_evaluated_slot);
    require!(elapsed >= c.tend_cooldown_slots, GardenError::TendCooldown);

    // Lazy evaluation against the median of the elapsed weather window.
    let window = ctx.accounts.feed.window(plant.last_evaluated_slot);
    let weather = m::median(&window);
    let plot = &mut ctx.accounts.plot;
    let (h, b, s) = m::evaluate(
        plant.health, plant.biomass, plot.soil_nutrients,
        c.base_decay_rate, c.base_growth_rate,
        elapsed, weather, plant.optimal_bps,
    );
    plant.health = h;
    plant.biomass = b;
    plot.soil_nutrients = s;
    plant.last_evaluated_slot = now;

    if !plant.is_dead() {
        // Care bonus: restore 15% of max health, capped.
        plant.health = (plant.health + 150).min(HEALTH_MAX);
        // Stage up roughly per doubling of biomass over seed cost.
        plant.growth_stage = match plant.biomass {
            0..=19 => 0, 20..=39 => 1, 40..=79 => 2,
            80..=159 => 3, 160..=319 => 4, _ => 5,
        };
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// compost — anyone may compost a dead plant; conservation-critical
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Compost<'info> {
    #[account(mut)]
    pub composter: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, seeds = [b"compost"], bump = pool.bump)]
    pub pool: Box<Account<'info, NutrientPool>>,
    #[account(mut)]
    pub plot: Box<Account<'info, Plot>>,
    /// Composter's own plot, receiving the bounty as soil. May equal `plot`.
    #[account(mut, has_one = owner @ GardenError::Unauthorized, constraint = bounty_plot.owner == composter.key() @ GardenError::Unauthorized)]
    pub bounty_plot: Box<Account<'info, Plot>>,
    /// CHECK: owner of bounty_plot, checked by has_one.
    pub owner: UncheckedAccount<'info>,
    #[account(
        mut,
        constraint = plant.plot == plot.key() @ GardenError::WrongPlot,
        close = composter
    )]
    pub plant: Box<Account<'info, Plant>>,
    #[account(
        seeds = [b"weather", region.region_id.to_le_bytes().as_ref()], bump = feed.bump,
        constraint = plot.region == region.key() @ GardenError::WrongRegion
    )]
    pub feed: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"region", region.region_id.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
}

pub fn compost(ctx: Context<Compost>) -> Result<()> {
    let now = Clock::get()?.slot;
    let c = &ctx.accounts.config;
    let plant = &mut ctx.accounts.plant;

    // Evaluate first — the plant must actually be dead *now*.
    let window = ctx.accounts.feed.window(plant.last_evaluated_slot);
    let weather = m::median(&window);
    let plot = &mut ctx.accounts.plot;
    let elapsed = now.saturating_sub(plant.last_evaluated_slot);
    let (h, b, s) = m::evaluate(
        plant.health, plant.biomass, plot.soil_nutrients,
        c.base_decay_rate, c.base_growth_rate,
        elapsed, weather, plant.optimal_bps,
    );
    plot.soil_nutrients = s;
    require!(h == 0, GardenError::PlantAlive);

    let (to_pool, to_soil, bounty) = m::compost_split(b, c.compost_pool_bps, c.compost_bounty_bps);
    let pool = &mut ctx.accounts.pool;
    pool.balance += to_pool;
    plot.soil_nutrients += to_soil;
    ctx.accounts.bounty_plot.soil_nutrients += bounty;

    // Share accounting: dead plot's owner earns pool shares == to_pool.
    pool.acc_reward_per_share = pool.acc_reward_per_share; // rewards accrue on draw
    plot.compost_shares += to_pool as u128;
    pool.total_compost_shares += to_pool as u128;

    // Clear the plot slot; plant account closes to composter (rent bounty).
    plot.plants[plant.slot_index as usize] = None;
    Ok(())
}

// ---------------------------------------------------------------------------
// draw_nutrients — pull pool nutrients pro-rata to lifetime compost shares
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct DrawNutrients<'info> {
    pub owner: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, seeds = [b"compost"], bump = pool.bump)]
    pub pool: Box<Account<'info, NutrientPool>>,
    #[account(mut, has_one = owner @ GardenError::Unauthorized)]
    pub plot: Box<Account<'info, Plot>>,
}

pub fn draw_nutrients(ctx: Context<DrawNutrients>) -> Result<()> {
    require!(!ctx.accounts.config.paused, GardenError::Paused);
    let pool = &mut ctx.accounts.pool;
    let plot = &mut ctx.accounts.plot;
    require!(pool.total_compost_shares > 0 && plot.compost_shares > 0, GardenError::NoShares);

    // v0.1 simple model: claimable = pool.balance * your_shares/total - debt,
    // capped at 10% of pool per draw to prevent drain races.
    let entitled = (pool.balance as u128) * plot.compost_shares / pool.total_compost_shares;
    let claim = entitled.saturating_sub(plot.reward_debt).min((pool.balance / 10) as u128) as u64;
    require!(claim > 0, GardenError::NothingToDraw);

    pool.balance -= claim;
    plot.soil_nutrients += claim; // conservation: pool -> soil
    plot.reward_debt += claim as u128;
    let _ = SHARE_SCALE; // reserved for v0.2 masterchef-style accumulator
    Ok(())
}

// ---------------------------------------------------------------------------
// admin: set_params / set_paused
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct AdminOnly<'info> {
    #[account(address = config.authority @ GardenError::Unauthorized)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
}

pub fn set_paused(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
    ctx.accounts.config.paused = paused;
    Ok(())
}

pub fn set_max_plots(ctx: Context<AdminOnly>, max_plots: u32) -> Result<()> {
    ctx.accounts.config.max_plots = max_plots;
    Ok(())
}
