# Adds initialize_token_metadata instruction to the Entropy Garden program.
# This does a CPI into Metaplex Token Metadata, signed by our eg_mint_auth PDA.
# One-time call, permissioned to the eg_config.authority (mainnet deployer).

lib_path = 'programs/entropy-garden/src/lib.rs'
eg_mod = 'programs/entropy-garden/src/eg/mod.rs'
cargo = 'programs/entropy-garden/Cargo.toml'

# 1. Add mpl-token-metadata dependency to Cargo.toml
c = open(cargo).read()
if 'mpl-token-metadata' not in c:
    c = c.replace(
        '[dependencies]',
        '[dependencies]\nmpl-token-metadata = { version = "4", features = ["cpi"] }')
    open(cargo,'w').write(c)
    print("Cargo.toml: mpl-token-metadata added")
else:
    print("Cargo.toml: already has mpl-token-metadata")

# 2. Add the metadata module to eg/mod.rs
m = open(eg_mod).read()
if 'pub mod metadata;' not in m:
    m = m.rstrip() + '\npub mod metadata;\n'
    open(eg_mod,'w').write(m)
    print("mod.rs: metadata module declared")
else:
    print("mod.rs: already declared")

# 3. Add router entry to lib.rs
l = open(lib_path).read()
if 'use eg::metadata::*;' not in l:
    l = l.replace('use eg::thread::*;', 'use eg::thread::*;\nuse eg::metadata::*;')
if 'pub fn initialize_token_metadata' not in l:
    l = l.replace(
        '    pub fn enter_maze(',
        '''    pub fn initialize_token_metadata(ctx: Context<InitializeTokenMetadata>, name: String, symbol: String, uri: String) -> Result<()> {
        eg::metadata::initialize_token_metadata(ctx, name, symbol, uri)
    }

    pub fn enter_maze(''')
    open(lib_path,'w').write(l)
    print("lib.rs: router entry added")
else:
    print("lib.rs: already has router entry")

print("\nNow write programs/entropy-garden/src/eg/metadata.rs manually.")
