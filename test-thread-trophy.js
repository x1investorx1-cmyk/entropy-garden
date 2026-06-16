// test-thread-trophy.js — walk to the heart, verify the HeartWalker trophy is stamped.
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
const HEARTWALKER=pda([te("heartwalker"),W.publicKey.toBytes()]);
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];
const D={enter:[106,217,18,214,223,28,203,196],reveal:[207,35,13,252,238,89,3,83],step:[109,84,180,73,157,201,237,0],abandon:[195,231,149,152,64,93,167,48]};
const MAZE_W=16,MAZE_H=16,PATH_LEN=48;
function keccakStep(s){const h=keccak_256.arrayBuffer(s);s.set(new Uint8Array(h).slice(0,32));return s[0];}
function stepXY(x,y,d){return d===0?[x,y-1]:d===1?[x+1,y]:d===2?[x,y+1]:[x-1,y];}
function pickDistinct(o,k){const b=[0,1,2,3],seen=[0,0,0,0],out=[0,0,0,0];let i=0;
  for(const x of o)if(!seen[x]){seen[x]=1;out[i++]=b[x];}for(let j=0;j<4;j++)if(!seen[j]){seen[j]=1;out[i++]=b[j];}return out[Math.min(k,3)];}
function truePath(seed){const st=new Uint8Array(seed),path=[];let x=0,y=0;
  const v=Array.from({length:MAZE_W},()=>new Array(MAZE_H).fill(false));v[0][0]=true;
  while(path.length<PATH_LEN){const r=keccakStep(st);const o=[r%4,Math.floor(r/4)%4,Math.floor(r/16)%4,Math.floor(r/64)%4];let m=false;
    for(let k=0;k<4;k++){const d=pickDistinct(o,k);const[nx,ny]=stepXY(x,y,d);
      if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H&&!v[nx][ny]){x=nx;y=ny;v[x][y]=true;path.push(d);m=true;break;}}
    if(!m){const d=keccakStep(st)%4;const[nx,ny]=stepXY(x,y,d);
      if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H){x=nx;y=ny;v[x][y]=true;path.push(d);}else path.push((d+2)%4);}}
  return path;}
async function send(keys,data){const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  const sig=await conn.sendRawTransaction(tx.serialize());await conn.confirmTransaction(sig,"confirmed");return sig;}
async function getThread(){const i=await conn.getAccountInfo(THREAD);if(!i)return null;const d=i.data;let o=8+32;
  o+=8;const seed=Uint8Array.from(d.slice(o,o+32));o+=32;const rev=d[o++],pos=d[o++],lc=d[o++];o+=8;o+=8;const heart=d[o++],act=d[o++];return{seed,rev,pos,heart,act};}
async function getHeartWalker(){const i=await conn.getAccountInfo(HEARTWALKER);if(!i)return null;const d=i.data;let o=8;
  const walker=new PublicKey(d.slice(o,o+32));o+=32;const firstSlot=d.readBigUInt64LE(o);o+=8;
  const totalHearts=d.readUInt32LE(o);o+=4;const bestAmp=d.readBigUInt64LE(o);o+=8;const fewestWrong=d.readUInt32LE(o);o+=4;
  const firstSeed=Uint8Array.from(d.slice(o,o+32));o+=32;return{walker:walker.toBase58(),firstSlot,totalHearts,bestAmp,fewestWrong,firstSeed};}

(async()=>{
  console.log("=== ARIADNE TROPHY TEST — reach heart, verify HeartWalker ===\n");
  console.log("HeartWalker PDA:",HEARTWALKER.toBase58());
  let t=await getThread();
  if(t&&t.act){console.log("abandoning existing thread…");
    await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true}],D.abandon);}
  const hwBefore=await getHeartWalker();
  console.log("HeartWalker before:",hwBefore?`exists, total_hearts=${hwBefore.totalHearts}`:"does not exist yet");

  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},{pubkey:TREASURY,isSigner:false,isWritable:true},
    {pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],D.enter);
  await new Promise(r=>setTimeout(r,4000));
  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:SLOT_HASHES,isSigner:false,isWritable:false}],D.reveal);
  t=await getThread();const path=truePath(t.seed);
  console.log("revealed. walking",path.length,"steps to the heart…\n");

  const stepKeys=[
    {pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},
    {pubkey:EG_CONFIG,isSigner:false,isWritable:true},{pubkey:EG_MINT,isSigner:false,isWritable:true},
    {pubkey:EG_AUTH,isSigner:false,isWritable:false},{pubkey:ata(W.publicKey),isSigner:false,isWritable:true},
    {pubkey:HEARTWALKER,isSigner:false,isWritable:true}, // NEW trophy account
    {pubkey:TREASURY,isSigner:false,isWritable:true},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},
    {pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
  ];
  for(let i=0;i<path.length;i++){
    try{await send(stepKeys,[...D.step,path[i]]);
      if((i+1)%12===0||i===path.length-1)console.log(`  …step ${i+1}/48`);
    }catch(e){console.log(`  step ${i+1} FAILED:`,e.message.slice(0,80));return;}
  }
  console.log("\nreached the heart. checking trophy…\n");
  const hw=await getHeartWalker();
  if(!hw){console.log("✗ HeartWalker NOT created — trophy failed");return;}
  console.log("=== HEARTWALKER TROPHY ===");
  console.log("  walker:",hw.walker);
  console.log("  first_heart_slot:",hw.firstSlot.toString());
  console.log("  total_hearts:",hw.totalHearts);
  console.log("  best_amplifier:",(Number(hw.bestAmp)/10000).toFixed(2)+"x");
  console.log("  fewest_wrong_turns:",hw.fewestWrong);
  console.log("  first_seed:",Buffer.from(hw.firstSeed).toString("hex").slice(0,24)+"…");
  console.log("\n✓ TROPHY STAMPED — permanent on-chain proof of reaching the heart.");
  console.log("  (first_seed renders the unique labyrinth rose for this walker)");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
