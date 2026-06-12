import * as anchor from "@coral-xyz/anchor";
const PROGRAM_ID = new anchor.web3.PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = await anchor.Program.fetchIdl(PROGRAM_ID, provider);
  const program: any = new anchor.Program(idl!, provider);

  const [config] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("config")], PROGRAM_ID);
  const [pool] = anchor.web3.PublicKey.findProgramAddressSync([Buffer.from("compost")], PROGRAM_ID);
  const c: any = await program.account.gardenConfig.fetch(config);
  const p: any = await program.account.nutrientPool.fetch(pool);

  console.log("=== ENTROPY GARDEN STATUS ===");
  console.log(`pool: ${p.balance.toString()} / ${p.genesisTotal.toString()} nutrients`);
  console.log(`plots: ${c.totalPlots}/${c.maxPlots}  paused: ${c.paused}`);
  for (const { account } of (await program.account.region.all()) as any[]) {
    console.log(`region ${account.regionId}: weather ${account.currentWeatherBps} bps, ${account.plotCount} plots`);
  }
  let soil = 0n, biomass = 0n;
  for (const { account } of (await program.account.plot.all()) as any[]) soil += BigInt(account.soilNutrients.toString());
  for (const { account } of (await program.account.plant.all()) as any[]) biomass += BigInt(account.biomass.toString());
  const sum = BigInt(p.balance.toString()) + soil + biomass;
  console.log(`conservation: ${sum.toString()} == ${p.genesisTotal.toString()} ${sum.toString() === p.genesisTotal.toString() ? "✅" : "🚨 VIOLATED"}`);
}
main().catch(console.error);
