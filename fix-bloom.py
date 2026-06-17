p = 'programs/entropy-garden/src/eg/bloom.rs'
s = open(p).read()

# FIX 1: RoundPhase needs Space impl. Simplest fix: store as u8 instead of enum.
# Replace the enum with a u8 constant pattern — no Space impl needed.
s = s.replace(
    '''#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq)]
pub enum RoundPhase { Commit, Growing, Resolved }

impl Default for RoundPhase { fn default() -> Self { RoundPhase::Commit } }''',
    '''// Phase stored as u8: 0=Commit, 1=Growing, 2=Resolved
pub const PHASE_COMMIT:   u8 = 0;
pub const PHASE_GROWING:  u8 = 1;
pub const PHASE_RESOLVED: u8 = 2;''')

# Update BloomRound to use u8 for phase
s = s.replace('    pub phase:          RoundPhase,', '    pub phase:          u8,')

# Update all RoundPhase references to u8 constants
s = s.replace('r.phase          = RoundPhase::Commit;', 'r.phase          = PHASE_COMMIT;')
s = s.replace('r.phase    = RoundPhase::Resolved;', 'r.phase    = PHASE_RESOLVED;')
s = s.replace('require!(r.phase == RoundPhase::Commit, GardenError::BadForecastWindow);',
              'require!(r.phase == PHASE_COMMIT, GardenError::BadForecastWindow);')
# advance phase in stake_bloom
s = s.replace('if now >= r.commit_end_slot { r.phase = RoundPhase::Growing; }',
              'if now >= r.commit_end_slot { r.phase = PHASE_GROWING; }')

# FIX 2: GardenError::FeeCap doesn't exist on server — use BadForecastWindow for stake bounds
# (or NotEnoughSoil — either reads fine for "invalid amount". BadForecastWindow is already used.)
s = s.replace('GardenError::FeeCap', 'GardenError::BadForecastWindow')

open(p,'w').write(s)
print("phase enum → u8:", "RoundPhase" not in s)
print("FeeCap removed:", "FeeCap" not in s)
print("PHASE_COMMIT:", "PHASE_COMMIT" in s)
print("phase as u8:", "pub phase:          u8," in s)
