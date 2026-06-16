# TEMPORARY testnet tweak: lower MIN_HARVEST_MASS so we can test the harvest
# path with the small accumulated mass. RESTORE to 50000 before mainnet.
p = 'programs/entropy-garden/src/eg/carbon.rs'
s = open(p).read()
import re
m = re.search(r'pub const MIN_HARVEST_MASS: u64 = (\d[\d_]*);', s)
print("current MIN_HARVEST_MASS:", m.group(1) if m else "?")
s = re.sub(r'pub const MIN_HARVEST_MASS: u64 = \d[\d_]*;',
           'pub const MIN_HARVEST_MASS: u64 = 1_000;', s)
open(p,'w').write(s)
print("set to 1_000 for testnet harvest test (REMEMBER to restore to 50_000)")
