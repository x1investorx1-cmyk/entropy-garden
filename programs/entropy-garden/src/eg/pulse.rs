use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::eg::*;
use crate::state::{WeatherFeed, Region, WEATHER_RING};
use crate::error::GardenError;

// ───────────────────────────────────────────────────────────────────────────
// THE PULSE — read the chain's tempo (TPS), predict its BAND.
// Resolves against region 1 (the Rainline), whose weather bps IS a normalized
// TPS reading (tps/CEILING × 10000). Four bands instead of binary storm/calm.
// Same cheat-proof future-resolution as Sky-Reading: predict the band the
// tempo will be in N slots from now; it can't be precomputed.
// ───────────────────────────────────────────────────────────────────────────

// Band thresholds in bps (region-1 feed units). Calibrate vs real X1 TPS.
//   DORMANT  band 0: bps < T1
//   STEADY   band 1: T1 <= bps < T2
//   BUSY     band 2: T2 <= bps < T3
//   SURGING  band 3: bps >= T3
pub const PULSE_T1: u16 = 2000;
pub const PULSE_T2: u16 = 4500;
pub const PULSE_T3: u16 = 7000;

pub const PULSE_REGION: u16 = 1;          // the Rainline carries the TPS reading
pub const MIN_PULSE_SLOTS: u64 = 40;
pub const MAX_PULSE_SLOTS: u64 = 2000;
pub const REWARD_PULSE: u64 = 3;          // same base as sky-read; 4-band difficulty rewards skill

/// classify a bps reading into a band 0..3
pub fn band_of(bps: u16) -> u8 {
    if bps < PULSE_T1 { 0 }
    else if bps < PULSE_T2 { 1 }
    else if bps < PULSE_T3 { 2 }
    else { 3 }
}

#[account]
#[derive(InitSpace)]
pub struct Pulse {
    pub reader: Pubkey,
    pub predicted_band: u8,    // 0=dormant,1=steady,2=busy,3=surging
    pub commit_slot: u64,
    pub resolve_slot: u64,
    pub baseline_bps: u16,
    pub resolved: bool,
    pub bump: u8,
}

// ── commit_pulse ────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(predicted_band: u8, window_slots: u64, commit_slot: u64)]
pub struct CommitPulse<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    #[account(seeds = [b"region", PULSE_REGION.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
    #[account(seeds = [b"weather", PULSE_REGION.to_le_bytes().as_ref()], bump = feed.bump)]
    pub feed: Box<Account<'info, WeatherFeed>>,
    #[account(
        init, payer = reader, space = 8 + Pulse::INIT_SPACE,
        seeds = [b"pulse", reader.key().as_ref(), commit_slot.to_le_bytes().as_ref()],
        bump
    )]
    pub pulse: Account<'info, Pulse>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    /// CHECK: treasury XNT sink
    #[account(mut, seeds = [b"eg_treasury"], bump, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn commit_pulse(ctx: Context<CommitPulse>, predicted_band: u8, window_slots: u64, commit_slot: u64) -> Result<()> {
    require!(predicted_band < 4, GardenError::BadForecastWindow);
    require!(window_slots >= MIN_PULSE_SLOTS && window_slots <= MAX_PULSE_SLOTS,
             GardenError::BadForecastWindow);
    let now = Clock::get()?.slot;
    require!(commit_slot <= now && now.saturating_sub(commit_slot) <= 30, GardenError::BadForecastWindow);

    let f = &ctx.accounts.feed;
    let baseline = if f.len > 0 {
        let idx = (f.head as usize + WEATHER_RING - 1) % WEATHER_RING;
        f.samples_bps[idx]
    } else { 0 };

    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.reader.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }), fee)?;
    }

    let p = &mut ctx.accounts.pulse;
    p.reader = ctx.accounts.reader.key();
    p.predicted_band = predicted_band;
    p.commit_slot = commit_slot;
    p.resolve_slot = now + window_slots;
    p.baseline_bps = baseline;
    p.resolved = false;
    p.bump = ctx.bumps.pulse;
    msg!("Pulse committed: band {} within {} slots", predicted_band, window_slots);
    Ok(())
}

// ── resolve_pulse ────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct ResolvePulse<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    #[account(
        mut, close = reader,
        seeds = [b"pulse", reader.key().as_ref(), pulse.commit_slot.to_le_bytes().as_ref()],
        bump = pulse.bump,
        has_one = reader @ GardenError::Unauthorized
    )]
    pub pulse: Account<'info, Pulse>,
    #[account(seeds = [b"weather", PULSE_REGION.to_le_bytes().as_ref()], bump = feed.bump)]
    pub feed: Box<Account<'info, WeatherFeed>>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(
        init_if_needed, payer = reader,
        associated_token::mint = eg_mint,
        associated_token::authority = reader,
    )]
    pub reader_eg: Box<Account<'info, TokenAccount>>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn resolve_pulse(ctx: Context<ResolvePulse>) -> Result<()> {
    let now = Clock::get()?.slot;
    let p = &ctx.accounts.pulse;
    require!(now >= p.resolve_slot, GardenError::ForecastNotReady);

    // read the latest tempo sample; must be at/after commit (future-resolution)
    let f = &ctx.accounts.feed;
    let mut latest_bps: Option<u16> = None;
    let mut latest_slot: u64 = 0;
    for i in 0..f.len as usize {
        if f.sample_slots[i] >= latest_slot {
            latest_slot = f.sample_slots[i];
            latest_bps = Some(f.samples_bps[i]);
        }
    }
    require!(latest_slot >= p.commit_slot, GardenError::ForecastNotReady);
    let bps = latest_bps.ok_or(GardenError::ForecastNotReady)?;

    let actual_band = band_of(bps);
    let correct = actual_band == p.predicted_band;

    if correct && !ctx.accounts.eg_config.paused {
        let amount = ctx.accounts.eg_config.reward_amount(REWARD_PULSE, now);
        if amount > 0 {
            let bump = ctx.accounts.eg_config.mint_authority_bump;
            let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
            let signer = &[seeds];
            anchor_spl::token::mint_to(
                CpiContext::new_with_signer(
                    ctx.accounts.token_program.to_account_info(),
                    anchor_spl::token::MintTo {
                        mint: ctx.accounts.eg_mint.to_account_info(),
                        to: ctx.accounts.reader_eg.to_account_info(),
                        authority: ctx.accounts.eg_mint_auth.to_account_info(),
                    }, signer),
                amount)?;
            ctx.accounts.eg_config.total_minted =
                ctx.accounts.eg_config.total_minted.saturating_add(amount);
            msg!("Pulse CORRECT: band {} ({} bps), +{} EG", actual_band, bps, amount);
        }
    } else {
        msg!("Pulse resolved: actual band {} ({} bps), predicted {}", actual_band, bps, p.predicted_band);
    }
    Ok(())
}
