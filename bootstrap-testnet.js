// bootstrap-testnet.js — claim a plot + plant a seed on TESTNET so we have something to tend.
// Run: node bootstrap-testnet.js
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction,
  SystemProgram, SYSVAR_SLOT_HASHES_PUBKEY,
} = require("@solana/web3.js");
const fs = require("fs"), os = require("os");

const RPC = "https://rpc.testnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const conn = new Connection(RPC, "confirmed");

const secret = Uint8Array.from(JSON.parse(fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8")));
const wallet = Keypair.fromSecretKey(secret);

const te = s => new TextEncoder().encode(s);
const pda = seeds => PublicKey.findProgramAddressSync(seeds, PID)[0];
const CONFIG = pda([te("config")]);
const POOL   = pda([te("compost")]);
const REGION0= pda([te("region"), new Uint8Array([0,0])]);

const CLAIM_DISC = [125,132,62,56,128,210,150,69];
const PLANT_DISC = [139,66,41,202,41,145,173,204];

function u32le(n){const b=Buffer.alloc(4);b.writeUInt32LE(n);return b;}
function u16le(n){const b=Buffer.alloc(2);b.writeUInt16LE(n);return b;}

async function send(ix, label){
  const tx = new Transaction().add(ix);
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  tx.sign(wallet);
  const sig = await conn.sendRawTransaction(tx.serialize());
  await conn.confirmTransaction(sig, "confirmed");
  console.log(`✅ ${label}: ${sig.slice(0,24)}…`);
}

(async () => {
  console.log("wallet:", wallet.publicKey.toBase58());
  console.log("balance:", (await conn.getBalance(wallet.publicKey))/1e9, "XNT\n");

  // read config.total_plots (u32) to derive this claim's plot PDA
  const cfg = await conn.getAccountInfo(CONFIG);
  // GardenConfig layout: 8 disc + 32 authority + 2 regions + 4 decay + 4 growth + 2 + 2
  //  + 8 crank + 8 cooldown + 8 genesis_soil + 4 max + 4 total_plots ...
  // total_plots offset = 8+32+2+4+4+2+2+8+8+8+4 = 82
  const totalPlots = cfg.data.readUInt32LE(82);
  console.log("current total_plots:", totalPlots);

  const plot = pda([te("plot"), wallet.publicKey.toBytes(), u32le(totalPlots)]);
  console.log("plot to be claimed:", plot.toBase58());

  // ---- claim_plot in region 0 ----
  await send(new TransactionInstruction({
    programId: PID, data: Buffer.from(CLAIM_DISC),
    keys: [
      {pubkey: wallet.publicKey, isSigner:true, isWritable:true},
      {pubkey: CONFIG, isSigner:false, isWritable:true},
      {pubkey: POOL, isSigner:false, isWritable:true},
      {pubkey: REGION0, isSigner:false, isWritable:true},
      {pubkey: plot, isSigner:false, isWritable:true},
      {pubkey: SystemProgram.programId, isSigner:false, isWritable:false},
    ],
  }), "claimed plot");

  // ---- plant_seed in slot 0, species 1 ----
  const slotIndex = 0, species = 1;
  const plant = pda([te("plant"), plot.toBytes(), new Uint8Array([slotIndex])]);
  console.log("plant to be created:", plant.toBase58());
  await send(new TransactionInstruction({
    programId: PID,
    data: Buffer.concat([Buffer.from(PLANT_DISC), Buffer.from([slotIndex]), u16le(species)]),
    keys: [
      {pubkey: wallet.publicKey, isSigner:true, isWritable:true},
      {pubkey: CONFIG, isSigner:false, isWritable:false},
      {pubkey: plot, isSigner:false, isWritable:true},
      {pubkey: plant, isSigner:false, isWritable:true},
      {pubkey: SYSVAR_SLOT_HASHES_PUBKEY, isSigner:false, isWritable:false},
      {pubkey: SystemProgram.programId, isSigner:false, isWritable:false},
    ],
  }), "planted seed");

  console.log("\n=== READY TO TEST EG ===");
  console.log("Plot  :", plot.toBase58());
  console.log("Plant :", plant.toBase58());
  console.log("Region: 0");
  console.log("\nPaste these into egtest.html, wait ~5 min (tend cooldown), then tend.");
})().catch(e => { console.error("ERR:", e.message); process.exit(1); });
