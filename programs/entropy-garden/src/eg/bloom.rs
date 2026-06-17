use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use crate::eg::EgConfig;
use crate::state::{WeatherFeed, WEATHER_RING};
use crate::error::GardenError;

// ───────────────────────────────────────────────────────────────────────────
// THE BLOOM RACE — multi-player EG redistribution game.
// Three phases per round: Commit (back a bloom) → Grow (chain decides) → Resolve.
// Winner = bloom whose on-chain signal grew the most. No new EG minted.
// Pure redistribution: stake EG, win EG from the shared pool.
// ───────────────────────────────────────────────────────────────────────────

pub const BLOOM_COUNT: usize = 2;            // rainbloom(0)=TPS, emberpetal(1)=fees
pub const COMMIT_SLOTS: u64  = 1200;          // ~2 min commit window
pub const GROW_SLOTS:   u64  = 600;          // ~4 min grow window
pub const TREASURY_CUT_BPS: u64 = 1_000;    // 10% to treasury EG on resolve
pub const MIN_STAKE:    u64  = 10_000_000_000; // 10 EG (9 decimals)
pub const MAX_STAKE:    u64  = 1_000_000_000_000; // 1000 EG
pub const MIN_POOL_TO_RESOLVE: u64 = 1_000_000_000; // 1 EG min pool (anti-dust)

// region IDs: bloom 0 = TPS (region 1), bloom 1 = fees (region 0)
pub const BLOOM_REGIONS: [u16; BLOOM_COUNT] = [1, 0];

// Phase stored as u8: 0=Commit, 1=Growing, 2=Resolved
pub const PHASE_COMMIT:   u8 = 0;
pub const PHASE_GROWING:  u8 = 1;
pub const PHASE_RESOLVED: u8 = 2;

#[account]
#[derive(InitSpace)]
pub struct BloomRound {
    pub round_id:       u64,
    pub phase:          u8,
    pub commit_end_slot:u64,
    pub grow_end_slot:  u64,
    pub baseline_bps:   [u16; BLOOM_COUNT],   // snapshot at round open
    pub total_staked:   [u64; BLOOM_COUNT],   // EG staked per bloom
    pub total_pool:     u64,                  // total EG in vault
    pub winner:         u8,                   // bloom_id of winner (set at resolve)
    pub resolved:       bool,
    pub vault_bump:     u8,
    pub bump:           u8,
    pub _reserved:      [u8; 32],
}

#[account]
#[derive(InitSpace)]
pub struct BloomStake {
    pub player:         Pubkey,
    pub round_id:       u64,
    pub bloom_id:       u8,
    pub amount:         u64,
    pub claimed:        bool,
    pub bump:           u8,
}

// ── helper: latest bps from a feed ────────────────────────────────────────
fn feed_bps(f: &WeatherFeed) -> u16 {
    if f.len == 0 { return 0; }
    let idx = (f.head as usize + WEATHER_RING - 1) % WEATHER_RING;
    f.samples_bps[idx]
}

// ── open_round ─────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct OpenRound<'info> {
    #[account(mut)]
    pub opener: Signer<'info>,
    #[account(
        init, payer = opener, space = 8 + BloomRound::INIT_SPACE,
        seeds = [b"bloom_round", round_id.to_le_bytes().as_ref()], bump
    )]
    pub round: Account<'info, BloomRound>,
    /// CHECK: vault PDA (will hold staked EG as a token account)
    #[account(seeds = [b"bloom_vault", round_id.to_le_bytes().as_ref()], bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(
        init_if_needed, payer = opener,
        associated_token::mint = eg_mint,
        associated_token::authority = vault_authority,
    )]
    pub vault: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"weather", BLOOM_REGIONS[0].to_le_bytes().as_ref()], bump = feed0.bump)]
    pub feed0: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"weather", BLOOM_REGIONS[1].to_le_bytes().as_ref()], bump = feed1.bump)]
    pub feed1: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    #[account(seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn open_round(ctx: Context<OpenRound>, round_id: u64) -> Result<()> {
    require!(!ctx.accounts.eg_config.paused, GardenError::Paused);
    let now = Clock::get()?.slot;
    let r = &mut ctx.accounts.round;
    r.round_id       = round_id;
    r.phase          = PHASE_COMMIT;
    r.commit_end_slot= now + COMMIT_SLOTS;
    r.grow_end_slot  = now + COMMIT_SLOTS + GROW_SLOTS;
    r.baseline_bps   = [feed_bps(&ctx.accounts.feed0), feed_bps(&ctx.accounts.feed1)];
    r.total_staked   = [0u64; BLOOM_COUNT];
    r.total_pool     = 0;
    r.winner         = 0;
    r.resolved       = false;
    r.vault_bump     = ctx.bumps.vault_authority;
    r.bump           = ctx.bumps.round;
    r._reserved      = [0u8; 32];
    msg!("Bloom Race #{} open: commit until slot {}", round_id, r.commit_end_slot);
    Ok(())
}

