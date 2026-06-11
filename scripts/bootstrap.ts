import * as anchor from "@coral-xyz/anchor";

const PROGRAM_ID = new anchor.web3.PublicKey(
  "8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");

async function main() {
  const provider = anchor.AnchorProvider.env();
  anchor.setProvider(provider);
  const idl = await anchor.Program.fetchIdl(PROGRAM_ID, provider);
  if (!idl) throw new Error("IDL not on-chain yet — run anchor idl init first");
  const program = new anchor.Program(idl, provider);

  console.log("genesis: initialize_garden(1,000,000,000 nutrients)...");
  await program.methods.initializeGarden(new anchor.BN(1_000_000_000))
    .accounts({ authority: provider.wallet.publicKey }).rpc();

  console.log("creating region 0 (FeeMarket weather)...");
  await program.methods.createRegion(0, { feeMarket: {} })
    .accounts({ authority: provider.wallet.publicKey }).rpc();

  console.log("creating region 1 (Transfers weather)...");
  await program.methods.createRegion(1, { transfers: {} })
    .accounts({ authority: provider.wallet.publicKey }).rpc();

  console.log("🌱 The garden is live. Nutrients conserved from this moment on.");
}
main().catch(e => { console.error(e); process.exit(1); });
