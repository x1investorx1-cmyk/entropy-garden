import * as anchor from "@coral-xyz/anchor";
import { PublicKey, SYSVAR_SLOT_HASHES_PUBKEY } from "@solana/web3.js";
import { startAnchor } from "solana-bankrun";
import { BankrunProvider } from "anchor-bankrun";
import { assert } from "chai";
import IDL from "../target/idl/entropy_garden.json";

const GENESIS = 1_000_000_000n;

describe("death and compost (time-warped)", () => {
  it("plant dies, composts, conservation holds exactly", async () => {
    const context = await startAnchor("", [], []);
    const provider = new BankrunProvider(context);
    anchor.setProvider(provider);
    const program = new anchor.Program(IDL as any, provider);
    const me = provider.wallet.publicKey;

    const pid = program.programId;
    const [pool] = PublicKey.findProgramAddressSync([Buffer.from("compost")], pid);
    const rid = Buffer.alloc(2); rid.writeUInt16LE(0);
    const [region] = PublicKey.findProgramAddressSync([Buffer.from("region"), rid], pid);
    const [feed] = PublicKey.findProgramAddressSync([Buffer.from("weather"), rid], pid);
    const idx = Buffer.alloc(4); idx.writeUInt32LE(0);
    const [plot] = PublicKey.findProgramAddressSync([Buffer.from("plot"), me.toBuffer(), idx], pid);
    const [plant] = PublicKey.findProgramAddressSync([Buffer.from("plant"), plot.toBuffer(), Buffer.from([0])], pid);

    // genesis
    await program.methods.initializeGarden(new anchor.BN(GENESIS.toString()))
      .accounts({ authority: me }).rpc();
    await program.methods.createRegion(0, { feeMarket: {} })
      .accounts({ authority: me }).rpc();
    await program.methods.claimPlot().accounts({ owner: me, region }).rpc();
    await program.methods.plantSeed(0, 1)
      .accounts({ owner: me, plot, slotHashes: SYSVAR_SLOT_HASHES_PUBKEY }).rpc();

    const conserved = async (expectPlant: boolean) => {
      const p: any = await program.account.nutrientPool.fetch(pool);
      const pl: any = await program.account.plot.fetch(plot);
      let sum = BigInt(p.balance.toString()) + BigInt(pl.soilNutrients.toString());
      if (expectPlant) {
        const pt: any = await program.account.plant.fetch(plant);
        sum += BigInt(pt.biomass.toString());
      }
      assert.equal(sum.toString(), GENESIS.toString(), "CONSERVATION VIOLATED");
    };
    await conserved(true);

    // neglect across deep time: warp + tend until dead.
    // each eval: clamped -400 decay, +150 care bonus while alive => 4 cycles to die
    const clock = await context.banksClient.getClock();
    let slot = clock.slot;
    for (let i = 0; i < 4; i++) {
      slot += 60_000n;                       // ~4.5h per warp at 2.5 slots/s
      context.warpToSlot(slot);
      try {
        await program.methods.tend()
          .accounts({ owner: me, plot, plant, feed, region }).rpc();
      } catch (e: any) {
        assert.include(e.toString(), "PlantDead", `unexpected on cycle ${i}: ${e}`);
        break;                                // died on a previous cycle — fine
      }
    }
    const dead: any = await program.account.plant.fetch(plant);
    assert.equal(dead.health, 0, "plant should be dead after 4 neglect cycles");
    await conserved(true);

    // compost: biomass splits pool/soil/bounty, plant account closes
    const poolBefore: any = await program.account.nutrientPool.fetch(pool);
    await program.methods.compost()
      .accounts({ composter: me, plot,  plant, feed, region })
      .rpc();
    const poolAfter: any = await program.account.nutrientPool.fetch(pool);
    assert.isTrue(poolAfter.balance.gt(poolBefore.balance), "pool should grow from compost");
    await conserved(false);                   // plant gone; sum still exact

    const plotAfter: any = await program.account.plot.fetch(plot);
    assert.isNull(plotAfter.plants[0], "plot slot should be cleared");
    console.log("    ☠️→🌱 full death/compost cycle: conservation exact to the unit");
  });
});
