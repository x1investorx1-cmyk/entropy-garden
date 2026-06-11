# Entropy Garden 🌱⛓️

A decay-driven, chain-reactive garden world on **X1** (SVM / Solana fork).
Plants live, grow, and die in real time — and the weather is the chain itself:
live X1 traffic (fees, TPS, activity by category) drives storms, rain, and
drought through a permissionless oracle crank.

## Core invariants (the constitution)

1. **Conservation law** — nutrients are never minted or burned after genesis:
   `pool.balance + Σ plot.soil + Σ plant.biomass == genesis_total`, asserted
   in every integration test.
2. **No flash-kill** — one lazy evaluation can never remove >40% of max
   health (`MAX_DECAY_PER_EVAL`), and weather samples are clamped to ±1500
   bps movement and median-filtered. Fuzz-tested in `math/decay.rs`.
3. **Lazy simulation** — nothing ticks globally; state is evaluated on access
   from elapsed slots + the weather window. Zero cranks for the simulation
   itself; cranks only feed weather.

## Layout

```
programs/entropy-garden/src/
  math/decay.rs      # pure integer math — consensus-critical, fully unit tested
  state/mod.rs       # GardenConfig, NutrientPool, Region, WeatherFeed, Plot, Plant
  instructions/mod.rs# initialize, create_region, update_weather, claim_plot,
                     # plant_seed, tend, compost, draw_nutrients, admin
tests/entropy-garden.ts  # lifecycle + conservation assertions (ts-mocha)
crank/index.ts           # weather oracle daemon (fee-market + TPS channels)
.github/workflows/ci.yml # cargo test (math) + anchor build/test
```

## Quick start

```bash
# prerequisites: Rust, Solana CLI (Agave), Anchor 0.31.x, Node 20, yarn
yarn install
cargo test -p entropy-garden --lib   # decay math suite — run this first
anchor build                          # generates the real program keypair
anchor keys sync                      # writes real program ID into lib.rs/Anchor.toml
anchor test                           # local validator integration tests
```

> `declare_id!` ships with a placeholder. `anchor keys sync` after first
> build replaces it with your real program ID everywhere.

## Deploy to X1

```bash
# testnet (verify current RPC at docs.x1.xyz; fund deployer from the faucet)
solana config set --url https://rpc.testnet.x1.xyz --keypair ~/.config/solana/x1-deployer.json
anchor deploy --provider.cluster https://rpc.testnet.x1.xyz
anchor idl init <PROGRAM_ID> -f target/idl/entropy_garden.json

# bootstrap
# initialize_garden(1_000_000_000), create_region(0, FeeMarket), create_region(1, Transfers)

# crank (one per machine; multiple operators recommended — median filtering)
ANCHOR_PROVIDER_URL=https://rpc.testnet.x1.xyz \
ANCHOR_WALLET=~/.config/solana/crank.json yarn crank
```

Mainnet: same commands against the mainnet RPC, **after** the pre-flight
checklist in `entropy-garden-launch-playbook.md` (multisig upgrade authority,
pause tested, 72h crank soak, plot cap at 500).

## v0.1 scope and known limitations (deliberate)

- **SEED token, harvest, crossbreed, export-to-NFT: not yet implemented.**
  The nutrient layer is the foundation and ships first; the SPL emission
  layer (epoch budgets, pro-rata harvest) is v0.2. The tokenomics doc
  specifies it fully.
- **`draw_nutrients` uses simplified share accounting** (entitlement minus
  debt, 10%-per-draw cap). v0.2 moves to a MasterChef-style
  `acc_reward_per_share` accumulator (field already reserved).
- **Genome entropy from SlotHashes is leader-influenceable.** Fine for
  base traits; rare traits need commit-reveal before they carry value.
- **Two weather channels live** (fee market, TPS) — the ones derivable from
  vanilla RPC. DeFi-swap and NFT-mint channels need a Geyser/indexer feed.
- **Slot-warp tests** (storm death, compost flow, cooldown expiry) need
  bankrun or `--slots-per-epoch` warping; scaffolded in test comments.
- `update_weather` rewards pay from the config PDA's lamport balance —
  fund it, or cranks run unrewarded (they still work).

## Security posture

Beta runs **upgradeable behind a multisig with a public freeze date**, a
`paused` switch that halts everything except compost/withdrawals, and a plot
cap. Conservation is monitored hourly off-chain. Found an inflation leak?
Bounty: first legendary genome at v0.2 + XN. Open an issue or contact the
maintainer privately for critical findings.

## License

MIT