// ── stake_bloom ────────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(round_id: u64, bloom_id: u8, amount: u64)]
pub struct StakeBloom<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(
        mut,
        seeds = [b"bloom_round", round_id.to_le_bytes().as_ref()], bump = round.bump,
    )]
    pub round: Account<'info, BloomRound>,
    #[account(
        init, payer = player, space = 8 + BloomStake::INIT_SPACE,
        seeds = [b"bloom_stake", round_id.to_le_bytes().as_ref(), player.key().as_ref()],
        bump
    )]
    pub stake: Account<'info, BloomStake>,
    /// CHECK: vault authority PDA
    #[account(seeds = [b"bloom_vault", round_id.to_le_bytes().as_ref()], bump = round.vault_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, associated_token::mint = eg_mint, associated_token::authority = vault_authority)]
    pub vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, associated_token::mint = eg_mint, associated_token::authority = player)]
    pub player_eg: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    #[account(seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn stake_bloom(ctx: Context<StakeBloom>, round_id: u64, bloom_id: u8, amount: u64) -> Result<()> {
    require!(!ctx.accounts.eg_config.paused, GardenError::Paused);
    let now = Clock::get()?.slot;
    let r = &mut ctx.accounts.round;
    require!(r.phase == PHASE_COMMIT, GardenError::BadForecastWindow);
    require!(now < r.commit_end_slot, GardenError::BadForecastWindow);
    require!((bloom_id as usize) < BLOOM_COUNT, GardenError::BadForecastWindow);
    require!(amount >= MIN_STAKE && amount <= MAX_STAKE, GardenError::BadForecastWindow);

    // transfer EG from player → vault
    anchor_spl::token::transfer(
        CpiContext::new(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.player_eg.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.player.to_account_info(),
            },
        ),
        amount,
    )?;

    r.total_staked[bloom_id as usize] = r.total_staked[bloom_id as usize].saturating_add(amount);
    r.total_pool = r.total_pool.saturating_add(amount);
    // advance phase if commit window closed
    if now >= r.commit_end_slot { r.phase = PHASE_GROWING; }

    let s = &mut ctx.accounts.stake;
    s.player   = ctx.accounts.player.key();
    s.round_id = round_id;
    s.bloom_id = bloom_id;
    s.amount   = amount;
    s.claimed  = false;
    s.bump     = ctx.bumps.stake;
    msg!("bloom {} stake: {} EG by {}", bloom_id, amount / 1_000_000_000, ctx.accounts.player.key());
    Ok(())
}

// ── resolve_round ──────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct ResolveRound<'info> {
    #[account(mut)]
    pub resolver: Signer<'info>,
    #[account(
        mut,
        seeds = [b"bloom_round", round_id.to_le_bytes().as_ref()], bump = round.bump,
    )]
    pub round: Account<'info, BloomRound>,
    /// CHECK: vault authority PDA
    #[account(seeds = [b"bloom_vault", round_id.to_le_bytes().as_ref()], bump = round.vault_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, associated_token::mint = eg_mint, associated_token::authority = vault_authority)]
    pub vault: Box<Account<'info, TokenAccount>>,
    // treasury EG account (receives 10% cut)
    #[account(mut)]
    pub treasury_eg: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"weather", BLOOM_REGIONS[0].to_le_bytes().as_ref()], bump = feed0.bump)]
    pub feed0: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"weather", BLOOM_REGIONS[1].to_le_bytes().as_ref()], bump = feed1.bump)]
    pub feed1: Box<Account<'info, WeatherFeed>>,
    #[account(seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    #[account(seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn resolve_round(ctx: Context<ResolveRound>, round_id: u64) -> Result<()> {
    let now = Clock::get()?.slot;
    let r = &mut ctx.accounts.round;
    require!(!r.resolved, GardenError::MazeAlreadyRevealed);
    require!(now >= r.grow_end_slot, GardenError::ForecastNotReady);
    require!(r.total_pool >= MIN_POOL_TO_RESOLVE, GardenError::PoolExhausted);

    // read current bps values
    let current = [feed_bps(&ctx.accounts.feed0), feed_bps(&ctx.accounts.feed1)];
    // winner = bloom whose signal grew the most, among blooms that HAVE backers
    // (a bloom with 0 stakers cannot win — the pool would be unclaimable)
    let mut winner: u8 = u8::MAX;
    let mut best_delta: u32 = 0;
    for i in 0..BLOOM_COUNT {
        if r.total_staked[i] == 0 { continue; } // skip unbacked blooms
        let base = r.baseline_bps[i] as u32;
        let cur  = current[i] as u32;
        let delta = cur.saturating_sub(base);
        if winner == u8::MAX || delta > best_delta {
            best_delta = delta;
            winner = i as u8;
        }
    }
    // require at least one backed bloom
    require!(winner != u8::MAX, GardenError::PoolExhausted);
    r.winner   = winner;
    r.resolved = true;
    r.phase    = PHASE_RESOLVED;

    // take 10% treasury cut from vault
    let cut = r.total_pool * TREASURY_CUT_BPS / 10_000;
    if cut > 0 {
        let rid = round_id.to_le_bytes();
        let vault_bump = r.vault_bump;
        let seeds: &[&[u8]] = &[b"bloom_vault", &rid, &[vault_bump]];
        let signer = &[seeds];
        anchor_spl::token::transfer(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                Transfer {
                    from: ctx.accounts.vault.to_account_info(),
                    to: ctx.accounts.treasury_eg.to_account_info(),
                    authority: ctx.accounts.vault_authority.to_account_info(),
                },
                signer,
            ),
            cut,
        )?;
    }
    msg!("Bloom Race #{} resolved: winner bloom {} (delta {}), pool {} EG",
         round_id, winner, best_delta, r.total_pool / 1_000_000_000);
    Ok(())
}

