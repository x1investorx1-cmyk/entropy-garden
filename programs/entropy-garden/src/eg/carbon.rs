use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::eg::*;
use crate::state::{Plant, WeatherFeed, WEATHER_RING};
use crate::error::GardenError;

// ───────────────────────────────────────────────────────────────────────────
// CARBON FARMING — plants sequester chain activity as biomass over time.
//   roots  ← fee pressure  (region 0 bps)  — the underground economy
//   leaves ← throughput     (region 1 bps)  — the visible canopy
// ACCUMULATION, not prediction: works on a quiet chain (only needs nonzero
// flow, not variation). On a calm chain leaves grow from steady TPS while
// roots stay shallow until X1 develops real fee pressure — the plant becomes
// a living record of the chain maturing. Harvest the sequestered biomass as EG.
// ───────────────────────────────────────────────────────────────────────────

pub const FEE_REGION: u16 = 0;   // roots feed
pub const TPS_REGION: u16 = 1;   // leaves feed

// Growth rate divisor. mass gain = bps * slots_elapsed / RATE_DIV.
// Calibrated so a plant sequestering steady ambient flow reaches a
// harvestable mass at ~gardening rate. Tune against real accumulation.
pub const RATE_DIV: u64 = 1_000;

// Cap slots_elapsed per sequester so a long-dormant sink can't mint a
// huge jump in one poke (and to bound integer growth). ~1 day of slots.
pub const MAX_ELAPSED: u64 = 216_000;

// Harvest reward base (whole EG, before era factor) per unit of balanced
// sequestered mass. Kept low; the diversity bonus rewards a mature chain.
pub const REWARD_CARBON_BASE: u64 = 1;

// Harvest payout divisor: brings accumulated mass down to ~gardening rate.
// ~1 day leaf-only accumulation (≈1.03M mass) pays ~10 EG; balanced pays 2x.
pub const HARVEST_DIV: u64 = 100_000;

// Minimum total mass before a sink can be harvested (avoid dust harvests).
pub const MIN_HARVEST_MASS: u64 = 100_000;

#[account]
#[derive(InitSpace)]
pub struct CarbonSink {
    pub plant: Pubkey,       // the plant this sink belongs to
    pub owner: Pubkey,       // who may harvest (the plant's planter)
    pub root_mass: u64,      // sequestered from fee pressure
    pub leaf_mass: u64,      // sequestered from throughput
    pub last_slot: u64,      // last sequester slot (accumulation clock)
    pub total_harvested: u64,// lifetime EG harvested from this sink
    pub bump: u8,
    pub _reserved: [u8; 32],
}

impl CarbonSink {
    /// total sequestered biomass
    pub fn total_mass(&self) -> u64 {
        self.root_mass.saturating_add(self.leaf_mass)
    }
    /// diversity balance 0..100: 100 = perfectly balanced root/leaf,
    /// 0 = all one type. Rewards a mature chain with BOTH fee + tps activity.
    pub fn balance_pct(&self) -> u64 {
        let r = self.root_mass;
        let l = self.leaf_mass;
        let tot = r.saturating_add(l);
        if tot == 0 { return 0; }
        let min = r.min(l);
        // 2*min/tot * 100 → 100 when r==l, 0 when one is zero
        (2u128 * min as u128 * 100 / tot as u128) as u64
    }
}

// helper: latest sample bps from a feed (most recent push)
fn latest_bps(f: &WeatherFeed) -> u16 {
    if f.len == 0 { return 0; }
    let idx = (f.head as usize + WEATHER_RING - 1) % WEATHER_RING;
    f.samples_bps[idx]
}

// ── init_carbon_sink ─────────────────────────────────────────────────────────
// Create a CarbonSink for a plant you own. Opt-in; live plants untouched.
#[derive(Accounts)]
pub struct InitCarbonSink<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(seeds = [b"plant", plant.plot.as_ref(), plant.slot_index.to_le_bytes().as_ref()],
              bump = plant.bump)]
    pub plant: Account<'info, Plant>,
    #[account(
        init, payer = owner, space = 8 + CarbonSink::INIT_SPACE,
        seeds = [b"carbon", plant.key().as_ref()], bump
    )]
    pub sink: Account<'info, CarbonSink>,
    pub system_program: Program<'info, System>,
}

pub fn init_carbon_sink(ctx: Context<InitCarbonSink>) -> Result<()> {
    let now = Clock::get()?.slot;
    let s = &mut ctx.accounts.sink;
    s.plant = ctx.accounts.plant.key();
    s.owner = ctx.accounts.owner.key();
    s.root_mass = 0;
    s.leaf_mass = 0;
    s.last_slot = now;
    s.total_harvested = 0;
    s.bump = ctx.bumps.sink;
    s._reserved = [0u8; 32];
    msg!("CarbonSink opened for plant {}", s.plant);
    Ok(())
}

