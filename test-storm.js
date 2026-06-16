// test-storm.js — TESTNET: calibrate Storm-Chaser. For several stress levels:
// claim a plot, plant, dev_grow(stress), storm-harvest, read EG gained.
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction,
  SystemProgram, SYSVAR_SLOT_HASHES_PUBKEY,
} = require("@solana/web3.js");
const fs = require("fs"), os = require("os");

const RPC = "https://rpc.testnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM   = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const conn = new Connection(RPC, "confirmed");
const secret = Uint8Array.from(JSON.parse(fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8")));
const W = Keypair.fromSecretKey(secret);

const te = s => new TextEncoder().encode(s);
const pda = seeds => PublicKey.findProgramAddressSync(seeds, PID)[0];
const CONFIG=pda([te("config")]), POOL=pda([te("compost")]);
const R0=pda([te("region"),new Uint8Array([0,0])]), F0=pda([te("weather"),new Uint8Array([0,0])]);
const EG_CONFIG=pda([te("eg_config")]), EG_MINT=pda([te("eg_mint")]);
const EG_AUTH=pda([te("eg_mint_auth")]), TREASURY=pda([te("eg_treasury")]);
const ataFor=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];

const D={claim:[125,132,62,56,128,210,150,69],plant:[139,66,41,202,41,145,173,204],
         grow:[239,82,60,45,34,198,87,175],harvest:[228,241,31,182,53,169,59,199]};
const EG=1_000_000_000n;
function u32(n){const b=Buffer.alloc(4);b.writeUInt32LE(n);return b;}
function u16(n){const b=Buffer.alloc(2);b.writeUInt16LE(n);return b;}
function u64(n){const b=Buffer.alloc(8);b.writeBigUInt64LE(BigInt(n));return b;}

async function send(keys,data,label){
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data}));
  tx.feePayer=W.publicKey; tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;
  tx.sign(W);
  const sig=await conn.sendRawTransaction(tx.serialize());
  await conn.confirmTransaction(sig,"confirmed");
  return sig;
}
async function bal(){const b=await conn.getTokenAccountBalance(ataFor(W.publicKey)).catch(()=>null);return b?BigInt(b.value.amount):0n;}
async function logsFor(sig){const t=await conn.getTransaction(sig,{maxSupportedTransactionVersion:0,commitment:"confirmed"});return t?.meta?.logMessages||[];}

async function oneRun(stress){
  // fresh plot each run
  const cfg=await conn.getAccountInfo(CONFIG);
  const total=cfg.data.readUInt32LE(82);
  const plot=pda([te("plot"),W.publicKey.toBytes(),u32(total)]);
  await send([
    {pubkey:W.publicKey,isSigner:true,isWritable:true},
    {pubkey:CONFIG,isSigner:false,isWritable:true},
    {pubkey:POOL,isSigner:false,isWritable:true},
    {pubkey:R0,isSigner:false,isWritable:true},
    {pubkey:plot,isSigner:false,isWritable:true},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ],Buffer.from(D.claim),"claim");

  const plant=pda([te("plant"),plot.toBytes(),new Uint8Array([0])]);
  await send([
    {pubkey:W.publicKey,isSigner:true,isWritable:true},
    {pubkey:CONFIG,isSigner:false,isWritable:false},
    {pubkey:plot,isSigner:false,isWritable:true},
    {pubkey:plant,isSigner:false,isWritable:true},
    {pubkey:SYSVAR_SLOT_HASHES_PUBKEY,isSigner:false,isWritable:false},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},
    {pubkey:EG_MINT,isSigner:false,isWritable:true},
    {pubkey:EG_AUTH,isSigner:false,isWritable:false},
    {pubkey:ataFor(W.publicKey),isSigner:false,isWritable:true},
    {pubkey:TREASURY,isSigner:false,isWritable:true},
    {pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ],Buffer.concat([Buffer.from(D.plant),Buffer.from([0]),u16(1)]),"plant");

  // dev_grow to stage 5 with the target stress
  await send([
    {pubkey:W.publicKey,isSigner:true,isWritable:false},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:false},
    {pubkey:plant,isSigner:false,isWritable:true},
  ],Buffer.concat([Buffer.from(D.grow),u64(stress)]),"dev_grow");

  // storm-harvest, capture EG delta + log
  const before=await bal();
  const sig=await send([
    {pubkey:W.publicKey,isSigner:true,isWritable:true},
    {pubkey:CONFIG,isSigner:false,isWritable:false},
    {pubkey:POOL,isSigner:false,isWritable:true},
    {pubkey:plot,isSigner:false,isWritable:true},
    {pubkey:plant,isSigner:false,isWritable:true},
    {pubkey:F0,isSigner:false,isWritable:false},
    {pubkey:R0,isSigner:false,isWritable:false},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},
    {pubkey:EG_MINT,isSigner:false,isWritable:true},
    {pubkey:EG_AUTH,isSigner:false,isWritable:false},
    {pubkey:ataFor(W.publicKey),isSigner:false,isWritable:true},
    {pubkey:TREASURY,isSigner:false,isWritable:true},
    {pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ],Buffer.from(D.harvest),"harvest");
  const after=await bal();
  const gained=Number(after-before)/1e9;
  const logs=await logsFor(sig);
  const stormLog=logs.find(l=>l.includes("storm-harvest"))||"(log not captured)";
  return {stress,gained,stormLog};
}

(async()=>{
  console.log("=== STORM-CHASER CALIBRATION (testnet) ===");
  console.log("wallet:",W.publicKey.toBase58());
  console.log("balance:",(await conn.getBalance(W.publicKey))/1e9,"XNT\n");
  console.log("Genesis 1.5x bonus is active, so base harvest = 20 * 1.5 = 30 EG\n");
  for(const stress of [0, 5000, 10000, 25000, 50000, 100000]){
    try{
      const r=await oneRun(stress);
      console.log(`stress ${String(stress).padStart(7)} -> ${r.gained.toFixed(2)} EG  | ${r.stormLog.replace(/^Program log: /,'')}`);
    }catch(e){console.log(`stress ${stress} FAILED: ${e.message||e}`); if(e.logs)console.log(e.logs.join("\n"));}
  }
  console.log("\nTune STORM_STRESS_DIVISOR so a 'real storm survivor' lands ~2x.");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
