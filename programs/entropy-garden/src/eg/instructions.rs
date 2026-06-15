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

// ---------------------------------------------------------------------------
// mint_allocations — one-time: mint the fixed 5% / 4% / 1% allocations.
// Guarded by _reserved[0] flag (0 = not yet minted, 1 = done). Authority-only.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct MintAllocations<'info> {
    #[account(mut, address = eg_config.authority @ GardenError::Unauthorized)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
    #[account(mut, seeds = [b"eg_mint"], bump, address = eg_config.eg_mint)]
    pub eg_mint: Account<'info, Mint>,
    /// CHECK: mint authority PDA
    #[account(seeds = [b"eg_mint_auth"], bump = eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    /// Treasury EG token account (owned by treasury PDA)
    #[account(mut)]
    pub treasury_eg: Account<'info, anchor_spl::token::TokenAccount>,
    /// Community EG token account
    #[account(mut)]
    pub community_eg: Account<'info, anchor_spl::token::TokenAccount>,
    /// Dev EG token account
    #[account(mut)]
    pub dev_eg: Account<'info, anchor_spl::token::TokenAccount>,
    pub token_program: Program<'info, anchor_spl::token::Token>,
}

pub fn mint_allocations(ctx: Context<MintAllocations>) -> Result<()> {
    let cfg = &mut ctx.accounts.eg_config;
    // guard: _reserved[0] == 1 means already minted
    require!(cfg._reserved[0] == 0, GardenError::AllocationsAlreadyMinted);

    let bump = cfg.mint_authority_bump;
    let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
    let signer = &[seeds];

    let mints = [
        (&ctx.accounts.treasury_eg,  ALLOC_TREASURY),
        (&ctx.accounts.community_eg, ALLOC_COMMUNITY),
        (&ctx.accounts.dev_eg,       ALLOC_DEV),
    ];
    for (acct, whole) in mints.iter() {
        let amount = (*whole).saturating_mul(EG_UNIT);
        anchor_spl::token::mint_to(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.to_account_info(),
                anchor_spl::token::MintTo {
                    mint: ctx.accounts.eg_mint.to_account_info(),
                    to: acct.to_account_info(),
                    authority: ctx.accounts.eg_mint_auth.to_account_info(),
                },
                signer,
            ),
            amount,
        )?;
        cfg.total_minted = cfg.total_minted.saturating_add(amount);
    }
    cfg._reserved[0] = 1; // mark allocations minted
    msg!("Allocations minted: 5% treasury, 4% community, 1% dev");
    Ok(())
}

// ---------------------------------------------------------------------------
// renounce_mint_authority — permanently set the mint's authority to None.
// After this, no human and no PDA can mint EG via allocations; only the
// program's mining instructions (which sign with the PDA) continue. To make
// supply truly fixed we instead flip a flag the mining path checks.
// NOTE: we keep mining alive (PDA-signed) but lock out any NEW allocation.
// ---------------------------------------------------------------------------

#[derive(Accounts)]
pub struct RenounceAuthority<'info> {
    #[account(mut, address = eg_config.authority @ GardenError::Unauthorized)]
    pub authority: Signer<'info>,
    #[account(mut, seeds = [b"eg_config"], bump = eg_config.bump)]
    pub eg_config: Account<'info, EgConfig>,
}

pub fn renounce_mint_authority(ctx: Context<RenounceAuthority>) -> Result<()> {
    let cfg = &mut ctx.accounts.eg_config;
    // _reserved[1] == 1 means admin authority renounced (allocations can never re-run)
    cfg._reserved[1] = 1;
    cfg.authority = Pubkey::default();
    msg!("Admin authority renounced — allocations permanently locked");
    Ok(())
}
