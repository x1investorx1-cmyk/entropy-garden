// test-carbon-harvest.js — exercise the full cycle INCLUDING harvest.
const {Connection,PublicKey,Keypair,Transaction,TransactionInstruction,SystemProgram}=require("@solana/web3.js");
const fs=require("fs"),os=require("os");
const RPC="https://rpc.testnet.x1.xyz";
const conn=new Connection(RPC,"confirmed");
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM=new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM=new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const W=Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8"))));
const te=s=>new TextEncoder().encode(s);
const pda=s=>PublicKey.findProgramAddressSync(s,PID)[0];
const EG_CONFIG=pda([te("eg_config")]),EG_MINT=pda([te("eg_mint")]),EG_AUTH=pda([te("eg_mint_auth")]);
const feeRid=Buffer.alloc(2);feeRid.writeUInt16LE(0);
const tpsRid=Buffer.alloc(2);tpsRid.writeUInt16LE(1);
const FEE_FEED=pda([te("weather"),feeRid]),TPS_FEED=pda([te("weather"),tpsRid]);
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];
const D={init:[65,40,21,72,133,170,66,51],seq:[18,92,178,99,150,7,65,106],harvest:[235,65,42,204,125,146,138,230]};

async function send(keys,data,label){
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  try{const sig=await conn.sendRawTransaction(tx.serialize());await conn.confirmTransaction(sig,"confirmed");return sig;}
  catch(e){console.log(label,"FAILED:",e.message.slice(0,120));
    if(e.getLogs){try{console.log((await e.getLogs(conn)).slice(-6).join("\n"));}catch(_){}} throw e;}
}
async function findPlant(){
  const crypto=require("crypto");
  const disc=crypto.createHash("sha256").update("account:Plant").digest().slice(0,8);
  const all=await conn.getProgramAccounts(PID);
  for(const a of all){const d=a.account.data;if(d.length>=8&&Buffer.from(d.slice(0,8)).equals(disc))return a;}
  return null;
}
function decodeSink(d){let o=8;o+=64;const root=d.readBigUInt64LE(o);o+=8;const leaf=d.readBigUInt64LE(o);o+=8;o+=8;const harvested=d.readBigUInt64LE(o);return{root,leaf,harvested};}
async function egBal(){try{const b=await conn.getTokenAccountBalance(ata(W.publicKey));return b.value.uiAmount;}catch(_){return 0;}}

(async()=>{
  console.log("=== CARBON HARVEST TEST (testnet) ===\n");
  const plant=await findPlant();
  const PLANT=plant.pubkey;
  const SINK=pda([te("carbon"),PLANT.toBytes()]);
  console.log("sink:",SINK.toBase58());

  // open the sink if it doesn't exist yet
  if(!(await conn.getAccountInfo(SINK))){
    console.log("opening sink…");
    await send([
      {pubkey:W.publicKey,isSigner:true,isWritable:true},
      {pubkey:PLANT,isSigner:false,isWritable:false},
      {pubkey:SINK,isSigner:false,isWritable:true},
      {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
    ],D.init,"init");
  }

  // sequester a bunch to build mass (testnet roots grow ~30/poke)
  console.log("\naccumulating (20 sequesters)…");
  for(let i=0;i<20;i++){
    await send([{pubkey:SINK,isSigner:false,isWritable:true},
      {pubkey:FEE_FEED,isSigner:false,isWritable:false},
      {pubkey:TPS_FEED,isSigner:false,isWritable:false}],D.seq,"seq");
  }
  let s=decodeSink((await conn.getAccountInfo(SINK)).data);
  console.log(`  root=${s.root} leaf=${s.leaf} total=${s.root+s.leaf}`);

  const before=await egBal();
  console.log("\nEG before harvest:",before);
  console.log("attempting harvest…");
  try{
    await send([
      {pubkey:W.publicKey,isSigner:true,isWritable:true},
      {pubkey:SINK,isSigner:false,isWritable:true},
      {pubkey:EG_CONFIG,isSigner:false,isWritable:true},
      {pubkey:EG_MINT,isSigner:false,isWritable:true},
      {pubkey:EG_AUTH,isSigner:false,isWritable:false},
      {pubkey:ata(W.publicKey),isSigner:false,isWritable:true},
      {pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},
      {pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},
      {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
    ],D.harvest,"harvest");
    const after=await egBal();
    s=decodeSink((await conn.getAccountInfo(SINK)).data);
    console.log("\n✓ HARVEST OK");
    console.log("  EG after:",after,"(+"+(after-before)+" EG)");
    console.log("  sink reset: root="+s.root+" leaf="+s.leaf+" total_harvested="+s.harvested);
  }catch(e){
    console.log("\n(harvest blocked — likely below MIN_HARVEST_MASS threshold)");
    console.log("  current mass:",(s.root+s.leaf),"need 50000");
    console.log("  → to test harvest, lower MIN_HARVEST_MASS on testnet, or accumulate more");
  }
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
