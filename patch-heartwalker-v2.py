# Idempotent HeartWalker patch — safe to run multiple times.
# Checks for each change before applying; won't double up.
p = 'programs/entropy-garden/src/eg/thread.rs'
s = open(p).read()

def once(s, old, new, marker):
    """Apply replacement only if marker not already present."""
    if marker in s:
        print(f"  [skip] already has: {marker[:40]}")
        return s
    if old not in s:
        print(f"  [WARN] anchor not found for: {marker[:40]}")
        return s
    print(f"  [apply] {marker[:40]}")
    return s.replace(old, new, 1)

# 1. Heart bonus 8 → 25 EG
s = once(s,
    'pub const HEART_BONUS_BP: u64 = 80000; // heart payout in basis points (80000 = 8 EG)',
    'pub const HEART_BONUS_BP: u64 = 250000; // heart payout in basis points (250000 = 25 EG)',
    'HEART_BONUS_BP: u64 = 250000')

# 2. wrong_turns field on Thread
s = once(s,
    '''    pub reached_heart: bool,    // prestige flag
    pub active: bool,           // is this thread in progress
    pub bump: u8,
    pub _reserved: [u8; 32],''',
    '''    pub reached_heart: bool,    // prestige flag
    pub active: bool,           // is this thread in progress
    pub bump: u8,
    pub wrong_turns: u32,       // wrong turns this run (for trophy stats)
    pub _reserved: [u8; 28],''',
    'pub wrong_turns: u32,')

# 3. init wrong_turns in enter_maze
s = once(s,
    '''    t.reached_heart = false;
    t.active = true;
    t.bump = ctx.bumps.thread;''',
    '''    t.reached_heart = false;
    t.active = true;
    t.bump = ctx.bumps.thread;
    t.wrong_turns = 0;''',
    't.wrong_turns = 0;')

# 4. count wrong turns
s = once(s,
    '''        // WRONG TURN — snap all the way to the gate
        let forfeited = t.pending_eg;
        t.pos = 0;''',
    '''        // WRONG TURN — snap all the way to the gate
        let forfeited = t.pending_eg;
        t.wrong_turns = t.wrong_turns.saturating_add(1);
        t.pos = 0;''',
    't.wrong_turns = t.wrong_turns.saturating_add(1);')

# 5. HeartWalker struct (insert after Thread struct closing brace)
heartwalker = '''
// ── HeartWalker: permanent on-chain trophy, one per wallet, never closed ──
#[account]
#[derive(InitSpace)]
pub struct HeartWalker {
    pub walker: Pubkey,
    pub first_heart_slot: u64,
    pub total_hearts: u32,
    pub best_amplifier_bp: u64,
    pub fewest_wrong_turns: u32,
    pub first_seed: [u8; 32],
    pub bump: u8,
    pub _reserved: [u8; 32],
}
'''
if 'pub struct HeartWalker' not in s:
    print("  [apply] HeartWalker struct")
    s = s.replace('    pub _reserved: [u8; 28],\n}\n', '    pub _reserved: [u8; 28],\n}\n' + heartwalker, 1)
else:
    print("  [skip] HeartWalker struct already present")

# 6. heart_walker account in StepThread
s = once(s,
    '''    #[account(init_if_needed, payer = walker,
        associated_token::mint = eg_mint, associated_token::authority = walker)]
    pub walker_eg: Box<Account<'info, TokenAccount>>,''',
    '''    #[account(init_if_needed, payer = walker,
        associated_token::mint = eg_mint, associated_token::authority = walker)]
    pub walker_eg: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = walker, space = 8 + HeartWalker::INIT_SPACE,
        seeds = [b"heartwalker", walker.key().as_ref()], bump)]
    pub heart_walker: Box<Account<'info, HeartWalker>>,''',
    'pub heart_walker:')

# 7. stamp HeartWalker on heart-reached
s = once(s,
    '''            mint_pending(&mut ctx.accounts.eg_config, &ctx.accounts.eg_mint,
                &ctx.accounts.eg_mint_auth, &ctx.accounts.walker_eg,
                &ctx.accounts.token_program, bump, paused, now, t)?;
            t.active = false;
            msg!("Ariadne: YOU REACHED THE HEART. The rose is yours.");
            return Ok(());''',
    '''            mint_pending(&mut ctx.accounts.eg_config, &ctx.accounts.eg_mint,
                &ctx.accounts.eg_mint_auth, &ctx.accounts.walker_eg,
                &ctx.accounts.token_program, bump, paused, now, t)?;
            let hw = &mut ctx.accounts.heart_walker;
            if hw.first_heart_slot == 0 {
                hw.walker = t.walker;
                hw.first_heart_slot = now;
                hw.first_seed = t.seed;
                hw.fewest_wrong_turns = t.wrong_turns;
                hw.best_amplifier_bp = t.amplifier_bp;
                hw.bump = ctx.bumps.heart_walker;
            }
            hw.total_hearts = hw.total_hearts.saturating_add(1);
            if t.amplifier_bp > hw.best_amplifier_bp { hw.best_amplifier_bp = t.amplifier_bp; }
            if t.wrong_turns < hw.fewest_wrong_turns { hw.fewest_wrong_turns = t.wrong_turns; }
            t.active = false;
            msg!("Ariadne: YOU REACHED THE HEART. Trophy stamped. Total hearts: {}", hw.total_hearts);
            return Ok(());''',
    'Trophy stamped')

open(p,'w').write(s)
# verify no duplication
print()
print("HeartWalker struct count:", s.count('pub struct HeartWalker'), "(must be 1)")
print("heart_walker field count:", s.count('pub heart_walker:'), "(must be 1)")
print("Trophy stamped count:", s.count('Trophy stamped'), "(must be 1)")
