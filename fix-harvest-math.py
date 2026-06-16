p = 'programs/entropy-garden/src/eg/carbon.rs'
s = open(p).read()
import re

# 1. MIN_HARVEST_MASS → 100_000 (= HARVEST_DIV, so min reward is exactly 1 EG)
s = re.sub(r'pub const MIN_HARVEST_MASS: u64 = \d[\d_]*;',
           'pub const MIN_HARVEST_MASS: u64 = 100_000;', s)
print("MIN_HARVEST_MASS → 100_000")

# 2. Guard: if amount == 0, do NOT reset (prevent harvest-for-nothing trap).
#    Replace the mint+reset block so reset only happens when amount > 0.
old = '''    if amount > 0 {
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
    }

    let s = &mut ctx.accounts.sink;
    s.total_harvested = s.total_harvested.saturating_add(amount);
    s.root_mass = 0;
    s.leaf_mass = 0;
    s.last_slot = now;
    msg!("carbon harvest: mass {} balance {}% → +{} EG", total_mass, balance, amount);
    Ok(())'''
new = '''    // guard: never reset the sink for a zero reward (don't trap a player's mass)
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
    Ok(())'''
if old in s:
    s = s.replace(old, new)
    print("harvest guard added (no reset on 0 reward)")
else:
    print("WARN: harvest block not matched — check manually")

open(p,'w').write(s)
# verify
print("MIN check:", "MIN_HARVEST_MASS: u64 = 100_000" in s)
print("guard:", "require!(amount > 0, GardenError::NotFlowering);" in s)
