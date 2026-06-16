use anchor_lang::prelude::*;

// EG token economics constants (Era-0 base rates, 9 decimals → *1e9 in mint amounts)
pub const EG_DECIMALS: u8 = 9;
pub const EG_UNIT: u64 = 1_000_000_000; // 1 EG in base units

// Era-0 base rewards (in whole EG, multiplied by EG_UNIT at mint time)
pub const REWARD_TEND: u64 = 1;       // 1 EG per tend
pub const REWARD_PLANT: u64 = 5;      // 5 EG per planting
pub const REWARD_HARVEST: u64 = 20;   // 20 EG on a flowering harvest
pub const REWARD_COMPOST: u64 = 1;    // ~1 EG civic reward
// Storm-Chaser: harvest bonus = base * cumulative_stress / DIVISOR (tuned after testing).
pub const STORM_STRESS_DIVISOR: u64 = 335000;

// Fixed allocations (one-time mint, then authority renounced). 9 decimals.
pub const ALLOC_TREASURY: u64 = 50_000_000;   // 5%  — LP seeding (program-locked)
pub const ALLOC_COMMUNITY: u64 = 40_000_000;  // 4%  — airdrops/quests (program-locked)
pub const ALLOC_DEV: u64 = 10_000_000;        // 1%  — dev, paired into public LP
pub const TOTAL_SUPPLY_CAP: u64 = 1_000_000_000; // 1B

// Emission schedule
pub const ERA_SLOTS: u64 = 6_480_000;       // ~30 days at 2.5 slots/sec
pub const GENESIS_BONUS_SLOTS: u64 = 1_512_000; // ~7 days
pub const GENESIS_BONUS_NUM: u64 = 3;       // 1.5x  = *3/2
pub const GENESIS_BONUS_DEN: u64 = 2;
// Era factor ×0.75 per era applied as (3/4)^era via integer mul/div in the reward fn.
pub const ERA_NUM: u64 = 3;
pub const ERA_DEN: u64 = 4;
pub const MAX_ERA_FOR_REWARD: u64 = 40; // beyond this, reward rounds to ~0; clamp to avoid overflow

/// EG mining configuration — a single PDA (seeds = [b"eg_config"]).
/// Created by init_eg_mint; holds the mint and the slot the mining era clock started.
#[account]
#[derive(InitSpace)]
pub struct EgConfig {
    pub authority: Pubkey,        // admin (can renounce by setting to default once stable)
    pub eg_mint: Pubkey,          // the EG SPL mint
    pub mint_authority_bump: u8,  // bump for the mint-authority PDA [b"eg_mint_auth"]
    pub genesis_slot: u64,        // slot at which mining went live (era clock origin)
    pub total_minted: u64,        // running total EG minted by play (base units)
    pub fee_lamports: u64,        // per-action fee in lamports (XNT), governance-adjustable
    pub fee_cap_lamports: u64,    // hard ceiling the fee can never exceed
    pub treasury: Pubkey,         // Treasury PDA (XNT accumulation for LP)
    pub paused: bool,             // EG-minting kill switch (separate from garden pause)
    pub bump: u8,
    pub _reserved: [u8; 64],      // room for staking/holder-registry fields next steps
}

impl EgConfig {
    /// Reward multiplier for the current slot, returned as (numerator, denominator)
    /// so callers can do `base * num / den` in integer math.
    /// Combines era decay (×0.75^era) and the one-week genesis bonus (×1.5).
    pub fn reward_factor(&self, now: u64) -> (u64, u64) {
        let elapsed = now.saturating_sub(self.genesis_slot);
        let era = (elapsed / ERA_SLOTS).min(MAX_ERA_FOR_REWARD);

        // era decay: (3/4)^era  → accumulate into num/den, clamped against overflow
        let mut num: u128 = 1;
        let mut den: u128 = 1;
        for _ in 0..era {
            num *= ERA_NUM as u128;
            den *= ERA_DEN as u128;
            // keep them bounded: if den huge, reward is ~0 anyway
            if den > (1u128 << 90) { return (0, 1); }
        }
        // genesis bonus within the first week
        if elapsed < GENESIS_BONUS_SLOTS {
            num *= GENESIS_BONUS_NUM as u128;
            den *= GENESIS_BONUS_DEN as u128;
        }
        // collapse to u64-safe range
        while num > (u64::MAX as u128) || den > (u64::MAX as u128) {
            num >>= 1; den >>= 1;
            if den == 0 { return (0, 1); }
        }
        (num as u64, den.max(1) as u64)
    }

    /// Compute the EG mint amount (base units) for a base whole-EG reward at `now`.
    pub fn reward_amount(&self, base_whole_eg: u64, now: u64) -> u64 {
        let (num, den) = self.reward_factor(now);
        // base_whole_eg * EG_UNIT * num / den, in u128 to avoid overflow
        let amt = (base_whole_eg as u128)
            .saturating_mul(EG_UNIT as u128)
            .saturating_mul(num as u128)
            / (den as u128).max(1);
        amt.min(u64::MAX as u128) as u64
    }
}

pub mod instructions;
pub mod skyread;
pub mod thread;
pub mod metadata;
pub mod pulse;
