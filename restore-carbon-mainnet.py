# Restore carbon constants for MAINNET.
p='programs/entropy-garden/src/eg/carbon.rs'
s=open(p).read(); import re
s=re.sub(r'pub const HARVEST_DIV: u64 = \d[\d_]*;','pub const HARVEST_DIV: u64 = 100_000;',s)
s=re.sub(r'pub const MIN_HARVEST_MASS: u64 = \d[\d_]*;','pub const MIN_HARVEST_MASS: u64 = 100_000;',s)
open(p,'w').write(s)
print("MAINNET: HARVEST_DIV=100000, MIN_HARVEST_MASS=100000")
print("(min harvest = 1 EG, a day ≈ 10 EG leaf / 20 balanced)")