// ── claim_winnings ─────────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct ClaimWinnings<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(seeds = [b"bloom_round", round_id.to_le_bytes().as_ref()], bump = round.bump)]
    pub round: Account<'info, BloomRound>,
    #[account(
        mut, close = player,
        seeds = [b"bloom_stake", round_id.to_le_bytes().as_ref(), player.key().as_ref()],
        bump = stake.bump, has_one = player @ GardenError::Unauthorized,
    )]
    pub stake: Account<'info, BloomStake>,
    /// CHECK: vault authority PDA
    #[account(seeds = [b"bloom_vault", round_id.to_le_bytes().as_ref()], bump = round.vault_bump)]
    pub vault_authority: UncheckedAccount<'info>,
    #[account(mut, associated_token::mint = eg_mint, associated_token::authority = vault_authority)]
    pub vault: Box<Account<'info, TokenAccount>>,
    #[account(mut, associated_token::mint = eg_mint, associated_token::authority = player)]
    pub player_eg: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    #[account(seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_winnings(ctx: Context<ClaimWinnings>, round_id: u64) -> Result<()> {
    let r = &ctx.accounts.round;
    require!(r.resolved, GardenError::ForecastNotReady);
    require!(!ctx.accounts.stake.claimed, GardenError::MazeAlreadyRevealed);
    require!(ctx.accounts.stake.bloom_id == r.winner, GardenError::Unauthorized);

    let winner_pool = r.total_staked[r.winner as usize];
    require!(winner_pool > 0, GardenError::PoolExhausted);

    // share = stake × (pool × 90%) / winner_pool
    let net_pool = r.total_pool - (r.total_pool * TREASURY_CUT_BPS / 10_000);
    let share = (ctx.accounts.stake.amount as u128)
        .saturating_mul(net_pool as u128)
        .checked_div(winner_pool as u128)
        .unwrap_or(0) as u64;
    require!(share > 0, GardenError::PoolExhausted);

    let rid = round_id.to_le_bytes();
    let vault_bump = r.vault_bump;
    let seeds: &[&[u8]] = &[b"bloom_vault", &rid, &[vault_bump]];
    let signer = &[seeds];
    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.vault.to_account_info(),
                to: ctx.accounts.player_eg.to_account_info(),
                authority: ctx.accounts.vault_authority.to_account_info(),
            },
            signer,
        ),
        share,
    )?;

    ctx.accounts.stake.claimed = true;
    msg!("bloom winner claim: {} EG to {}", share / 1_000_000_000, ctx.accounts.player.key());
    Ok(())
}

// ── close_losing_stake ─────────────────────────────────────────────────────
#[derive(Accounts)]
#[instruction(round_id: u64)]
pub struct CloseLosingStake<'info> {
    #[account(mut)]
    pub player: Signer<'info>,
    #[account(seeds = [b"bloom_round", round_id.to_le_bytes().as_ref()], bump = round.bump)]
    pub round: Account<'info, BloomRound>,
    #[account(
        mut, close = player,
        seeds = [b"bloom_stake", round_id.to_le_bytes().as_ref(), player.key().as_ref()],
        bump = stake.bump, has_one = player @ GardenError::Unauthorized,
    )]
    pub stake: Account<'info, BloomStake>,
}

pub fn close_losing_stake(ctx: Context<CloseLosingStake>, _round_id: u64) -> Result<()> {
    let r = &ctx.accounts.round;
    require!(r.resolved, GardenError::ForecastNotReady);
    require!(ctx.accounts.stake.bloom_id != r.winner, GardenError::Unauthorized);
    msg!("losing stake closed — rent returned to {}", ctx.accounts.player.key());
    Ok(())
}
