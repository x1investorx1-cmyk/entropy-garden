use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use anchor_spl::associated_token::AssociatedToken;
use crate::eg::*;
use crate::error::GardenError;

// ───────────────────────────────────────────────────────────────────────────
// ARIADNE'S THREAD — a labyrinth quest.
// Trace the one true path to the heart (the rose). Each correct step winds the
// amplifier higher and accrues PENDING EG. Reach a checkpoint to LOCK IN
// (mint) your pending EG. One wrong turn snaps you all the way back to the
// gate, forfeits pending EG since the last checkpoint, resets the amplifier,
// and costs a small penalty fee.
//
// Anti-cheat: the maze is generated from hash(pubkey, future_blockhash), so it
// can't be precomputed before you commit. Economics (fees + penalties + reward
// tuned to gardening rate) make botting unprofitable rather than relying on a
// hidden solution.
// ───────────────────────────────────────────────────────────────────────────

pub const MAZE_W: u8 = 16;            // maze grid width
pub const MAZE_H: u8 = 16;            // maze grid height
pub const PATH_LEN: u8 = 48;          // length of the true path to the heart
pub const CHECKPOINT_EVERY: u8 = 6;   // lock in pending EG every N steps
pub const REWARD_STEP_BASE: u64 = 1;  // base EG per correct step (×era×genesis×amplifier)
pub const HEART_BONUS: u64 = 25;      // big EG payout for reaching the rose
pub const AMP_STEP_BP: u64 = 800;     // amplifier climbs +8% (800 bp) per correct step
pub const AMP_MAX_BP: u64 = 50000;    // amplifier caps at ×5.0
pub const REVEAL_DELAY: u64 = 3;      // slots to wait before maze reveal (future blockhash)
pub const REVEAL_MAX: u64 = 150;      // entry_slot+this must still be resolvable

// Directions
pub const DIR_N: u8 = 0;
pub const DIR_E: u8 = 1;
pub const DIR_S: u8 = 2;
pub const DIR_W: u8 = 3;

#[account]
#[derive(InitSpace)]
pub struct Thread {
    pub walker: Pubkey,
    pub entry_slot: u64,        // slot the player committed (entropy base)
    pub seed: [u8; 32],         // maze seed = hash(walker, blockhash@entry_slot+REVEAL_DELAY)
    pub revealed: bool,         // has the maze been derived yet
    pub pos: u8,                // current step index along the path (0 = gate)
    pub last_checkpoint: u8,    // step index of last locked-in checkpoint
    pub amplifier_bp: u64,      // current amplifier in basis points (10000 = ×1.0)
    pub pending_eg: u64,        // EG accrued since last checkpoint (whole-EG units, pre-era)
    pub reached_heart: bool,    // prestige flag
    pub active: bool,           // is this thread in progress
    pub bump: u8,
    pub _reserved: [u8; 32],
}

// ── maze generation ────────────────────────────────────────────────────────
// Deterministic from seed. We don't store the whole maze; we derive the TRUE
// PATH as a sequence of directions. The path is a self-avoiding walk on the
// grid driven by the seed, producing PATH_LEN steps from gate to heart.
// The program can regenerate it any time to validate a step. The seed is public
// on-chain (anti-cheat is economic, not secrecy — see module header).

fn keccak_step(state: &mut [u8; 32]) -> u8 {
    // cheap PRNG: hash the state, return a byte, advance state
    let h = anchor_lang::solana_program::keccak::hash(state);
    state.copy_from_slice(&h.to_bytes());
    state[0]
}

/// Returns the true path as a Vec of directions (length PATH_LEN).
/// Self-avoiding: tracks visited cells, picks a seed-driven unvisited neighbor;
/// if stuck, it backs off deterministically. Guaranteed to yield PATH_LEN dirs.
pub fn true_path(seed: &[u8; 32]) -> Vec<u8> {
    let mut state = *seed;
    let mut path = Vec::with_capacity(PATH_LEN as usize);
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    let mut visited = [[false; MAZE_H as usize]; MAZE_W as usize];
    visited[0][0] = true;

    while path.len() < PATH_LEN as usize {
        // candidate directions in seed-shuffled order
        let r = keccak_step(&mut state);
        let order = [
            (r as usize) % 4,
            ((r as usize) / 4) % 4,
            ((r as usize) / 16) % 4,
            ((r as usize) / 64) % 4,
        ];
        let mut moved = false;
        // try directions; prefer unvisited cells in-bounds
        for k in 0..4 {
            let d = pick_distinct(&order, k);
            let (nx, ny) = step_xy(x, y, d);
            if nx >= 0 && nx < MAZE_W as i32 && ny >= 0 && ny < MAZE_H as i32
                && !visited[nx as usize][ny as usize] {
                x = nx; y = ny;
                visited[x as usize][y as usize] = true;
                path.push(d);
                moved = true;
                break;
            }
        }
        if !moved {
            // stuck: forced re-seed jump to keep determinism & make progress
            let d = (keccak_step(&mut state)) % 4;
            let (nx, ny) = step_xy(x, y, d);
            if nx >= 0 && nx < MAZE_W as i32 && ny >= 0 && ny < MAZE_H as i32 {
                x = nx; y = ny;
                visited[x as usize][y as usize] = true;
                path.push(d);
            } else {
                // bounce: push a wrap-safe direction
                path.push((d + 2) % 4);
            }
        }
    }
    path
}

