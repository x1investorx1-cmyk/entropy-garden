use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::eg::*;
use crate::state::{WeatherFeed, Region, WEATHER_RING};
use crate::error::GardenError;

// Weather thresholds (bps). Storm = busy/high pressure, Calm = quiet.
pub const STORM_THRESHOLD: u16 = 4500;
pub const CALM_THRESHOLD: u16 = 2500;
// Forecast window bounds (slots). ~40 slots ≈ 16s minimum; cap to keep it snappy.
pub const MIN_FORECAST_SLOTS: u64 = 40;
pub const MAX_FORECAST_SLOTS: u64 = 2000;
// Reward for a correct read (whole EG, before era factor).
pub const REWARD_SKYREAD: u64 = 3;

/// A single committed forecast. PDA seeded by reader + region + commit slot,
/// so each forecast is unique and can't be replayed.
#[account]
#[derive(InitSpace)]
pub struct Forecast {
    pub reader: Pubkey,
    pub region_id: u16,
    pub predict_storm: bool,   // true = predicts storm, false = predicts calm
    pub commit_slot: u64,      // slot the forecast was made
    pub resolve_slot: u64,     // earliest slot it can be resolved
    pub baseline_bps: u16,     // weather at commit time (for reference)
    pub resolved: bool,
    pub bump: u8,
}

// ── commit_forecast ────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(region_id: u16, predict_storm: bool, window_slots: u64, commit_slot: u64)]
pub struct CommitForecast<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    #[account(seeds = [b"region", region_id.to_le_bytes().as_ref()], bump = region.bump)]
    pub region: Account<'info, Region>,
    #[account(
        seeds = [b"weather", region_id.to_le_bytes().as_ref()], bump = feed.bump
    )]
    pub feed: Box<Account<'info, WeatherFeed>>,
    #[account(
        init, payer = reader, space = 8 + Forecast::INIT_SPACE,
        seeds = [b"forecast", reader.key().as_ref(), region_id.to_le_bytes().as_ref(),
                 commit_slot.to_le_bytes().as_ref()],
        bump
    )]
    pub forecast: Account<'info, Forecast>,
    // fee → treasury (50/50 handled same as other actions; here full fee to treasury)
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    /// CHECK: treasury XNT sink
    #[account(mut, seeds = [b"eg_treasury"], bump, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn commit_forecast(ctx: Context<CommitForecast>, region_id: u16, predict_storm: bool, window_slots: u64, commit_slot: u64) -> Result<()> {
    require!(window_slots >= MIN_FORECAST_SLOTS && window_slots <= MAX_FORECAST_SLOTS,
             GardenError::BadForecastWindow);
    let now = Clock::get()?.slot;
    require!(commit_slot <= now && now.saturating_sub(commit_slot) <= 30, GardenError::BadForecastWindow);
    // baseline = most recent sample
    let f = &ctx.accounts.feed;
    let baseline = if f.len > 0 {
        let idx = (f.head as usize + WEATHER_RING - 1) % WEATHER_RING;
        f.samples_bps[idx]
    } else { 0 };

    // fee → treasury
    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.reader.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }), fee)?;
    }

    let fc = &mut ctx.accounts.forecast;
    fc.reader = ctx.accounts.reader.key();
    fc.region_id = region_id;
    fc.predict_storm = predict_storm;
    fc.commit_slot = commit_slot;
    fc.resolve_slot = commit_slot + window_slots;
    fc.baseline_bps = baseline;
    fc.resolved = false;
    fc.bump = ctx.bumps.forecast;
    msg!("Forecast: region {} predicts {} by slot {}", region_id,
         if predict_storm {"storm"} else {"calm"}, fc.resolve_slot);
    Ok(())
}

// ── resolve_forecast ───────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct ResolveForecast<'info> {
    #[account(mut)]
    pub reader: Signer<'info>,
    #[account(
        mut,
        seeds = [b"forecast", reader.key().as_ref(), forecast.region_id.to_le_bytes().as_ref(),
                 forecast.commit_slot.to_le_bytes().as_ref()],
        bump = forecast.bump,
        has_one = reader @ GardenError::Unauthorized,
        close = reader   // refund the rent when resolved
    )]
    pub forecast: Account<'info, Forecast>,
    #[account(seeds = [b"weather", forecast.region_id.to_le_bytes().as_ref()], bump = feed.bump)]
    pub feed: Box<Account<'info, WeatherFeed>>,
    // EG reward path
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

pub fn resolve_forecast(ctx: Context<ResolveForecast>) -> Result<()> {
    let now = Clock::get()?.slot;
    let fc = &ctx.accounts.forecast;
    require!(now >= fc.resolve_slot, GardenError::ForecastNotReady);

    // read weather sampled at/after resolve_slot — must have a fresh sample
    let f = &ctx.accounts.feed;
    let mut latest_bps: Option<u16> = None;
    let mut latest_slot: u64 = 0;
    for i in 0..f.len as usize {
        if f.sample_slots[i] >= latest_slot {
            latest_slot = f.sample_slots[i];
            latest_bps = Some(f.samples_bps[i]);
        }
    }
    require!(latest_slot >= fc.commit_slot, GardenError::ForecastNotReady);
    let bps = latest_bps.ok_or(GardenError::ForecastNotReady)?;

    // Determine outcome: did it reach the predicted state?
    let is_storm = bps >= STORM_THRESHOLD;
    let is_calm = bps <= CALM_THRESHOLD;
    let correct = (fc.predict_storm && is_storm) || (!fc.predict_storm && is_calm);

    if correct && !ctx.accounts.eg_config.paused {
        let amount = ctx.accounts.eg_config.reward_amount(REWARD_SKYREAD, now);
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
            msg!("Sky-read CORRECT: {} bps, +{} EG", bps, amount);
        }
    } else {
        msg!("Sky-read resolved: {} bps, prediction {}", bps, if correct {"correct"} else {"missed"});
    }
    Ok(())
}
