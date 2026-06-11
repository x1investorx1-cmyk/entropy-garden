//! Pure, integer-only decay/growth math. Consensus-critical.
//!
//! No floats, no dependencies, no randomness. Every function is total and
//! saturating. This module compiles on the host target so `cargo test` runs
//! it without any Solana toolchain.

/// Basis points denominator.
pub const BPS: u64 = 10_000;
/// Maximum plant health.
pub const MAX_HEALTH: u16 = 1_000;
/// Hard clamp: a single lazy evaluation can never remove more than 40% of
/// max health, regardless of elapsed slots or weather. Anti flash-kill.
pub const MAX_DECAY_PER_EVAL: u64 = (MAX_HEALTH as u64) * 4 / 10; // 400
/// Weather samples may not move more than this many bps from the previous
/// accepted sample. Anti oracle-manipulation.
pub const MAX_WEATHER_DELTA_BPS: u16 = 1_500;
/// Slots per "tick" used as the time denominator in rate constants.
pub const SLOTS_PER_TICK: u64 = 1_000;
/// Stress denominator: comfort reaches zero at stress >= 5000 bps.
pub const COMFORT_ZERO_STRESS: u64 = 5_000;
/// Stress curve knee: multiplier is 2x at this stress.
const STRESS_KNEE: u64 = 2_500;

/// Distance between current weather and a plant's optimal, in bps.
#[inline]
pub fn stress(weather_bps: u16, optimal_bps: u16) -> u16 {
    weather_bps.abs_diff(optimal_bps)
}

/// Quadratic stress multiplier in bps: 1 + (stress/2500)^2.
/// stress 0     -> 10_000 (1.00x)
/// stress 2500  -> 20_000 (2.00x)
/// stress 5000  -> 50_000 (5.00x)
/// stress 10000 -> 170_000 (17.0x)
#[inline]
pub fn stress_multiplier_bps(stress: u16) -> u64 {
    let s = stress as u64;
    BPS + (s * s * BPS) / (STRESS_KNEE * STRESS_KNEE)
}

/// Health decay over `elapsed` slots, clamped. `base_rate` = health units
/// lost per SLOTS_PER_TICK at zero stress.
#[inline]
pub fn decay(base_rate: u32, elapsed: u64, stress: u16) -> u64 {
    let raw = (base_rate as u64)
        .saturating_mul(elapsed)
        .saturating_mul(stress_multiplier_bps(stress))
        / (SLOTS_PER_TICK * BPS);
    raw.min(MAX_DECAY_PER_EVAL)
}

/// Comfort factor in bps: 1 - stress/5000, floored at 0.
#[inline]
pub fn comfort_bps(stress: u16) -> u64 {
    BPS.saturating_sub((stress as u64).saturating_mul(BPS) / COMFORT_ZERO_STRESS)
}

/// Biomass growth over `elapsed` slots. Returns (growth, nutrients_consumed).
/// Growth converts soil nutrients into biomass 1:1 (conservation law), gated
/// by comfort. `g0` = max biomass gain per SLOTS_PER_TICK at perfect comfort.
#[inline]
pub fn growth(g0: u32, elapsed: u64, stress: u16, soil_available: u64) -> (u64, u64) {
    let wanted = (g0 as u64).saturating_mul(elapsed).saturating_mul(comfort_bps(stress))
        / (SLOTS_PER_TICK * BPS);
    let actual = wanted.min(soil_available);
    (actual, actual)
}

/// Apply one lazy evaluation step to (health, biomass, soil).
/// Returns the new triple. Pure; caller persists.
pub fn evaluate(
    health: u16,
    biomass: u64,
    soil: u64,
    base_rate: u32,
    g0: u32,
    elapsed: u64,
    weather_bps: u16,
    optimal_bps: u16,
) -> (u16, u64, u64) {
    let s = stress(weather_bps, optimal_bps);
    let d = decay(base_rate, elapsed, s);
    let new_health = (health as u64).saturating_sub(d) as u16;
    if new_health == 0 {
        // Dead plants neither grow nor consume soil.
        return (0, biomass, soil);
    }
    let (g, consumed) = growth(g0, elapsed, s, soil);
    (new_health, biomass.saturating_add(g), soil - consumed)
}

/// Accept-or-clamp a new weather sample against the previous accepted value.
#[inline]
pub fn clamp_weather(prev_bps: u16, proposed_bps: u16) -> u16 {
    let lo = prev_bps.saturating_sub(MAX_WEATHER_DELTA_BPS);
    let hi = prev_bps
        .saturating_add(MAX_WEATHER_DELTA_BPS)
        .min(BPS as u16);
    proposed_bps.clamp(lo, hi)
}

/// Median of the `n` most recent samples in a ring buffer (n <= 64).
/// Zero-valued never-written slots are excluded via the `len` of valid data.
pub fn median(samples: &[u16]) -> u16 {
    if samples.is_empty() {
        return (BPS / 2) as u16; // neutral weather if no data
    }
    let mut v: [u16; 64] = [0; 64];
    let n = samples.len().min(64);
    v[..n].copy_from_slice(&samples[..n]);
    let slice = &mut v[..n];
    slice.sort_unstable();
    if n % 2 == 1 {
        slice[n / 2]
    } else {
        ((slice[n / 2 - 1] as u32 + slice[n / 2] as u32) / 2) as u16
    }
}

