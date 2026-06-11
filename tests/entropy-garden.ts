import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { PublicKey, SYSVAR_SLOT_HASHES_PUBKEY } from "@solana/web3.js";
import { assert } from "chai";
import { EntropyGarden } from "../target/types/entropy_garden";

const GENESIS = new anchor.BN(1_000_000_000);

describe("entropy-garden lifecycle", () => {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const program = anchor.workspace.EntropyGarden as Program<EntropyGarden>;
  const me = provider.wallet.publicKey;

  const [config] = PublicKey.findProgramAddressSync(
    [Buffer.from("config")], program.programId);
  const [pool] = PublicKey.findProgramAddressSync(
    [Buffer.from("compost")], program.programId);
  const regionId = 0;
  const ridBuf = Buffer.alloc(2); ridBuf.writeUInt16LE(regionId);
  const [region] = PublicKey.findProgramAddressSync(
    [Buffer.from("region"), ridBuf], program.programId);
  const [feed] = PublicKey.findProgramAddressSync(
    [Buffer.from("weather"), ridBuf], program.programId);

  let plot: PublicKey;
  let plant: PublicKey;

  /** THE invariant: pool + Σ soil + Σ biomass === genesis_total */
  async function assertConservation() {
    const p = await program.account.nutrientPool.fetch(pool);
    let sum = BigInt(p.balance.toString());
    for (const { account } of await program.account.plot.all()) {
      sum += BigInt(account.soilNutrients.toString());
    }
    for (const { account } of await program.account.plant.all()) {
      sum += BigInt(account.biomass.toString());
    }
    assert.equal(sum.toString(), p.genesisTotal.toString(),
      "CONSERVATION LAW VIOLATED");
  }

  it("initializes garden + pool", async () => {
    await program.methods.initializeGarden(GENESIS)
      .accounts({ authority: me }).rpc();
    const c = await program.account.gardenConfig.fetch(config);
    assert.equal(c.totalPlots, 0);
    await assertConservation();
  });

  it("creates a region with a weather feed", async () => {
    await program.methods.createRegion(regionId, { defiSwaps: {} })
      .accounts({ authority: me }).rpc();
    const r = await program.account.region.fetch(region);
    assert.equal(r.currentWeatherBps, 5000);
  });

  it("accepts a clamped weather sample from a crank", async () => {
    const slot = await provider.connection.getSlot();
    // Propose an extreme storm; clamp should cap movement at +1500.
    await program.methods.updateWeather(10000, new anchor.BN(slot))
      .accounts({ crank: me, region, feed,
        slotHashes: SYSVAR_SLOT_HASHES_PUBKEY }).rpc();
    const r = await program.account.region.fetch(region);
    assert.equal(r.currentWeatherBps, 6500, "clamp failed");
  });

  it("claims a plot (soil flows pool -> plot)", async () => {
    const idx = Buffer.alloc(4); idx.writeUInt32LE(0);
    [plot] = PublicKey.findProgramAddressSync(
      [Buffer.from("plot"), me.toBuffer(), idx], program.programId);
    await program.methods.claimPlot()
      .accounts({ owner: me, region }).rpc();
    const pl = await program.account.plot.fetch(plot);
    assert.isTrue(pl.soilNutrients.gtn(0));
    await assertConservation();
  });

  it("plants a seed (soil -> biomass, genome derived)", async () => {
    [plant] = PublicKey.findProgramAddressSync(
      [Buffer.from("plant"), plot.toBuffer(), Buffer.from([0])],
      program.programId);
    await program.methods.plantSeed(0, 1)
      .accounts({ owner: me, plot,
        slotHashes: SYSVAR_SLOT_HASHES_PUBKEY }).rpc();
    const p = await program.account.plant.fetch(plant);
    assert.equal(p.health, 1000);
    assert.equal(p.biomass.toNumber(), 10);
    assert.isTrue(p.optimalBps >= 1000 && p.optimalBps <= 9000);
    await assertConservation();
  });

  it("rejects tend during cooldown", async () => {
    try {
      await program.methods.tend()
        .accounts({ owner: me, plot, plant, feed, region }).rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.toString(), "TendCooldown");
    }
  });

  it("rejects compost of a living plant", async () => {
    try {
      await program.methods.compost()
        .accounts({ composter: me, plot, bountyPlot: plot, owner: me,
          plant, feed, region }).rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      console.error("FULL ERROR >>>", e.toString()); assert.include(e.toString(), "PlantAlive");
    }
  });

  it("pause blocks gameplay", async () => {
    await program.methods.setPaused(true).accounts({ authority: me }).rpc();
    try {
      await program.methods.claimPlot().accounts({ owner: me, region }).rpc();
      assert.fail("should have thrown");
    } catch (e: any) {
      assert.include(e.toString(), "Paused");
    }
    await program.methods.setPaused(false).accounts({ authority: me }).rpc();
    await assertConservation();
  });

  // NOTE: tend-after-cooldown, storm-death, and compost-flow tests need slot
  // advancement — run them with `solana-test-validator --slots-per-epoch 32`
  // and a warp helper, or under bankrun (see tests/bankrun/ TODO in README).
});
