// test-carbon.js — full carbon farming cycle on testnet.
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

const EG_CONFIG=pda([te("eg_config")]),EG_MINT=pda([te("eg_mint")]);
const EG_AUTH=pda([te("eg_mint_auth")]);
const feeRid=Buffer.alloc(2);feeRid.writeUInt16LE(0);
const tpsRid=Buffer.alloc(2);tpsRid.writeUInt16LE(1);
const FEE_FEED=pda([te("weather"),feeRid]);
const TPS_FEED=pda([te("weather"),tpsRid]);
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];

const D={
  init:[65,40,21,72,133,170,66,51],
  seq:[18,92,178,99,150,7,65,106],
  harvest:[235,65,42,204,125,146,138,230],
};
const PLANT_DISC=null; // we'll scan program accounts for plants

async function send(keys,data,label){
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  try{const sig=await conn.sendRawTransaction(tx.serialize());await conn.confirmTransaction(sig,"confirmed");return sig;}
  catch(e){console.log(label,"FAILED:",e.message.slice(0,120));
    if(e.getLogs){try{const lg=await e.getLogs(conn);console.log(lg.slice(-6).join("\n"));}catch(_){}} throw e;}
}

// find a plant by scanning program accounts and matching the Plant discriminator in JS
async function findPlant(){
  const crypto=require("crypto");
  const disc=crypto.createHash("sha256").update("account:Plant").digest().slice(0,8);
  const all=await conn.getProgramAccounts(PID);
  for(const a of all){
    const d=a.account.data;
    if(d.length>=8 && Buffer.from(d.slice(0,8)).equals(disc)){
      // prefer a plant whose planter we are — but any valid plant works for the sink test
      return a;
    }
  }
  return null;
}

function decodeSink(d){
  let o=8;
  const plant=new PublicKey(d.slice(o,o+32));o+=32;
  const owner=new PublicKey(d.slice(o,o+32));o+=32;
  const root=d.readBigUInt64LE(o);o+=8;
  const leaf=d.readBigUInt64LE(o);o+=8;
  const last=d.readBigUInt64LE(o);o+=8;
  const harvested=d.readBigUInt64LE(o);o+=8;
  return{plant:plant.toBase58(),owner:owner.toBase58(),root,leaf,last,harvested};
}

(async()=>{
  console.log("=== CARBON FARMING TEST (testnet) ===\n");
  const plant=await findPlant();
  if(!plant){console.log("No plant found on testnet to attach a sink to. Plant one first.");return;}
  const PLANT=plant.pubkey;
  console.log("using plant:",PLANT.toBase58());
  // decode plant plot + slot_index for the seeds check (sink derives from plant key only)
  const SINK=pda([te("carbon"),PLANT.toBytes()]);
  console.log("carbon sink PDA:",SINK.toBase58());

  // 1. init sink (if not exists)
  const existing=await conn.getAccountInfo(SINK);
  if(!existing){
    console.log("\nopening carbon sink…");
    // need plant.plot + slot_index for the plant account seeds in InitCarbonSink
    const pd=plant.account.data; let po=8;
    const plot=new PublicKey(pd.slice(po,po+32));po+=32;
    const slotIndex=pd.readUInt8(po);
    const PLOT=plot, SI=slotIndex;
    await send([
      {pubkey:W.publicKey,isSigner:true,isWritable:true},
      {pubkey:PLANT,isSigner:false,isWritable:false},
      {pubkey:SINK,isSigner:false,isWritable:true},
      {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
    ],D.init,"init");
    console.log("  sink opened ✓");
  } else { console.log("  sink already exists"); }

  // 2. sequester a few times with delays to accumulate
  for(let i=0;i<3;i++){
    await new Promise(r=>setTimeout(r,3000));
    await send([
      {pubkey:SINK,isSigner:false,isWritable:true},
      {pubkey:FEE_FEED,isSigner:false,isWritable:false},
      {pubkey:TPS_FEED,isSigner:false,isWritable:false},
    ],D.seq,"sequester");
    const s=decodeSink((await conn.getAccountInfo(SINK)).data);
    console.log(`  sequester ${i+1}: root=${s.root} leaf=${s.leaf}`);
  }

  // 3. read final state
  const s=decodeSink((await conn.getAccountInfo(SINK)).data);
  console.log("\n=== sink state ===");
  console.log("  root_mass:",s.root.toString(),"(from fees)");
  console.log("  leaf_mass:",s.leaf.toString(),"(from TPS)");
  console.log("  total:",(s.root+s.leaf).toString());
  console.log("\n(harvest needs total_mass >= 100; calibrate RATE_DIV if growth too slow/fast)");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