// ── sequester ────────────────────────────────────────────────────────────────
// Permissionless poke: read the live feeds, add growth scaled by time elapsed.
// Anyone can call this for any sink (like the weather crank). No reward to the
// poker — it just advances accumulation. The owner harvests later.
#[derive(Accounts)]
pub struct Sequester<'info> {
    #[account(mut, seeds = [b"carbon", sink.plant.as_ref()], bump = sink.bump)]
    pub sink: Account<'info, CarbonSink>,
    #[account(seeds = [b"weather", FEE_REGION.to_le_bytes().as_ref()], bump = fee_feed.bump)]
    pub fee_feed: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"weather", TPS_REGION.to_le_bytes().as_ref()], bump = tps_feed.bump)]
    pub tps_feed: Box<Account<'info, WeatherFeed>>,
}

pub fn sequester(ctx: Context<Sequester>) -> Result<()> {
    let now = Clock::get()?.slot;
    let s = &mut ctx.accounts.sink;
    let elapsed = now.saturating_sub(s.last_slot).min(MAX_ELAPSED);
    if elapsed == 0 { return Ok(()); }

    let fee_bps = latest_bps(&ctx.accounts.fee_feed) as u64;
    let tps_bps = latest_bps(&ctx.accounts.tps_feed) as u64;

    // growth = signal * time / RATE_DIV. Real signal × real elapsed slots.
    let root_growth = fee_bps.saturating_mul(elapsed) / RATE_DIV;
    let leaf_growth = tps_bps.saturating_mul(elapsed) / RATE_DIV;

    s.root_mass = s.root_mass.saturating_add(root_growth);
    s.leaf_mass = s.leaf_mass.saturating_add(leaf_growth);
    s.last_slot = now;
    msg!("sequester: +{} root, +{} leaf (fee {} bps, tps {} bps, {} slots)",
         root_growth, leaf_growth, fee_bps, tps_bps, elapsed);
    Ok(())
}

// ── harvest_carbon ────────────────────────────────────────────────────────────
// Owner harvests sequestered biomass as EG. Reward scales with total mass and
// the diversity balance (balanced root+leaf pays more → rewards a mature chain).
// Resets masses to 0 (the carbon is "released" as EG). Tuned ~gardening rate.
#[derive(Accounts)]
pub struct HarvestCarbon<'info> {
    #[account(mut)]
    pub owner: Signer<'info>,
    #[account(mut, seeds = [b"carbon", sink.plant.as_ref()], bump = sink.bump,
              has_one = owner @ GardenError::Unauthorized)]
    pub sink: Account<'info, CarbonSink>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(
        init_if_needed, payer = owner,
        associated_token::mint = eg_mint,
        associated_token::authority = owner,
    )]
    pub owner_eg: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn harvest_carbon(ctx: Context<HarvestCarbon>) -> Result<()> {
    let now = Clock::get()?.slot;
    let total_mass = ctx.accounts.sink.total_mass();
    require!(total_mass >= MIN_HARVEST_MASS, GardenError::NotFlowering);

    if ctx.accounts.eg_config.paused {
        return Ok(());
    }

    // base reward from total mass, scaled by diversity balance.
    // reward = base_per_unit * total_mass * (100 + balance_pct) / 100 ... but
    // keep it bounded: use mass/normalizer so it tracks ~gardening rate.
    let balance = ctx.accounts.sink.balance_pct(); // 0..100
    // diversity multiplier: 1.0x (all one type) up to 2.0x (balanced)
    let diversity_num = 100 + balance; // 100..200
    // raw EG (whole units) before era factor
    let base = REWARD_CARBON_BASE
        .saturating_mul(total_mass)
        .saturating_mul(diversity_num) / (100u64.saturating_mul(HARVEST_DIV));
    let amount = ctx.accounts.eg_config.reward_amount(base, now);

    // guard: never reset the sink for a zero reward (don't trap a player's mass)
    require!(amount > 0, GardenError::NotFlowering);

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
            }, signer),
        amount)?;
    ctx.accounts.eg_config.total_minted =
        ctx.accounts.eg_config.total_minted.saturating_add(amount);

    let s = &mut ctx.accounts.sink;
    s.total_harvested = s.total_harvested.saturating_add(amount);
    s.root_mass = 0;
    s.leaf_mass = 0;
    s.last_slot = now;
    msg!("carbon harvest: mass {} balance {}% → +{} EG", total_mass, balance, amount);
    Ok(())
}
