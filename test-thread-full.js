// test-thread-full.js — walk an ENTIRE maze gate→heart, verify economics.
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

const MAZE_W=16,MAZE_H=16,PATH_LEN=48;
function keccakStep(state){const h=keccak_256.arrayBuffer(state);state.set(new Uint8Array(h).slice(0,32));return state[0];}
function stepXY(x,y,d){return d===0?[x,y-1]:d===1?[x+1,y]:d===2?[x,y+1]:[x-1,y];}
function pickDistinct(order,k){const base=[0,1,2,3],seen=[0,0,0,0],out=[0,0,0,0];let i=0;
  for(const o of order)if(!seen[o]){seen[o]=1;out[i++]=base[o];}
  for(let j=0;j<4;j++)if(!seen[j]){seen[j]=1;out[i++]=base[j];}return out[Math.min(k,3)];}
function truePath(seed){const state=new Uint8Array(seed),path=[];let x=0,y=0;
  const vis=Array.from({length:MAZE_W},()=>new Array(MAZE_H).fill(false));vis[0][0]=true;
  while(path.length<PATH_LEN){const r=keccakStep(state);
    const order=[r%4,Math.floor(r/4)%4,Math.floor(r/16)%4,Math.floor(r/64)%4];let moved=false;
    for(let k=0;k<4;k++){const d=pickDistinct(order,k);const[nx,ny]=stepXY(x,y,d);
      if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H&&!vis[nx][ny]){x=nx;y=ny;vis[x][y]=true;path.push(d);moved=true;break;}}
    if(!moved){const d=keccakStep(state)%4;const[nx,ny]=stepXY(x,y,d);
      if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H){x=nx;y=ny;vis[x][y]=true;path.push(d);}else path.push((d+2)%4);}}
  return path;}

async function send(keys,data){const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  const sig=await conn.sendRawTransaction(tx.serialize());await conn.confirmTransaction(sig,"confirmed");return sig;}
async function getThread(){const info=await conn.getAccountInfo(THREAD);if(!info)return null;const d=info.data;let o=8;
  o+=32;const entrySlot=d.readBigUInt64LE(o);o+=8;const seed=Uint8Array.from(d.slice(o,o+32));o+=32;
  const revealed=d[o++],pos=d[o++],lastCp=d[o++],amp=d.readBigUInt64LE(o);o+=8;const pending=d.readBigUInt64LE(o);o+=8;
  const heart=d[o++],active=d[o++];return{entrySlot,seed,revealed,pos,lastCp,amp,pending,heart,active};}
async function egBal(){const b=await conn.getTokenAccountBalance(ata(W.publicKey)).catch(()=>null);return b?parseFloat(b.value.uiAmountString):0;}

(async()=>{
  console.log("=== ARIADNE'S THREAD — full run (testnet) ===\n");
  let t=await getThread();
  if(t&&t.active){console.log("abandoning existing thread…");
    await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true}],D.abandon);}

  const egBefore=await egBal();
  console.log("EG before:",egBefore);

  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},{pubkey:TREASURY,isSigner:false,isWritable:true},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],D.enter);
  console.log("entered. waiting to reveal…");
  await new Promise(r=>setTimeout(r,4000));
  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:SLOT_HASHES,isSigner:false,isWritable:false}],D.reveal);
  t=await getThread();
  const path=truePath(t.seed);
  console.log("revealed. walking all",path.length,"steps…\n");

  const stepKeys=[
    {pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},{pubkey:EG_MINT,isSigner:false,isWritable:true},
    {pubkey:EG_AUTH,isSigner:false,isWritable:false},{pubkey:ata(W.publicKey),isSigner:false,isWritable:true},
    {pubkey:TREASURY,isSigner:false,isWritable:true},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ];
  for(let i=0;i<path.length;i++){
    try{await send(stepKeys,[...D.step,path[i]]);
      if((i+1)%6===0||i===path.length-1){t=await getThread();
        console.log(`  step ${i+1}/48 · amp ×${(Number(t.amp)/10000).toFixed(2)} · pending_bp ${t.pending} · EG ${await egBal()}`);}
    }catch(e){console.log(`  step ${i+1} FAILED:`,e.message.slice(0,60));break;}
  }
  t=await getThread();
  const egAfter=await egBal();
  console.log("\n=== RESULT ===");
  console.log("reached heart:",!!t?.heart);
  console.log("EG earned this run:",(egAfter-egBefore).toFixed(2),"EG");
  console.log("expected ~6.6 EG (genesis 1.5x applied: ~66 'whole' = wait, let me note:)");
  console.log("(base 0.30/step × amp, +8 heart, ×1.5 genesis, over 48 steps)");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
