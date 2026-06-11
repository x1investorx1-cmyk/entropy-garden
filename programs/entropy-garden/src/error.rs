use anchor_lang::prelude::*;

#[error_code]
pub enum GardenError {
    #[msg("Garden is paused")]
    Paused,
    #[msg("Unauthorized")]
    Unauthorized,
    #[msg("Region is not open")]
    RegionClosed,
    #[msg("Beta plot cap reached")]
    PlotCapReached,
    #[msg("Nutrient pool exhausted")]
    PoolExhausted,
    #[msg("Invalid plant slot index")]
    BadSlotIndex,
    #[msg("Plant slot already occupied")]
    SlotOccupied,
    #[msg("Not enough soil nutrients")]
    NotEnoughSoil,
    #[msg("Plant is dead")]
    PlantDead,
    #[msg("Plant is still alive")]
    PlantAlive,
    #[msg("Tend cooldown not elapsed")]
    TendCooldown,
    #[msg("Plant does not belong to this plot")]
    WrongPlot,
    #[msg("Plot does not belong to this region")]
    WrongRegion,
    #[msg("Weather sample out of range")]
    BadSample,
    #[msg("Weather sample too old or from the future")]
    StaleSample,
    #[msg("Weather updates too frequent")]
    TooFrequent,
    #[msg("No compost shares")]
    NoShares,
    #[msg("Nothing to draw")]
    NothingToDraw,
}
