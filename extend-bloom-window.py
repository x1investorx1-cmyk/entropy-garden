p='programs/entropy-garden/src/eg/bloom.rs'
s=open(p).read(); import re
s=re.sub(r'pub const COMMIT_SLOTS: u64\s*=\s*\d+;','pub const COMMIT_SLOTS: u64  = 1200;',s)
s=re.sub(r'pub const GROW_SLOTS: u64\s*=\s*\d+;','pub const GROW_SLOTS:   u64  = 600;',s)
open(p,'w').write(s)
import re as r2
cs=r2.search(r'COMMIT_SLOTS: u64\s*=\s*(\d+)',s).group(1)
gs=r2.search(r'GROW_SLOTS:\s*u64\s*=\s*(\d+)',s).group(1)
print(f"COMMIT_SLOTS={cs}, GROW_SLOTS={gs}")
