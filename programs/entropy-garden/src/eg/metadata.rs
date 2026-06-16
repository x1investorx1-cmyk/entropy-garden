use anchor_lang::prelude::*;
use anchor_spl::token::Mint;
use crate::eg::EgConfig;
use crate::error::GardenError;

pub const MPL_TOKEN_METADATA_ID: &str = "metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s";

#[derive(Accounts)]
pub struct InitializeTokenMetadata<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(seeds=[b"eg_config"], bump=eg_config.bump,
              has_one=authority @ GardenError::Unauthorized)]
    pub eg_config: Account<'info, EgConfig>,
    #[account(mut, seeds=[b"eg_mint"], bump, address=eg_config.eg_mint)]
    pub eg_mint: Account<'info, Mint>,
    /// CHECK: our mint authority PDA — signs via invoke_signed
    #[account(seeds=[b"eg_mint_auth"], bump=eg_config.mint_authority_bump)]
    pub eg_mint_auth: UncheckedAccount<'info>,
    /// CHECK: Metaplex metadata PDA (will be created)
    #[account(mut)]
    pub metadata_account: UncheckedAccount<'info>,
    /// CHECK: Metaplex Token Metadata program
    #[account(address = MPL_TOKEN_METADATA_ID.parse::<Pubkey>().unwrap())]
    pub token_metadata_program: UncheckedAccount<'info>,
    pub system_program: Program<'info, System>,
    /// CHECK: rent sysvar
    #[account(address = anchor_lang::solana_program::sysvar::rent::ID)]
    pub rent: UncheckedAccount<'info>,
}

pub fn initialize_token_metadata(
    ctx: Context<InitializeTokenMetadata>,
    name: String,
    symbol: String,
    uri: String,
) -> Result<()> {
    let bump = ctx.accounts.eg_config.mint_authority_bump;
    let seeds: &[&[u8]] = &[b"eg_mint_auth", &[bump]];
    let signer_seeds = &[seeds];

    // Build the CreateMetadataAccountV3 instruction manually
    // to avoid a heavy Metaplex dependency. The instruction discriminator
    // and layout are stable and well-documented.
    let metadata_program_id = ctx.accounts.token_metadata_program.key();

    // Instruction data: discriminator (33) + DataV2 layout
    // name (4+len), symbol (4+len), uri (4+len), seller_fee=0, creators=None, collection=None, uses=None
    // is_mutable = true, collection_details = None
    let name_bytes = name.as_bytes();
    let symbol_bytes = symbol.as_bytes();
    let uri_bytes = uri.as_bytes();

    let mut data = vec![33u8]; // CreateMetadataAccountV3 discriminator
    // DataV2
    data.extend_from_slice(&(name_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(name_bytes);
    data.extend_from_slice(&(symbol_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(symbol_bytes);
    data.extend_from_slice(&(uri_bytes.len() as u32).to_le_bytes());
    data.extend_from_slice(uri_bytes);
    data.extend_from_slice(&0u16.to_le_bytes()); // seller_fee_basis_points
    data.push(0); // creators = None
    data.push(0); // collection = None
    data.push(0); // uses = None
    // is_mutable
    data.push(1u8);
    // collection_details = None
    data.push(0);

    let ix = anchor_lang::solana_program::instruction::Instruction {
        program_id: metadata_program_id,
        accounts: vec![
            anchor_lang::solana_program::instruction::AccountMeta::new(
                ctx.accounts.metadata_account.key(), false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                ctx.accounts.eg_mint.key(), false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                ctx.accounts.eg_mint_auth.key(), true), // mint_authority must sign
            anchor_lang::solana_program::instruction::AccountMeta::new(
                ctx.accounts.authority.key(), true), // payer
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                ctx.accounts.authority.key(), true), // update_authority
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                anchor_lang::solana_program::system_program::ID, false),
            anchor_lang::solana_program::instruction::AccountMeta::new_readonly(
                ctx.accounts.rent.key(), false),
        ],
        data,
    };

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.metadata_account.to_account_info(),
            ctx.accounts.eg_mint.to_account_info(),
            ctx.accounts.eg_mint_auth.to_account_info(),
            ctx.accounts.authority.to_account_info(),
            ctx.accounts.system_program.to_account_info(),
            ctx.accounts.rent.to_account_info(),
            ctx.accounts.token_metadata_program.to_account_info(),
        ],
        signer_seeds,
    )?;

    msg!("EG token metadata created: {} ({})", name, symbol);
    Ok(())
}