fn pick_distinct(order: &[usize; 4], k: usize) -> u8 {
    // produce 4 distinct directions deterministically from a base order
    let base = [DIR_N, DIR_E, DIR_S, DIR_W];
    let mut seen = [false; 4];
    let mut out = [0u8; 4];
    let mut idx = 0;
    for &o in order.iter() {
        if !seen[o] { seen[o] = true; out[idx] = base[o]; idx += 1; }
    }
    for i in 0..4 { if !seen[i] { seen[i] = true; out[idx] = base[i]; idx += 1; } }
    out[k.min(3)]
}

fn step_xy(x: i32, y: i32, dir: u8) -> (i32, i32) {
    match dir {
        DIR_N => (x, y - 1),
        DIR_E => (x + 1, y),
        DIR_S => (x, y + 1),
        DIR_W => (x - 1, y),
        _ => (x, y),
    }
}

// ── enter_maze ─────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct EnterMaze<'info> {
    #[account(mut)]
    pub walker: Signer<'info>,
    #[account(
        init_if_needed, payer = walker, space = 8 + Thread::INIT_SPACE,
        seeds = [b"thread", walker.key().as_ref()], bump
    )]
    pub thread: Account<'info, Thread>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    /// CHECK: treasury XNT sink
    #[account(mut, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
}

pub fn enter_maze(ctx: Context<EnterMaze>) -> Result<()> {
    let t = &mut ctx.accounts.thread;
    require!(!t.active, GardenError::ThreadAlreadyActive);
    let now = Clock::get()?.slot;

    // entry fee → treasury
    let fee = ctx.accounts.eg_config.fee_lamports;
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.walker.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }), fee)?;
    }

    t.walker = ctx.accounts.walker.key();
    t.entry_slot = now;
    t.seed = [0u8; 32];
    t.revealed = false;
    t.pos = 0;
    t.last_checkpoint = 0;
    t.amplifier_bp = 10000; // ×1.0
    t.pending_eg = 0;
    t.reached_heart = false;
    t.active = true;
    t.bump = ctx.bumps.thread;
    msg!("Ariadne: entered the labyrinth at slot {}", now);
    Ok(())
}

// ── reveal_maze ────────────────────────────────────────────────────────────
// After REVEAL_DELAY slots, derive the seed from a recent blockhash the player
// could NOT have known at entry. Uses the slot_hashes sysvar.
#[derive(Accounts)]
pub struct RevealMaze<'info> {
    #[account(mut)]
    pub walker: Signer<'info>,
    #[account(mut, seeds = [b"thread", walker.key().as_ref()], bump = thread.bump,
              has_one = walker @ GardenError::Unauthorized)]
    pub thread: Account<'info, Thread>,
    /// CHECK: slot hashes sysvar, read for blockhash entropy
    #[account(address = anchor_lang::solana_program::sysvar::slot_hashes::ID)]
    pub slot_hashes: UncheckedAccount<'info>,
}

pub fn reveal_maze(ctx: Context<RevealMaze>) -> Result<()> {
    let t = &mut ctx.accounts.thread;
    require!(t.active, GardenError::NoActiveThread);
    require!(!t.revealed, GardenError::MazeAlreadyRevealed);
    let now = Clock::get()?.slot;
    require!(now >= t.entry_slot + REVEAL_DELAY, GardenError::RevealTooEarly);

    // read the most recent slot hash from the sysvar as entropy
    let data = ctx.accounts.slot_hashes.try_borrow_data()?;
    // slot_hashes layout: 8-byte len, then entries of (u64 slot, 32-byte hash)
    require!(data.len() >= 8 + 40, GardenError::RevealTooEarly);
    let mut bh = [0u8; 32];
    bh.copy_from_slice(&data[16..48]); // first entry's hash

    // seed = keccak(walker || blockhash)
    let mut pre = Vec::with_capacity(64);
    pre.extend_from_slice(t.walker.as_ref());
    pre.extend_from_slice(&bh);
    let h = anchor_lang::solana_program::keccak::hash(&pre);
    t.seed = h.to_bytes();
    t.revealed = true;
    msg!("Ariadne: the labyrinth takes shape");
    Ok(())
}

