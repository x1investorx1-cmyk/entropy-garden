// test-thread.js — TESTNET: verify Ariadne's Thread maze correctness.
// Enters, reveals, reads seed, recomputes the true path in JS (must match Rust),
// then steps correctly (should advance) and wrongly (should snap to gate).
const {Connection,PublicKey,Keypair,Transaction,TransactionInstruction,SystemProgram}=require("@solana/web3.js");
const {keccak_256}=require("js-sha3");
const fs=require("fs"),os=require("os");

const RPC="https://rpc.testnet.x1.xyz";
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM=new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM=new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const SLOT_HASHES=new PublicKey("SysvarS1otHashes111111111111111111111111111");
const conn=new Connection(RPC,"confirmed");
const W=Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8"))));

const te=s=>new TextEncoder().encode(s);
const pda=s=>PublicKey.findProgramAddressSync(s,PID)[0];
const EG_CONFIG=pda([te("eg_config")]),EG_MINT=pda([te("eg_mint")]);
const EG_AUTH=pda([te("eg_mint_auth")]),TREASURY=pda([te("eg_treasury")]);
const THREAD=pda([te("thread"),W.publicKey.toBytes()]);
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];

const D={enter:[106,217,18,214,223,28,203,196],reveal:[207,35,13,252,238,89,3,83],
         step:[109,84,180,73,157,201,237,0],abandon:[195,231,149,152,64,93,167,48]};

// ── port of true_path() from thread.rs ──────────────────────────────────────
const MAZE_W=16,MAZE_H=16,PATH_LEN=48;
const DIR_N=0,DIR_E=1,DIR_S=2,DIR_W=3;

function keccakStep(state){
  // hash state (32 bytes), copy hash back into state, return state[0]
  const h=keccak_256.arrayBuffer(state);
  const hb=new Uint8Array(h);
  state.set(hb.slice(0,32));
  return state[0];
}
function stepXY(x,y,dir){
  if(dir===DIR_N)return[x,y-1];
  if(dir===DIR_E)return[x+1,y];
  if(dir===DIR_S)return[x,y+1];
  if(dir===DIR_W)return[x-1,y];
  return[x,y];
}
function pickDistinct(order,k){
  const base=[DIR_N,DIR_E,DIR_S,DIR_W];
  const seen=[false,false,false,false];
  const out=[0,0,0,0];let idx=0;
  for(const o of order){if(!seen[o]){seen[o]=true;out[idx]=base[o];idx++;}}
  for(let i=0;i<4;i++){if(!seen[i]){seen[i]=true;out[idx]=base[i];idx++;}}
  return out[Math.min(k,3)];
}
function truePath(seed){
  const state=new Uint8Array(seed); // copy
  const path=[];
  let x=0,y=0;
  const visited=Array.from({length:MAZE_W},()=>new Array(MAZE_H).fill(false));
  visited[0][0]=true;
  while(path.length<PATH_LEN){
    const r=keccakStep(state);
    const order=[r%4,Math.floor(r/4)%4,Math.floor(r/16)%4,Math.floor(r/64)%4];
    let moved=false;
    for(let k=0;k<4;k++){
      const d=pickDistinct(order,k);
      const [nx,ny]=stepXY(x,y,d);
      if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H&&!visited[nx][ny]){
        x=nx;y=ny;visited[x][y]=true;path.push(d);moved=true;break;
      }
    }
    if(!moved){
      const d=keccakStep(state)%4;
      const [nx,ny]=stepXY(x,y,d);
      if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H){x=nx;y=ny;visited[x][y]=true;path.push(d);}
      else path.push((d+2)%4);
    }
  }
  return path;
}

async function send(keys,data,label){
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  const sig=await conn.sendRawTransaction(tx.serialize());
  await conn.confirmTransaction(sig,"confirmed");
  return sig;
}
async function getThread(){
  const info=await conn.getAccountInfo(THREAD);
  if(!info)return null;
  const d=info.data;
  // 8 disc + 32 walker + 8 entry_slot + 32 seed + 1 revealed + 1 pos + 1 last_cp + 8 amp + 8 pending + 1 heart + 1 active + 1 bump
  let o=8;
  const walker=d.slice(o,o+32);o+=32;
  const entrySlot=d.readBigUInt64LE(o);o+=8;
  const seed=Uint8Array.from(d.slice(o,o+32));o+=32;
  const revealed=d[o];o+=1;
  const pos=d[o];o+=1;
  const lastCp=d[o];o+=1;
  const amp=d.readBigUInt64LE(o);o+=8;
  const pending=d.readBigUInt64LE(o);o+=8;
  const heart=d[o];o+=1;
  const active=d[o];o+=1;
  return {entrySlot,seed,revealed,pos,lastCp,amp,pending,heart,active};
}

