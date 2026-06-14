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
    // ---- EG mining ----
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Box<Account<'info, crate::eg::EgConfig>>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, anchor_spl::token::Mint>>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(
        init_if_needed, payer = owner,
        associated_token::mint = eg_mint,
        associated_token::authority = owner,
    )]
    pub owner_eg: Box<Account<'info, anchor_spl::token::TokenAccount>>,
    /// CHECK: treasury XNT sink
    #[account(mut, seeds = [b"eg_treasury"], bump, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
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

    // ---- EG mining: fee -> treasury, mint REWARD_PLANT ----
    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.owner.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }),
            fee,
        )?;
    }
    if !ctx.accounts.eg_config.paused {
        let amount = ctx.accounts.eg_config.reward_amount(crate::eg::REWARD_PLANT, now);
        if amount > 0 {
            let bump = ctx.accounts.eg_config.mint_authority_bump;
            let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
            let signer = &[seeds];
            anchor_spl::token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    anchor_spl::token::MintTo {
                        mint: ctx.accounts.eg_mint.to_account_info(),
                        to: ctx.accounts.owner_eg.to_account_info(),
                        authority: ctx.accounts.eg_mint_auth.to_account_info(),
                    },
                    signer,
                ),
                amount,
            )?;
            ctx.accounts.eg_config.total_minted =
                ctx.accounts.eg_config.total_minted.saturating_add(amount);
            msg!("EG plant reward: {}", amount);
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// tend — the core interaction; runs lazy evaluation then applies care bonus
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Tend<'info> {
    #[account(mut)]
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

    // ---- EG mining accounts (step 1) ----
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Box<Account<'info, crate::eg::EgConfig>>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, anchor_spl::token::Mint>>,
    /// CHECK: mint authority PDA, signs via seeds
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(
        init_if_needed,
        payer = owner,
        associated_token::mint = eg_mint,
        associated_token::authority = owner,
    )]
    pub owner_eg: Box<Account<'info, anchor_spl::token::TokenAccount>>,
    /// CHECK: Treasury PDA receiving the fee; validated by seeds + address
    #[account(mut, seeds = [b"eg_treasury"], bump, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn tend(ctx: Context<Tend>) -> Result<()> {
    require!(!ctx.accounts.config.paused, GardenError::Paused);
    let now = Clock::get()?.slot;
    let cooldown = ctx.accounts.config.tend_cooldown_slots; // copy value, not a borrow
    let base_decay = ctx.accounts.config.base_decay_rate;
    let base_growth = ctx.accounts.config.base_growth_rate;

    // Compute streak in plain locals while we hold the plant borrow, so the borrow
    // ends before the EG mint block re-borrows ctx.accounts mutably.
    let (streak, snum, sden) = {
        let plant = &mut ctx.accounts.plant;
        require!(!plant.is_dead(), GardenError::PlantDead);
        let elapsed = now.saturating_sub(plant.last_evaluated_slot);
        require!(elapsed >= cooldown, GardenError::TendCooldown);

        // Lazy evaluation against the median of the elapsed weather window.
        let window = ctx.accounts.feed.window(plant.last_evaluated_slot);
        let weather = m::median(&window);
        let plot = &mut ctx.accounts.plot;
        let (h, b, s) = m::evaluate(
            plant.health, plant.biomass, plot.soil_nutrients,
            base_decay, base_growth,
            elapsed, weather, plant.optimal_bps,
        );
        plant.health = h;
        plant.biomass = b;
        plot.soil_nutrients = s;
        plant.last_evaluated_slot = now;
        let stress_now = m::stress(weather, plant.optimal_bps);
        plant.cumulative_stress = plant.cumulative_stress
            .saturating_add((stress_now as u64).saturating_mul(elapsed) / 1000);

        if !plant.is_dead() {
            plant.health = (plant.health + 150).min(HEALTH_MAX);
            plant.growth_stage = match plant.biomass {
                0..=19 => 0, 20..=39 => 1, 40..=79 => 2,
                80..=159 => 3, 160..=319 => 4, _ => 5,
            };
        }

        // Streak tracking in plant._reserved[0..8] (count) + [8..16] (last-tend slot).
        let last_streak_slot = u64::from_le_bytes(plant._reserved[8..16].try_into().unwrap());
        let on_time = last_streak_slot == 0 || now.saturating_sub(last_streak_slot) <= cooldown * 4;
        let mut streak = u64::from_le_bytes(plant._reserved[0..8].try_into().unwrap());
        streak = if on_time { streak.saturating_add(1) } else { 1 };
        plant._reserved[0..8].copy_from_slice(&streak.to_le_bytes());
        plant._reserved[8..16].copy_from_slice(&now.to_le_bytes());

        let (snum, sden): (u64, u64) = match streak {
            0..=2 => (1, 1),
            3..=5 => (5, 4),
            6..=10 => (3, 2),
            _ => (2, 1),
        };
        (streak, snum, sden)
    }; // plant & plot borrows end here

    // Collect the per-action fee in XNT (lamports) → treasury PDA.
    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(
                ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.owner.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                },
            ),
            fee,
        )?;
    }

    // Mint EG: base REWARD_TEND, scaled by streak, then era/genesis factor inside mint_reward.
    if !ctx.accounts.eg_config.paused {
        let base = crate::eg::REWARD_TEND.saturating_mul(snum) / sden.max(1);
        let amount = ctx.accounts.eg_config.reward_amount(base, now);
        if amount > 0 {
            let bump = ctx.accounts.eg_config.mint_authority_bump;
            let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
            let signer = &[seeds];
            anchor_spl::token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    anchor_spl::token::MintTo {
                        mint: ctx.accounts.eg_mint.to_account_info(),
                        to: ctx.accounts.owner_eg.to_account_info(),
                        authority: ctx.accounts.eg_mint_auth.to_account_info(),
                    },
                    signer,
                ),
                amount,
            )?;
            let cfg = &mut ctx.accounts.eg_config;
            cfg.total_minted = cfg.total_minted.saturating_add(amount);
            msg!("EG mined: {} (streak {})", amount, streak);
        }
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
    // ---- EG mining ----
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Box<Account<'info, crate::eg::EgConfig>>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, anchor_spl::token::Mint>>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(
        init_if_needed, payer = composter,
        associated_token::mint = eg_mint,
        associated_token::authority = composter,
    )]
    pub composter_eg: Box<Account<'info, anchor_spl::token::TokenAccount>>,
    /// CHECK: treasury XNT sink
    #[account(mut, seeds = [b"eg_treasury"], bump, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
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

    let (to_pool, to_soil, _bounty) = m::compost_split(b, c.compost_pool_bps, 0);
    let pool = &mut ctx.accounts.pool;
    pool.balance += to_pool;
    plot.soil_nutrients += to_soil;

    // Share accounting: dead plot's owner earns pool shares == to_pool.
    pool.acc_reward_per_share = pool.acc_reward_per_share; // rewards accrue on draw
    plot.compost_shares += to_pool as u128;
    pool.total_compost_shares += to_pool as u128;

    // Clear the plot slot; plant account closes to composter (rent bounty).
    plot.plants[plant.slot_index as usize] = None;

    // ---- EG mining: civic reward for composting ----
    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.composter.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }),
            fee,
        )?;
    }
    if !ctx.accounts.eg_config.paused {
        let amount = ctx.accounts.eg_config.reward_amount(crate::eg::REWARD_COMPOST, now);
        if amount > 0 {
            let bump = ctx.accounts.eg_config.mint_authority_bump;
            let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
            let signer = &[seeds];
            anchor_spl::token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    anchor_spl::token::MintTo {
                        mint: ctx.accounts.eg_mint.to_account_info(),
                        to: ctx.accounts.composter_eg.to_account_info(),
                        authority: ctx.accounts.eg_mint_auth.to_account_info(),
                    },
                    signer,
                ),
                amount,
            )?;
            ctx.accounts.eg_config.total_minted =
                ctx.accounts.eg_config.total_minted.saturating_add(amount);
            msg!("EG compost reward: {}", amount);
        }
    }
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

