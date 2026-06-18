# Idempotent patch: wire airdrop.rs into the program.
eg_mod='programs/entropy-garden/src/eg/mod.rs'
lib='programs/entropy-garden/src/lib.rs'

m=open(eg_mod).read()
if 'pub mod airdrop;' not in m:
    m=m.rstrip()+'\npub mod airdrop;\n'
    open(eg_mod,'w').write(m); print("mod.rs: airdrop declared")
else: print("mod.rs: already has airdrop")

l=open(lib).read()
if 'use eg::airdrop::*;' not in l:
    l=l.replace('use eg::bloom::*;','use eg::bloom::*;\nuse eg::airdrop::*;')
if 'pub fn init_airdrop' not in l:
    anchor='    pub fn open_round('
    l=l.replace(anchor,'''    pub fn init_airdrop(ctx: Context<InitAirdrop>, merkle_root: [u8; 32], amount_per_claim: u64) -> Result<()> {
        eg::airdrop::init_airdrop(ctx, merkle_root, amount_per_claim)
    }
    pub fn claim_airdrop(ctx: Context<ClaimAirdrop>, proof: Vec<[u8; 32]>) -> Result<()> {
        eg::airdrop::claim_airdrop(ctx, proof)
    }

''' + anchor)
    open(lib,'w').write(l); print("lib.rs: airdrop router added")
else: print("lib.rs: already has airdrop router")

l2=open(lib).read()
print("use stmt:", "use eg::airdrop::*;" in l2)
print("init_airdrop:", "pub fn init_airdrop" in l2)
print("claim_airdrop:", "pub fn claim_airdrop" in l2)
