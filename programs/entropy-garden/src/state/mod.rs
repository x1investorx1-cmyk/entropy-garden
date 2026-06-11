use anchor_lang::prelude::*;

pub const HEALTH_MAX: u16 = 1_000;
pub const PLANT_SLOTS: usize = 6;
pub const WEATHER_RING: usize = 64;

#[account]
#[derive(InitSpace)]
pub struct GardenConfig {
    pub authority: Pubkey,
    pub regions: u16,
    /// Health units lost per 1000 slots at zero stress.
    pub base_decay_rate: u32,
    /// Max biomass gained per 1000 slots at perfect comfort.
    pub base_growth_rate: u32,
    /// bps of dead biomass routed to the global pool on compost.
    pub compost_pool_bps: u16,
    /// bps of dead biomass paid to the composter as bounty.
    pub compost_bounty_bps: u16,
    /// Lamports paid to a crank per accepted weather sample.
    pub crank_reward_lamports: u64,
    /// Minimum slots between tends of the same plant.
    pub tend_cooldown_slots: u64,
    /// Soil allocation granted to a newly claimed plot.
    pub genesis_soil_per_plot: u64,
    /// Beta guardrail: cap on total plots.
    pub max_plots: u32,
    pub total_plots: u32,
    pub paused: bool,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct NutrientPool {
    /// Invariant: balance + Σ plot.soil + Σ plant.biomass == genesis total.
    pub balance: u64,
    pub genesis_total: u64,
    /// Share accounting for draw_nutrients (staking-pool math).
    pub total_compost_shares: u128,
    /// Accumulated pool-reward per share, scaled by 1e12.
    pub acc_reward_per_share: u128,
    pub bump: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum WeatherChannel {
    DefiSwaps,
    NftMints,
    Transfers,
    FeeMarket,
}

#[account]
#[derive(InitSpace)]
pub struct Region {
    pub region_id: u16,
    pub channel: WeatherChannel,
    /// Last accepted (clamped) weather value, bps.
    pub current_weather_bps: u16,
    pub last_weather_slot: u64,
    pub plot_count: u32,
    pub open: bool,
    pub bump: u8,
}

/// Ring buffer of accepted weather samples for median consumption.
#[account]
#[derive(InitSpace)]
pub struct WeatherFeed {
    pub region_id: u16,
    pub head: u8,
    pub len: u8,
    pub samples_bps: [u16; WEATHER_RING],
    pub sample_slots: [u64; WEATHER_RING],
    pub bump: u8,
}

impl WeatherFeed {
    pub fn push(&mut self, bps: u16, slot: u64) {
        let i = self.head as usize;
        self.samples_bps[i] = bps;
        self.sample_slots[i] = slot;
        self.head = ((i + 1) % WEATHER_RING) as u8;
        if (self.len as usize) < WEATHER_RING {
            self.len += 1;
        }
    }
    /// Samples newer than `min_slot`, most useful for lazy evaluation windows.
    pub fn window(&self, min_slot: u64) -> Vec<u16> {
        (0..self.len as usize)
            .filter(|&i| self.sample_slots[i] >= min_slot)
            .map(|i| self.samples_bps[i])
            .collect()
    }
}

#[account]
#[derive(InitSpace)]
pub struct Plot {
    pub owner: Pubkey,
    pub region: Pubkey,
    pub plot_index: u32,
    pub soil_nutrients: u64,
    pub plants: [Option<Pubkey>; PLANT_SLOTS],
    /// Lifetime compost contribution shares (for pool draws).
    pub compost_shares: u128,
    /// Reward debt for share accounting.
    pub reward_debt: u128,
    pub bump: u8,
}

#[account]
#[derive(InitSpace)]
pub struct Plant {
    pub plot: Pubkey,
    pub slot_index: u8,
    pub species: u16,
    pub genome: [u8; 32],
    /// Weather value (bps) at which this plant thrives.
    pub optimal_bps: u16,
    pub planted_slot: u64,
    pub last_evaluated_slot: u64,
    pub health: u16,
    pub biomass: u64,
    pub growth_stage: u8,
    pub bump: u8,
}

impl Plant {
    pub fn is_dead(&self) -> bool {
        self.health == 0
    }
}
