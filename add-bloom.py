# Idempotent patch: wire bloom.rs into the program.
import re

eg_mod = 'programs/entropy-garden/src/eg/mod.rs'
lib    = 'programs/entropy-garden/src/lib.rs'

m = open(eg_mod).read()
if 'pub mod bloom;' not in m:
    m = m.rstrip() + '\npub mod bloom;\n'
    open(eg_mod,'w').write(m); print("mod.rs: bloom declared")
else: print("mod.rs: already has bloom")

l = open(lib).read()
if 'use eg::bloom::*;' not in l:
    l = l.replace('use eg::carbon::*;', 'use eg::carbon::*;\nuse eg::bloom::*;')
if 'pub fn open_round' not in l:
    anchor = '    pub fn enter_maze('
    l = l.replace(anchor, '''    pub fn open_round(ctx: Context<OpenRound>, round_id: u64) -> Result<()> {
        eg::bloom::open_round(ctx, round_id)
    }
    pub fn stake_bloom(ctx: Context<StakeBloom>, round_id: u64, bloom_id: u8, amount: u64) -> Result<()> {
        eg::bloom::stake_bloom(ctx, round_id, bloom_id, amount)
    }
    pub fn resolve_round(ctx: Context<ResolveRound>, round_id: u64) -> Result<()> {
        eg::bloom::resolve_round(ctx, round_id)
    }
    pub fn claim_winnings(ctx: Context<ClaimWinnings>, round_id: u64) -> Result<()> {
        eg::bloom::claim_winnings(ctx, round_id)
    }
    pub fn close_losing_stake(ctx: Context<CloseLosingStake>, round_id: u64) -> Result<()> {
        eg::bloom::close_losing_stake(ctx, round_id)
    }

''' + anchor)
    open(lib,'w').write(l); print("lib.rs: bloom router added")
else: print("lib.rs: already has bloom router")

l2 = open(lib).read()
print("use stmt:", "use eg::bloom::*;" in l2)
print("open_round:", "pub fn open_round" in l2)
print("stake_bloom:", "pub fn stake_bloom" in l2)
print("resolve_round:", "pub fn resolve_round" in l2)
