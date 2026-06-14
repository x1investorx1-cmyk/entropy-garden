use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};
use crate::eg::*;
use crate::error::GardenError;

// ---------------------------------------------------------------------------
// init_eg_mint — one-time: create EG mint + EgConfig, authority = program PDA
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct InitEgMint<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        space = 8 + EgConfig::INIT_SPACE,
        seeds = [b"eg_config"],
        bump
    )]
    pub eg_config: Account<'info, EgConfig>,

    /// The EG mint — authority is the mint-authority PDA.
    #[account(
        init,
        payer = authority,
        mint::decimals = EG_DECIMALS,
        mint::authority = eg_mint_auth,
        seeds = [b"eg_mint"],
        bump
    )]
    pub eg_mint: Account<'info, Mint>,

    /// CHECK: PDA that holds mint authority; never signs except via seeds.
    #[account(seeds = [b"eg_mint_auth"], bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,

    /// CHECK: Treasury PDA — owns XNT accumulated from fees. Validated by seeds.
    #[account(
        mut,
        seeds = [b"eg_treasury"],
        bump
    )]
    pub treasury: UncheckedAccount<'info>,

    pub system_program: Program<'info, System>,
    pub token_program: Program<'info, Token>,
    pub rent: Sysvar<'info, Rent>,
}

pub fn init_eg_mint(ctx: Context<InitEgMint>, fee_lamports: u64, fee_cap_lamports: u64) -> Result<()> {
    require!(fee_lamports <= fee_cap_lamports, GardenError::FeeAboveCap);
    let now = Clock::get()?.slot;
    let cfg = &mut ctx.accounts.eg_config;
    cfg.authority = ctx.accounts.authority.key();
    cfg.eg_mint = ctx.accounts.eg_mint.key();
    cfg.mint_authority_bump = ctx.bumps.eg_mint_auth;
    cfg.genesis_slot = now;
    cfg.total_minted = 0;
    cfg.fee_lamports = fee_lamports;
    cfg.fee_cap_lamports = fee_cap_lamports;
    cfg.treasury = ctx.accounts.treasury.key();
    cfg.paused = false;
    cfg.bump = ctx.bumps.eg_config;
    cfg._reserved = [0u8; 64];
    msg!("EG mint live: {} | era clock starts slot {}", cfg.eg_mint, now);
    Ok(())
}
