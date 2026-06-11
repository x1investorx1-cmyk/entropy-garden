/**
 * Entropy Garden weather crank.
 *
 * Reads live X1 chain activity per weather channel, normalizes it to bps,
 * and posts one sample per region per interval. Permissionless: anyone can
 * run this; the program clamps + median-filters, and pays a small XN reward
 * per accepted sample.
 *
 * Run: ANCHOR_PROVIDER_URL=<x1-rpc> ANCHOR_WALLET=~/.config/solana/crank.json yarn crank
 */
import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SYSVAR_SLOT_HASHES_PUBKEY, Connection } from "@solana/web3.js";

const INTERVAL_MS = 15_000;          // ~one sample / 25+ slots on X1
const PROGRAM_ID = new PublicKey("Entg111111111111111111111111111111111111111");

// region_id -> channel sampler
const CHANNELS: Record<number, (c: Connection) => Promise<number>> = {
  0: sampleFeeMarket,   // drought index from prioritization fees
  1: sampleTps,         // raw transfer pressure -> rain
  // 2: DeFi swaps, 3: NFT mints — need an indexer/Geyser feed; start with
  // the two derivable from vanilla RPC and add indexer-backed channels later.
};

/** Recent prioritization fees -> 0..10000 bps (log-scaled drought index). */
async function sampleFeeMarket(conn: Connection): Promise<number> {
  const fees = await conn.getRecentPrioritizationFees();
  if (!fees.length) return 5000;
  const avg = fees.reduce((a, f) => a + f.prioritizationFee, 0) / fees.length;
  // 0 lamports -> 0 bps; 10k+ microlamports -> ~10000 bps. Tune on real data.
  return Math.min(10_000, Math.round(2_500 * Math.log10(1 + avg)));
}

/** Network TPS from recent performance samples -> bps against a ceiling. */
async function sampleTps(conn: Connection): Promise<number> {
  const perf = await conn.getRecentPerformanceSamples(4);
  if (!perf.length) return 5000;
  const tps = perf.reduce((a, s) => a + s.numTransactions / s.samplePeriodSecs, 0) / perf.length;
  const CEILING_TPS = 50_000; // X1 is high-throughput; calibrate live
  return Math.min(10_000, Math.round((tps / CEILING_TPS) * 10_000));
}

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = await anchor.Program.fetchIdl(PROGRAM_ID, provider);
  if (!idl) throw new Error("IDL not found on-chain — run `anchor idl init` first");
  const program = new anchor.Program(idl, provider);

  console.log(`crank up: ${provider.wallet.publicKey.toBase58()}`);

  setInterval(async () => {
    for (const [rid, sampler] of Object.entries(CHANNELS)) {
      try {
        const regionId = Number(rid);
        const bps = await sampler(provider.connection);
        const slot = await provider.connection.getSlot();
        const ridBuf = Buffer.alloc(2); ridBuf.writeUInt16LE(regionId);
        const [region] = PublicKey.findProgramAddressSync(
          [Buffer.from("region"), ridBuf], PROGRAM_ID);
        const [feed] = PublicKey.findProgramAddressSync(
          [Buffer.from("weather"), ridBuf], PROGRAM_ID);

        const sig = await program.methods
          .updateWeather(bps, new anchor.BN(slot))
          .accounts({
            crank: provider.wallet.publicKey,
            region, feed,
            slotHashes: SYSVAR_SLOT_HASHES_PUBKEY,
          })
          .rpc();
        console.log(`region ${regionId}: ${bps} bps @ slot ${slot} (${sig.slice(0, 8)}…)`);
      } catch (e: any) {
        // TooFrequent / clamp rejections are normal in multi-crank setups.
        const msg = e?.toString() ?? "";
        if (!msg.includes("TooFrequent")) console.error(`region ${rid}:`, msg.slice(0, 160));
      }
    }
  }, INTERVAL_MS);
}

main().catch(console.error);