/// Compost split for a dead plant's biomass.
/// Returns (to_pool, to_local_soil, bounty). Sums exactly to biomass.
pub fn compost_split(biomass: u64, pool_bps: u16, bounty_bps: u16) -> (u64, u64, u64) {
    let to_pool = biomass * pool_bps as u64 / BPS;
    let bounty = biomass * bounty_bps as u64 / BPS;
    let to_soil = biomass - to_pool - bounty; // remainder -> soil, exact conservation
    (to_pool, to_soil, bounty)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stress_multiplier_anchor_points() {
        assert_eq!(stress_multiplier_bps(0), 10_000);
        assert_eq!(stress_multiplier_bps(2_500), 20_000);
        assert_eq!(stress_multiplier_bps(5_000), 50_000);
        assert_eq!(stress_multiplier_bps(10_000), 170_000);
    }

    #[test]
    fn decay_matches_design_doc_example() {
        // base_rate=8, storm W=9000 vs optimal=5000 -> stress 4000, m=3.56
        // decay per 1000 slots ~= 28.48 -> floor 28
        let d = decay(8, 1_000, 4_000);
        assert_eq!(d, 28);
        // ~3.9h of storm (35,000 slots) kills a 1000-health plant — but
        // clamp caps a single evaluation at 400.
        let d = decay(8, 35_000, 4_000);
        assert_eq!(d, MAX_DECAY_PER_EVAL);
    }

    #[test]
    fn flash_kill_impossible_in_one_eval() {
        // Even absurd elapsed/stress can't exceed the clamp.
        assert_eq!(decay(u32::MAX, u64::MAX, 10_000), MAX_DECAY_PER_EVAL);
        // Therefore a full-health plant survives any single evaluation.
        let (h, _, _) = evaluate(MAX_HEALTH, 0, 0, u32::MAX, 0, u64::MAX, 10_000, 0);
        assert!(h >= MAX_HEALTH - MAX_DECAY_PER_EVAL as u16);
        assert!(h > 0);
    }

    #[test]
    fn zero_elapsed_is_noop() {
        let (h, b, s) = evaluate(800, 50, 1_000, 8, 5, 0, 9_000, 5_000);
        assert_eq!((h, b, s), (800, 50, 1_000));
    }

    #[test]
    fn growth_consumes_soil_exactly_conservation() {
        let soil = 100u64;
        let (h, b, s) = evaluate(1_000, 0, soil, 0, 5, 10_000, 5_000, 5_000);
        assert_eq!(h, 1_000);
        assert_eq!(b + s, soil); // biomass + remaining soil == initial soil
        assert_eq!(b, 50); // perfect comfort: 5/1000slots * 10000 slots = 50
    }

    #[test]
    fn growth_limited_by_soil() {
        let (_, b, s) = evaluate(1_000, 0, 10, 0, 5, 1_000_000, 5_000, 5_000);
        assert_eq!(b, 10);
        assert_eq!(s, 0);
    }

    #[test]
    fn no_growth_outside_tolerance_band() {
        assert_eq!(comfort_bps(5_000), 0);
        assert_eq!(comfort_bps(9_999), 0);
        let (_, b, s) = evaluate(1_000, 0, 1_000, 0, 5, 50_000, 10_000, 5_000);
        assert_eq!(b, 0);
        assert_eq!(s, 1_000);
    }

    #[test]
    fn dead_plants_dont_grow_or_eat() {
        let (h, b, s) = evaluate(1, 7, 500, 1_000, 5, 1_000_000, 10_000, 0);
        assert_eq!(h, 0);
        assert_eq!(b, 7);
        assert_eq!(s, 500);
    }

    #[test]
    fn weather_clamp_bounds_movement() {
        assert_eq!(clamp_weather(5_000, 9_000), 6_500);
        assert_eq!(clamp_weather(5_000, 1_000), 3_500);
        assert_eq!(clamp_weather(5_000, 5_100), 5_100);
        assert_eq!(clamp_weather(9_500, 10_000), 10_000); // capped at BPS
        assert_eq!(clamp_weather(500, 0), 0);
    }

    #[test]
    fn median_odd_even_empty() {
        assert_eq!(median(&[1, 9, 5]), 5);
        assert_eq!(median(&[1, 9, 5, 7]), 6);
        assert_eq!(median(&[]), 5_000);
        // Single dishonest crank in a window moves nothing.
        assert_eq!(median(&[5_000, 5_000, 5_000, 5_000, 10_000]), 5_000);
    }

    #[test]
    fn compost_split_conserves_exactly() {
        for biomass in [0u64, 1, 3, 999, 1_000_000_007] {
            let (p, soil, bounty) = compost_split(biomass, 7_000, 100);
            assert_eq!(p + soil + bounty, biomass);
        }
    }

    /// Fuzz-ish invariant sweep: conservation holds across a parameter grid.
    #[test]
    fn evaluate_never_creates_or_destroys_nutrients() {
        for &soil in &[0u64, 1, 100, 1_000_000] {
            for &elapsed in &[0u64, 1, 999, 1_000, 86_400 * 2] {
                for &w in &[0u16, 2_500, 5_000, 7_500, 10_000] {
                    let (_, b, s) = evaluate(1_000, 0, soil, 8, 5, elapsed, w, 5_000);
                    assert_eq!(b + s, soil, "leak at soil={soil} elapsed={elapsed} w={w}");
                }
            }
        }
    }
}
