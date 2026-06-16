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
    #[msg("Fee exceeds the hard cap.")]
    FeeAboveCap,
    #[msg("EG minting is paused.")]
    EgPaused,
    #[msg("Plant must be at stage 5 (flowering) to harvest.")]
    NotFlowering,
    #[msg("Allocations have already been minted.")]
    AllocationsAlreadyMinted,
    #[msg("Forecast window out of allowed range.")]
    BadForecastWindow,
    #[msg("Forecast cannot be resolved yet.")]
    ForecastNotReady,
    #[msg("You already have an active thread.")]
    ThreadAlreadyActive,
    #[msg("No active thread.")]
    NoActiveThread,
    #[msg("The maze has already been revealed.")]
    MazeAlreadyRevealed,
    #[msg("The maze hasn't been revealed yet.")]
    MazeNotRevealed,
    #[msg("Too early to reveal the maze.")]
    RevealTooEarly,
    #[msg("Invalid direction.")]
    BadDirection,
    #[msg("You're already at the heart.")]
    AlreadyAtHeart,
}
