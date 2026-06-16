# TESTNET ONLY: small divisor+threshold so harvest mints visible EG fast.
# RESTORE to 100000/100000 before mainnet (use restore-carbon-mainnet.py).
p='programs/entropy-garden/src/eg/carbon.rs'
s=open(p).read(); import re
s=re.sub(r'pub const HARVEST_DIV: u64 = \d[\d_]*;','pub const HARVEST_DIV: u64 = 1_000;',s)
s=re.sub(r'pub const MIN_HARVEST_MASS: u64 = \d[\d_]*;','pub const MIN_HARVEST_MASS: u64 = 1_000;',s)
open(p,'w').write(s)
print("TESTNET: HARVEST_DIV=1000, MIN_HARVEST_MASS=1000 (verify nonzero reward)")
