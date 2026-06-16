import re
p = 'programs/entropy-garden/src/eg/thread.rs'
s = open(p).read()
orig = s

# ── 1. Heart bonus 8 → 25 EG ──
s = s.replace(
    'pub const HEART_BONUS_BP: u64 = 80000; // heart payout in basis points (80000 = 8 EG)',
    'pub const HEART_BONUS_BP: u64 = 250000; // heart payout in basis points (250000 = 25 EG)')

# ── 2. Add wrong_turns tracking to Thread (use first 4 reserved bytes as u32) ──
# We track wrong_turns in a real field by repurposing reserved. Cleaner: add a field.
# Thread has _reserved[32]; we convert 4 bytes to wrong_turns:u32 and shrink reserved to 28.
s = s.replace(
    '''    pub reached_heart: bool,    // prestige flag
    pub active: bool,           // is this thread in progress
    pub bump: u8,
    pub _reserved: [u8; 32],''',
    '''    pub reached_heart: bool,    // prestige flag
    pub active: bool,           // is this thread in progress
    pub bump: u8,
    pub wrong_turns: u32,       // wrong turns this run (for trophy stats)
    pub _reserved: [u8; 28],''')

# init wrong_turns in enter_maze
s = s.replace(
    '''    t.reached_heart = false;
    t.active = true;
    t.bump = ctx.bumps.thread;''',
    '''    t.reached_heart = false;
    t.active = true;
    t.bump = ctx.bumps.thread;
    t.wrong_turns = 0;''')

# count wrong turns in the wrong-turn branch
s = s.replace(
    '''        // WRONG TURN — snap all the way to the gate
        let forfeited = t.pending_eg;
        t.pos = 0;''',
    '''        // WRONG TURN — snap all the way to the gate
        let forfeited = t.pending_eg;
        t.wrong_turns = t.wrong_turns.saturating_add(1);
        t.pos = 0;''')

# ── 3. HeartWalker permanent trophy account ──
heartwalker = '''
// ── HeartWalker: permanent on-chain trophy, one per wallet, never closed ──
#[account]
#[derive(InitSpace)]
pub struct HeartWalker {
    pub walker: Pubkey,
    pub first_heart_slot: u64,      // when they first reached the heart
    pub total_hearts: u32,          // how many times they've completed the labyrinth
    pub best_amplifier_bp: u64,     // highest amplifier at a heart-reach
    pub fewest_wrong_turns: u32,    // best (lowest) wrong-turn count on a winning run
    pub first_seed: [u8; 32],       // seed of their first victory → renders trophy rose
    pub bump: u8,
    pub _reserved: [u8; 32],
}
'''
# insert HeartWalker struct after the Thread struct's closing
s = s.replace('    pub _reserved: [u8; 28],\n}\n', '    pub _reserved: [u8; 28],\n}\n' + heartwalker, 1)

# ── 4. Add heart_walker account to StepThread ──
s = s.replace(
    '''    #[account(init_if_needed, payer = walker,
        associated_token::mint = eg_mint, associated_token::authority = walker)]
    pub walker_eg: Box<Account<'info, TokenAccount>>,''',
    '''    #[account(init_if_needed, payer = walker,
        associated_token::mint = eg_mint, associated_token::authority = walker)]
    pub walker_eg: Box<Account<'info, TokenAccount>>,
    #[account(init_if_needed, payer = walker, space = 8 + HeartWalker::INIT_SPACE,
        seeds = [b"heartwalker", walker.key().as_ref()], bump)]
    pub heart_walker: Box<Account<'info, HeartWalker>>,''')

# ── 5. Stamp the HeartWalker when the heart is reached ──
# In the heart branch, after minting, before closing the thread.
s = s.replace(
    '''            mint_pending(&mut ctx.accounts.eg_config, &ctx.accounts.eg_mint,
                &ctx.accounts.eg_mint_auth, &ctx.accounts.walker_eg,
                &ctx.accounts.token_program, bump, paused, now, t)?;
            t.active = false;
            msg!("Ariadne: YOU REACHED THE HEART. The rose is yours.");
            return Ok(());''',
    '''            mint_pending(&mut ctx.accounts.eg_config, &ctx.accounts.eg_mint,
                &ctx.accounts.eg_mint_auth, &ctx.accounts.walker_eg,
                &ctx.accounts.token_program, bump, paused, now, t)?;
            // stamp the permanent trophy
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
            return Ok(());'''
)

open(p,'w').write(s)
changed = s != orig
print("patches applied:", changed)
print("heart bonus 25 EG:", "250000 = 25 EG" in s)
print("wrong_turns field:", "pub wrong_turns: u32" in s)
print("HeartWalker struct:", "pub struct HeartWalker" in s)
print("heart_walker account:", "pub heart_walker:" in s)
print("trophy stamped:", "Trophy stamped" in s)