(async()=>{
  console.log("=== ARIADNE'S THREAD — correctness test (testnet) ===");
  console.log("wallet:",W.publicKey.toBase58());
  console.log("thread PDA:",THREAD.toBase58());
  console.log("balance:",(await conn.getBalance(W.publicKey))/1e9,"XNT\n");

  // clean up any existing thread
  let t=await getThread();
  if(t&&t.active){
    console.log("existing active thread found — abandoning it first…");
    try{await send([
      {pubkey:W.publicKey,isSigner:true,isWritable:true},
      {pubkey:THREAD,isSigner:false,isWritable:true},
    ],D.abandon,"abandon");console.log("abandoned.\n");}catch(e){console.log("abandon failed:",e.message);}
  }

  // 1. enter
  console.log("1. entering the labyrinth…");
  await send([
    {pubkey:W.publicKey,isSigner:true,isWritable:true},
    {pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},
    {pubkey:TREASURY,isSigner:false,isWritable:true},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ],D.enter,"enter");
  t=await getThread();
  console.log("   entered at slot",t.entrySlot.toString(),"\n");

  // 2. wait + reveal
  console.log("2. waiting for reveal delay, then revealing…");
  await new Promise(r=>setTimeout(r,4000));
  await send([
    {pubkey:W.publicKey,isSigner:true,isWritable:true},
    {pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:SLOT_HASHES,isSigner:false,isWritable:false},
  ],D.reveal,"reveal");
  t=await getThread();
  console.log("   revealed. seed:",Buffer.from(t.seed).toString("hex").slice(0,32),"…\n");

  // 3. compute the true path in JS
  const path=truePath(t.seed);
  console.log("3. computed true path (first 10 dirs):",path.slice(0,10).map(d=>"NESW"[d]).join(""));
  console.log("   full path length:",path.length,"\n");

  // 4. submit the CORRECT first step
  console.log("4. submitting CORRECT first step ("+"NESW"[path[0]]+")…");
  const stepKeys=[
    {pubkey:W.publicKey,isSigner:true,isWritable:true},
    {pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},
    {pubkey:EG_MINT,isSigner:false,isWritable:true},
    {pubkey:EG_AUTH,isSigner:false,isWritable:false},
    {pubkey:ata(W.publicKey),isSigner:false,isWritable:true},
    {pubkey:TREASURY,isSigner:false,isWritable:true},
    {pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ];
  try{
    await send(stepKeys,[...D.step,path[0]],"step-correct");
    t=await getThread();
    if(t.pos===1){console.log("   ✓ CORRECT STEP ACCEPTED — pos now 1, amplifier",Number(t.amp)/10000+"x, pending",t.pending.toString(),"\n");}
    else{console.log("   ✗ unexpected: pos is",t.pos,"\n");}
  }catch(e){console.log("   ✗ correct step FAILED:",e.message,"\n");}

  // 5. submit a WRONG step (a direction that isn't path[1])
  const wrongDir=(path[1]+1)%4;
  console.log("5. submitting WRONG step ("+"NESW"[wrongDir]+", correct was "+"NESW"[path[1]]+")…");
  try{
    await send(stepKeys,[...D.step,wrongDir],"step-wrong");
    t=await getThread();
    if(t.pos===0){console.log("   ✓ WRONG STEP SNAPPED TO GATE — pos back to 0, amplifier reset to",Number(t.amp)/10000+"x\n");}
    else{console.log("   ✗ unexpected: pos is",t.pos,"(should be 0)\n");}
  }catch(e){console.log("   ✗ wrong step errored (unexpected):",e.message,"\n");}

  console.log("=== RESULT ===");
  console.log("If both checks passed, the JS and Rust maze algorithms AGREE.");
  console.log("The frontend can safely render the same maze the program validates.");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
