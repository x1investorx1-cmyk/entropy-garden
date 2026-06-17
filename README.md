# Entropy Garden 🌹

**A living experiment on X1 · Est. Genesis, June 2026**

A garden whose weather is the blockchain itself. Every storm on this page is real traffic on X1, right now — fee pressure becomes drought, transaction flow becomes rain. Plants live and die by it. Nothing here is simulated.

**entropygarden.xyz** · Program: `8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By` · Token: `EkaYAgWf6mpDCiqcDP9cMbcvKj5MHxCSutPbdSXJcaQ2`

---

## How to play

1. **Claim a plot** in the Dry Field or the Rainline — each region reads a different live channel of X1.
2. **Plant a seed.** Its genome sets the sky it loves; it blooms as a unique red rose.
3. **Tend it through real weather.** Keep it alive as the chain storms and calms.
4. **Harvest at full bloom** — or compost to return nutrients to the shared soil.

## How to earn EG

| Quest | Description | Earns |
|---|---|---|
| 🌱 **Garden** | Plant, tend, harvest. Streaks multiply your reward. | ~20 EG/harvest |
| 🌥️ **Sky-Reading** | Forecast storm or calm. Read it right for bonus EG. Pure skill. | 3 EG correct |
| 🌩️ **Storm-Chaser** | Plants that survive harsh weather pay a harvest bonus. | up to ×3 harvest |
| 🧵 **Ariadne's Thread** | Trace the labyrinth to the rose at its heart. Hard, but it pays — and crowns you a permanent trophy. | ~61 EG perfect run |
| 🌿 **Carbon Farming** | Open a carbon sink for any plant. Roots grow from fee pressure, leaves from throughput. Passive — it grows while you sleep. | ~34 EG/harvest cycle |

## Tokenomics

- **Total supply:** 1,000,000,000 EG (fixed)
- **90% mined by play** — no presale, no airdrop to insiders. Everybody grows in.
- **5% Treasury** — locked, accumulates XNT fees from all garden actions
- **4% Community** — locked, for future airdrop to early gardeners
- **1% Dev** — earmarked for the first liquidity pool

EG is minted at the moment of each action. Era decay (×0.75 per 30-day era) keeps early mining meaningful. Genesis bonus (×1.5) active now.

## Architecture

Built on **X1 Chain** (Solana fork). The garden's weather is sampled by a permissionless crank every ~15 seconds:

- **Region 0 — The Dry Field:** fee pressure → drought index (bps)
- **Region 1 — The Rainline:** transaction throughput → rainfall (bps)

Both feeds flow into an on-chain ring buffer (64 samples). Plant growth, quest resolution, and carbon farming all read from these live feeds. Nothing is simulated or admin-controlled.

### On-chain accounts

| Account | Address |
|---|---|
| Program | `8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By` |
| EG Mint | `EkaYAgWf6mpDCiqcDP9cMbcvKj5MHxCSutPbdSXJcaQ2` |
| EG Config | `8pMeHLSU3XSTipyeRMcvuD4m5Yav2VrjMtLT5t53poFj` |
| Treasury | `7WrZ9gKMygVBDNi2nk3ibZcc7RS4CeaGQnKb4tYy4hbZ` |
| Region 0 | `F5bTy6FQbUxN8S9MciG9vEcatFZqAPWAYobnUEd4vcqc` |
| Region 1 | `BSbYnXJ6YoKTjez3toMzioYFR44dUW54sV6kndsv8VFi` |

### Pages

| Page | Description |
|---|---|
| `garden.html` | Claim plots, plant seeds, tend and harvest |
| `rose.html` | Your plants — unique red roses, storm badges, carbon trophy |
| `skyread-mainnet.html` | Sky-Reading — forecast storm or calm |
| `ariadne.html` | Ariadne's Thread — the labyrinth quest |
| `carbon.html` | Carbon Farming — sequester chain activity as biomass |
| `map.html` | The living map — all plots, live weather |
| `tree.html` | The nutrient tree — conservation visualization |

## Design principles

- **Nobody buys in — everybody grows in.** 90% of EG is mined by play. No presale.
- **The chain's activity is the weather.** Every mechanic reads live X1 data. Nothing is simulated.
- **Conservation.** The nutrient pool is finite (1 billion units). Composting returns nutrients. The soil is shared.
- **Fair reward mechanics.** Every quest is tuned so random bots earn below the gardening baseline. Skill earns more.
- **Grows with the chain.** Carbon farming accumulates from ambient activity. The Pulse and staking are designed but banked until X1 has the volatility and scale to justify them.

## Tech stack

- **Program:** Anchor 0.31.1 / Solana 3.0.0 (Agave)
- **Crank:** TypeScript / ts-node, systemd supervised
- **Frontend:** Vanilla HTML/CSS/JS, solana-web3.js, Wallet Standard
- **Deploy:** Vercel (auto-deploy on git push), domain entropygarden.xyz
- **Server:** Hetzner Ubuntu 24

## Roadmap (banked, with honest triggers)

| Feature | Status | Trigger |
|---|---|---|
| Community airdrop (4% / 40M EG) | Designed | When holder distribution is healthy |
| Liquidity pool (~0.008 XNT/EG) | Designed | After airdrop, when concentration is healthy |
| The Pulse (TPS band prediction) | Built + banked | When chain tempo varies across bands |
| EG Staking (earn XNT yield) | Designed | When enough EG mined, enough holders |
| Scale farming (fields + seasons) | Sketched | When players hold many plots |

---

*The garden is a living experiment. It tends itself slowly, and honestly.*
