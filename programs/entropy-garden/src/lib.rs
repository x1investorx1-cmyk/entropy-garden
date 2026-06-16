use anchor_lang::prelude::*;

pub mod eg;
pub mod error;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;
use eg::instructions::*;
use eg::skyread::*;
use eg::thread::*;
use state::WeatherChannel;

declare_id!("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");

#[program]
pub mod entropy_garden {
    use super::*;

    pub fn initialize_garden(ctx: Context<InitializeGarden>, genesis_nutrients: u64) -> Result<()> {
        instructions::initialize_garden(ctx, genesis_nutrients)
    }

    pub fn create_region(ctx: Context<CreateRegion>, region_id: u16, channel: WeatherChannel) -> Result<()> {
        instructions::create_region(ctx, region_id, channel)
    }

    pub fn update_weather(ctx: Context<UpdateWeather>, proposed_bps: u16, sampled_slot: u64) -> Result<()> {
        instructions::update_weather(ctx, proposed_bps, sampled_slot)
    }

    pub fn claim_plot(ctx: Context<ClaimPlot>) -> Result<()> {
        instructions::claim_plot(ctx)
    }

    pub fn plant_seed(ctx: Context<PlantSeed>, slot_index: u8, species: u16) -> Result<()> {
        instructions::plant_seed(ctx, slot_index, species)
    }

    pub fn tend(ctx: Context<Tend>) -> Result<()> {
        instructions::tend(ctx)
    }

    pub fn compost(ctx: Context<Compost>) -> Result<()> {
        instructions::compost(ctx)
    }

    pub fn draw_nutrients(ctx: Context<DrawNutrients>) -> Result<()> {
        instructions::draw_nutrients(ctx)
    }

    pub fn set_paused(ctx: Context<AdminOnly>, paused: bool) -> Result<()> {
        instructions::set_paused(ctx, paused)
    }

    pub fn set_max_plots(ctx: Context<AdminOnly>, max_plots: u32) -> Result<()> {
        instructions::set_max_plots(ctx, max_plots)
    }

    pub fn harvest(ctx: Context<Harvest>) -> Result<()> {
        instructions::harvest(ctx)
    }

    pub fn mint_allocations(ctx: Context<MintAllocations>) -> Result<()> {
        eg::instructions::mint_allocations(ctx)
    }

    pub fn renounce_mint_authority(ctx: Context<RenounceAuthority>) -> Result<()> {
        eg::instructions::renounce_mint_authority(ctx)
    }

    pub fn enter_maze(ctx: Context<EnterMaze>) -> Result<()> {
        eg::thread::enter_maze(ctx)
    }
    pub fn reveal_maze(ctx: Context<RevealMaze>) -> Result<()> {
        eg::thread::reveal_maze(ctx)
    }
    pub fn step_thread(ctx: Context<StepThread>, direction: u8) -> Result<()> {
        eg::thread::step_thread(ctx, direction)
    }
    pub fn abandon_thread(ctx: Context<AbandonThread>) -> Result<()> {
        eg::thread::abandon_thread(ctx)
    }

    pub fn commit_forecast(ctx: Context<CommitForecast>, region_id: u16, predict_storm: bool, window_slots: u64, commit_slot: u64) -> Result<()> {
        eg::skyread::commit_forecast(ctx, region_id, predict_storm, window_slots, commit_slot)
    }

    pub fn resolve_forecast(ctx: Context<ResolveForecast>) -> Result<()> {
        eg::skyread::resolve_forecast(ctx)
    }

    pub fn init_eg_mint(ctx: Context<InitEgMint>, fee_lamports: u64, fee_cap_lamports: u64) -> Result<()> {
        eg::instructions::init_eg_mint(ctx, fee_lamports, fee_cap_lamports)
    }
}