// ---------------------------------------------------------------------------
// harvest — claim EG from a flowering (stage 5) plant; plant is composted
// 20 EG base reward; biomass returns to soil (conservation holds)
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct Harvest<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(seeds = [b"config"], bump = config.bump)]
    pub config: Account<'info, GardenConfig>,
    #[account(mut, seeds = [b"compost"], bump = pool.bump)]
    pub pool: Box<Account<'info, NutrientPool>>,
    #[account(mut, has_one = owner @ GardenError::Unauthorized)]
    pub plot: Box<Account<'info, Plot>>,
    #[account(
        mut,
        constraint = plant.plot == plot.key() @ GardenError::WrongPlot,
        close = owner
    )]
    pub plant: Box<Account<'info, Plant>>,
    #[account(
        seeds = [b"weather", region.region_id.to_le_bytes().as_ref()], bump = feed.bump,
        constraint = plot.region == region.key() @ GardenError::WrongRegion
    )]
    pub feed: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"region", region.region_id.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
    // ---- EG mining ----
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Box<Account<'info, crate::eg::EgConfig>>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, anchor_spl::token::Mint>>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(
        init_if_needed, payer = owner,
        associated_token::mint = eg_mint,
        associated_token::authority = owner,
    )]
    pub owner_eg: Box<Account<'info, anchor_spl::token::TokenAccount>>,
    /// CHECK: treasury XNT sink
    #[account(mut, seeds = [b"eg_treasury"], bump, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
    pub associated_token_program: Program<'info, anchor_spl::associated_token::AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn harvest(ctx: Context<Harvest>) -> Result<()> {
    require!(!ctx.accounts.config.paused, GardenError::Paused);
    let now = Clock::get()?.slot;
    let c = &ctx.accounts.config;
    let plant = &mut ctx.accounts.plant;

    // Evaluate current state
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

    // Must be alive AND at stage 5 (flowering)
    require!(h > 0, GardenError::PlantDead);
    require!(plant.growth_stage >= 5, GardenError::NotFlowering);

    // Return biomass to soil (conservation: plant -> soil)
    plot.soil_nutrients += b;
    let compost_share = b * c.compost_pool_bps as u64 / 10_000;
    plot.soil_nutrients = plot.soil_nutrients.saturating_sub(compost_share);
    ctx.accounts.pool.balance += compost_share;

    // Clear the plant slot
    plot.plants[plant.slot_index as usize] = None;

    // ---- EG harvest reward (the big one) ----
    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.owner.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }),
            fee,
        )?;
    }
    if !ctx.accounts.eg_config.paused {
        let amount = ctx.accounts.eg_config.reward_amount(crate::eg::REWARD_HARVEST, now);
        if amount > 0 {
            let bump = ctx.accounts.eg_config.mint_authority_bump;
            let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
            let signer = &[seeds];
            anchor_spl::token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    anchor_spl::token::MintTo {
                        mint: ctx.accounts.eg_mint.to_account_info(),
                        to: ctx.accounts.owner_eg.to_account_info(),
                        authority: ctx.accounts.eg_mint_auth.to_account_info(),
                    },
                    signer,
                ),
                amount,
            )?;
            ctx.accounts.eg_config.total_minted =
                ctx.accounts.eg_config.total_minted.saturating_add(amount);
            msg!("EG harvest reward: {} | cumulative stress: {}", amount, plant.cumulative_stress);
        }
    }
    Ok(())
}
