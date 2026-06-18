use anchor_lang::prelude::*;
use anchor_lang::solana_program::keccak;
use anchor_spl::token::{Mint, Token, TokenAccount, Transfer};
use anchor_spl::associated_token::AssociatedToken;
use crate::eg::EgConfig;
use crate::error::GardenError;

// ───────────────────────────────────────────────────────────────────────────
// THE AIRDROP — merkle-claim distribution from the Community allocation.
// Eligible wallets (in a merkle tree) claim a fixed amount of EG each.
// One claim per wallet. EG comes from community_eg, signed by the
// [b"eg_community"] PDA. No new EG minted — pure distribution.
// ───────────────────────────────────────────────────────────────────────────

#[account]
#[derive(InitSpace)]
pub struct AirdropConfig {
    pub authority:        Pubkey,    // admin who set it up (can close)
    pub merkle_root:      [u8; 32],  // root of the eligible-wallet tree
    pub amount_per_claim: u64,       // EG per wallet (base units)
    pub total_claimed:    u64,       // running total claimed
    pub num_claims:       u32,       // how many wallets have claimed
    pub paused:           bool,
    pub bump:             u8,
    pub _reserved:        [u8; 32],
}

#[account]
#[derive(InitSpace)]
pub struct AirdropClaim {
    pub wallet:  Pubkey,
    pub amount:  u64,
    pub bump:    u8,
}

// ── verify a sorted merkle proof (matches the JS builder) ──────────────────
fn verify_proof(proof: &[[u8; 32]], root: &[u8; 32], leaf: &[u8; 32]) -> bool {
    let mut computed = *leaf;
    for sibling in proof.iter() {
        // sorted pair: hash(min(a,b) || max(a,b))
        computed = if computed <= *sibling {
            keccak::hashv(&[&computed, sibling]).0
        } else {
            keccak::hashv(&[sibling, &computed]).0
        };
    }
    computed == *root
}

// ── init_airdrop ────────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct InitAirdrop<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init, payer = authority, space = 8 + AirdropConfig::INIT_SPACE,
        seeds = [b"airdrop_config"], bump
    )]
    pub airdrop: Account<'info, AirdropConfig>,
    #[account(seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    pub system_program: Program<'info, System>,
}

pub fn init_airdrop(ctx: Context<InitAirdrop>, merkle_root: [u8; 32], amount_per_claim: u64) -> Result<()> {
    // only the eg_config authority can set up the airdrop
    require!(ctx.accounts.authority.key() == ctx.accounts.eg_config.authority, GardenError::Unauthorized);
    let a = &mut ctx.accounts.airdrop;
    a.authority        = ctx.accounts.authority.key();
    a.merkle_root      = merkle_root;
    a.amount_per_claim = amount_per_claim;
    a.total_claimed    = 0;
    a.num_claims       = 0;
    a.paused           = false;
    a.bump             = ctx.bumps.airdrop;
    a._reserved        = [0u8; 32];
    msg!("Airdrop initialized: {} EG per claim", amount_per_claim / 1_000_000_000);
    Ok(())
}

// ── claim_airdrop ───────────────────────────────────────────────────────────
#[derive(Accounts)]
pub struct ClaimAirdrop<'info> {
    #[account(mut)]
    pub claimer: Signer<'info>,
    #[account(mut, seeds = [b"airdrop_config"], bump = airdrop.bump)]
    pub airdrop: Account<'info, AirdropConfig>,
    // claim record — init prevents double-claims (one per wallet)
    #[account(
        init, payer = claimer, space = 8 + AirdropClaim::INIT_SPACE,
        seeds = [b"airdrop_claim", claimer.key().as_ref()], bump
    )]
    pub claim_record: Account<'info, AirdropClaim>,
    // community allocation source — constrained to be the EG mint
    // and owned by the [b"eg_community"] PDA authority below.
    #[account(
        mut,
        token::mint = eg_mint,
        token::authority = community_authority,
    )]
    pub community_eg: Box<Account<'info, TokenAccount>>,
    /// CHECK: the [b"eg_community"] PDA that owns community_eg
    #[account(seeds = [b"eg_community"], bump)]
    pub community_authority: UncheckedAccount<'info>,
    // claimer's EG token account (created if needed)
    #[account(
        init_if_needed, payer = claimer,
        associated_token::mint = eg_mint,
        associated_token::authority = claimer,
    )]
    pub claimer_eg: Box<Account<'info, TokenAccount>>,
    #[account(seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Box<Account<'info, Mint>>,
    #[account(seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    pub token_program: Program<'info, Token>,
    pub associated_token_program: Program<'info, AssociatedToken>,
    pub system_program: Program<'info, System>,
}

pub fn claim_airdrop(ctx: Context<ClaimAirdrop>, proof: Vec<[u8; 32]>) -> Result<()> {
    let a = &mut ctx.accounts.airdrop;
    require!(!a.paused, GardenError::Paused);

    // leaf = keccak256(claimer pubkey)
    let leaf = keccak::hash(&ctx.accounts.claimer.key().to_bytes()).0;
    require!(verify_proof(&proof, &a.merkle_root, &leaf), GardenError::Unauthorized);

    // transfer amount_per_claim from community_eg → claimer, signed by [b"eg_community"]
    let bump = ctx.bumps.community_authority;
    let seeds: &[&[u8]] = &[b"eg_community", &[bump]];
    let signer = &[seeds];
    anchor_spl::token::transfer(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.to_account_info(),
            Transfer {
                from: ctx.accounts.community_eg.to_account_info(),
                to: ctx.accounts.claimer_eg.to_account_info(),
                authority: ctx.accounts.community_authority.to_account_info(),
            },
            signer,
        ),
        a.amount_per_claim,
    )?;

    // record the claim
    let rec = &mut ctx.accounts.claim_record;
    rec.wallet = ctx.accounts.claimer.key();
    rec.amount = a.amount_per_claim;
    rec.bump   = ctx.bumps.claim_record;

    a.total_claimed = a.total_claimed.saturating_add(a.amount_per_claim);
    a.num_claims    = a.num_claims.saturating_add(1);
    msg!("airdrop claim: {} EG to {}", a.amount_per_claim / 1_000_000_000, ctx.accounts.claimer.key());
    Ok(())
}