// ── step ───────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct StepThread<'info> {
    #[account(mut)]
    pub walker: Signer<'info>,
    #[account(mut, seeds = [b"thread", walker.key().as_ref()], bump = thread.bump,
              has_one = walker @ GardenError::Unauthorized)]
    pub thread: Account<'info, Thread>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    #[account(init_if_needed, payer = walker,
        associated_token::mint = eg_mint, associated_token::authority = walker)]
    pub walker_eg: Box<Account<'info, TokenAccount>>,
    /// CHECK: treasury
    #[account(mut, address = eg_config.treasury)]
    pub treasury: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn step_thread(ctx: Context<StepThread>, direction: u8) -> Result<()> {
    require!(direction < 4, GardenError::BadDirection);
    let now = Clock::get()?.slot;

    // snapshot config values up front
    let bump = ctx.accounts.eg_config.mint_authority_bump;
    let paused = ctx.accounts.eg_config.paused;
    let fee = ctx.accounts.eg_config.fee_lamports;

    // per-step fee → treasury
    if fee > 0 {
        anchor_lang::system_program::transfer(
            CpiContext::new(ctx.accounts.system_program.to_account_info(),
                anchor_lang::system_program::Transfer {
                    from: ctx.accounts.walker.to_account_info(),
                    to: ctx.accounts.treasury.to_account_info(),
                }), fee)?;
    }

    let t = &mut ctx.accounts.thread;
    require!(t.active, GardenError::NoActiveThread);
    require!(t.revealed, GardenError::MazeNotRevealed);
    require!(!t.reached_heart, GardenError::AlreadyAtHeart);

    let path = true_path(&t.seed);
    let expected = path[t.pos as usize];

    if direction == expected {
        // CORRECT STEP
        t.pos += 1;
        // amplifier climbs
        t.amplifier_bp = (t.amplifier_bp + AMP_STEP_BP).min(AMP_MAX_BP);
        // accrue pending EG = base × amplifier
        let step_eg = (REWARD_STEP_BASE as u128 * t.amplifier_bp as u128 / 10000u128) as u64;
        t.pending_eg = t.pending_eg.saturating_add(step_eg.max(1));

        // reached the heart?
        if t.pos >= PATH_LEN {
            t.pending_eg = t.pending_eg.saturating_add(HEART_BONUS);
            t.reached_heart = true;
            mint_pending(&mut ctx.accounts.eg_config, &ctx.accounts.eg_mint,
                &ctx.accounts.eg_mint_auth, &ctx.accounts.walker_eg,
                &ctx.accounts.token_program, bump, paused, now, t)?;
            t.active = false;
            msg!("Ariadne: YOU REACHED THE HEART. The rose is yours.");
            return Ok(());
        }

        // checkpoint? lock in pending
        if t.pos % CHECKPOINT_EVERY == 0 {
            mint_pending(&mut ctx.accounts.eg_config, &ctx.accounts.eg_mint,
                &ctx.accounts.eg_mint_auth, &ctx.accounts.walker_eg,
                &ctx.accounts.token_program, bump, paused, now, t)?;
            t.last_checkpoint = t.pos;
            msg!("Ariadne: checkpoint at step {} — pending locked in", t.pos);
        } else {
            msg!("Ariadne: step {} correct, amplifier ×{}.{:02}", t.pos,
                 t.amplifier_bp / 10000, (t.amplifier_bp % 10000) / 100);
        }
    } else {
        // WRONG TURN — snap all the way to the gate
        let forfeited = t.pending_eg;
        t.pos = 0;
        t.last_checkpoint = 0;
        t.amplifier_bp = 10000;
        t.pending_eg = 0;
        // extra penalty fee → treasury (the sting)
        if fee > 0 {
            anchor_lang::system_program::transfer(
                CpiContext::new(ctx.accounts.system_program.to_account_info(),
                    anchor_lang::system_program::Transfer {
                        from: ctx.accounts.walker.to_account_info(),
                        to: ctx.accounts.treasury.to_account_info(),
                    }), fee)?;
        }
        msg!("Ariadne: WRONG TURN. The thread snaps — back to the gate. Forfeited {} pending EG.", forfeited);
    }
    Ok(())
}

fn mint_pending<'info>(
    eg_config: &mut Account<'info, EgConfig>,
    eg_mint: &Account<'info, Mint>,
    eg_mint_auth: &UncheckedAccount<'info>,
    walker_eg: &Account<'info, TokenAccount>,
    token_program: &Program<'info, Token>,
    bump: u8, paused: bool, now: u64,
    t: &mut Thread,
) -> Result<()> {
    if paused || t.pending_eg == 0 { t.pending_eg = 0; return Ok(()); }
    let amount = eg_config.reward_amount(t.pending_eg, now);
    if amount > 0 {
        let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
        let signer = &[seeds];
        anchor_spl::token::mint_to(
            CpiContext::new_with_signer(
                token_program.to_account_info(),
                anchor_spl::token::MintTo {
                    mint: eg_mint.to_account_info(),
                    to: walker_eg.to_account_info(),
                    authority: eg_mint_auth.to_account_info(),
                }, signer),
            amount)?;
        eg_config.total_minted = eg_config.total_minted.saturating_add(amount);
    }
    t.pending_eg = 0;
    Ok(())
}

// ── abandon ────────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct AbandonThread<'info> {
    #[account(mut)]
    pub walker: Signer<'info>,
    #[account(mut, seeds = [b"thread", walker.key().as_ref()], bump = thread.bump,
              has_one = walker @ GardenError::Unauthorized, close = walker)]
    pub thread: Account<'info, Thread>,
}

pub fn abandon_thread(_ctx: Context<AbandonThread>) -> Result<()> {
    msg!("Ariadne: the thread is abandoned. The labyrinth forgets you.");
    Ok(())
}
