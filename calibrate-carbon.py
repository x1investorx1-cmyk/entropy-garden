# Recalibrate carbon.rs: RATE_DIV 1M→1000 (granular mass), add HARVEST_DIV.
p = 'programs/entropy-garden/src/eg/carbon.rs'
s = open(p).read()

# 1. RATE_DIV: 1_000_000 → 1_000
if 'pub const RATE_DIV: u64 = 1_000_000;' in s:
    s = s.replace('pub const RATE_DIV: u64 = 1_000_000;',
                  'pub const RATE_DIV: u64 = 1_000;')
    print("RATE_DIV → 1000")
elif 'pub const RATE_DIV: u64 = 1_000;' in s:
    print("RATE_DIV already 1000")

# 2. add HARVEST_DIV constant after REWARD_CARBON_BASE
if 'pub const HARVEST_DIV' not in s:
    s = s.replace(
        'pub const REWARD_CARBON_BASE: u64 = 1;',
        'pub const REWARD_CARBON_BASE: u64 = 1;\n\n// Harvest payout divisor: brings accumulated mass down to ~gardening rate.\n// ~1 day leaf-only accumulation (≈1.03M mass) pays ~10 EG; balanced pays 2x.\npub const HARVEST_DIV: u64 = 100_000;')
    print("HARVEST_DIV added")
else:
    print("HARVEST_DIV already present")

# 3. update harvest formula to divide by HARVEST_DIV
old_formula = '''    let base = REWARD_CARBON_BASE
        .saturating_mul(total_mass)
        .saturating_mul(diversity_num) / 100;'''
new_formula = '''    let base = REWARD_CARBON_BASE
        .saturating_mul(total_mass)
        .saturating_mul(diversity_num) / (100u64.saturating_mul(HARVEST_DIV));'''
if old_formula in s:
    s = s.replace(old_formula, new_formula)
    print("harvest formula updated with HARVEST_DIV")
elif 'HARVEST_DIV))' in s:
    print("harvest formula already updated")

# 4. MIN_HARVEST_MASS: with granular mass, 100 is trivially small now.
# Raise it so harvests aren't dust. ~half a day of leaf = ~500k. Set 50000 (~1hr).
if 'pub const MIN_HARVEST_MASS: u64 = 100;' in s:
    s = s.replace('pub const MIN_HARVEST_MASS: u64 = 100;',
                  'pub const MIN_HARVEST_MASS: u64 = 50_000;')
    print("MIN_HARVEST_MASS → 50000 (~1hr accumulation)")

open(p,'w').write(s)
print("\ncalibration applied")
