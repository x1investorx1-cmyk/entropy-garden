const {Connection,PublicKey,Keypair,Transaction,TransactionInstruction,SystemProgram,SendTransactionError}=require("@solana/web3.js");
const {keccak_256}=require("js-sha3");const fs=require("fs"),os=require("os");
const RPC="https://rpc.testnet.x1.xyz";
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM=new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM=new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const SLOT_HASHES=new PublicKey("SysvarS1otHashes111111111111111111111111111");
const conn=new Connection(RPC,"confirmed");
const W=Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8"))));
const te=s=>new TextEncoder().encode(s);const pda=s=>PublicKey.findProgramAddressSync(s,PID)[0];
const EG_CONFIG=pda([te("eg_config")]),EG_MINT=pda([te("eg_mint")]),EG_AUTH=pda([te("eg_mint_auth")]),TREASURY=pda([te("eg_treasury")]);
const THREAD=pda([te("thread"),W.publicKey.toBytes()]),HEARTWALKER=pda([te("heartwalker"),W.publicKey.toBytes()]);
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];
const D={enter:[106,217,18,214,223,28,203,196],reveal:[207,35,13,252,238,89,3,83],step:[109,84,180,73,157,201,237,0],abandon:[195,231,149,152,64,93,167,48]};
const MAZE_W=16,MAZE_H=16,PATH_LEN=48;
function ks(s){const h=keccak_256.arrayBuffer(s);s.set(new Uint8Array(h).slice(0,32));return s[0];}
function sx(x,y,d){return d===0?[x,y-1]:d===1?[x+1,y]:d===2?[x,y+1]:[x-1,y];}
function pd2(o,k){const b=[0,1,2,3],sn=[0,0,0,0],ou=[0,0,0,0];let i=0;for(const x of o)if(!sn[x]){sn[x]=1;ou[i++]=b[x];}for(let j=0;j<4;j++)if(!sn[j]){sn[j]=1;ou[i++]=b[j];}return ou[Math.min(k,3)];}
function tp(seed){const st=new Uint8Array(seed),p=[];let x=0,y=0;const v=Array.from({length:MAZE_W},()=>new Array(MAZE_H).fill(false));v[0][0]=true;
  while(p.length<PATH_LEN){const r=ks(st);const o=[r%4,Math.floor(r/4)%4,Math.floor(r/16)%4,Math.floor(r/64)%4];let m=false;
    for(let k=0;k<4;k++){const d=pd2(o,k);const[nx,ny]=sx(x,y,d);if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H&&!v[nx][ny]){x=nx;y=ny;v[x][y]=1;p.push(d);m=true;break;}}
    if(!m){const d=ks(st)%4;const[nx,ny]=sx(x,y,d);if(nx>=0&&nx<MAZE_W&&ny>=0&&ny<MAZE_H){x=nx;y=ny;v[x][y]=1;p.push(d);}else p.push((d+2)%4);}}return p;}
async function sendLogged(keys,data,label){
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  try{const sig=await conn.sendRawTransaction(tx.serialize());await conn.confirmTransaction(sig,"confirmed");return sig;}
  catch(e){console.log(label,"FAILED.");if(e instanceof SendTransactionError){try{console.log("LOGS:",JSON.stringify(await e.getLogs(conn),null,1));}catch(_){console.log("raw:",e.message);}}else console.log(e.message);throw e;}}
async function gt(){const i=await conn.getAccountInfo(THREAD);if(!i)return null;const d=i.data;let o=8+32+8;const seed=Uint8Array.from(d.slice(o,o+32));o+=32;const rev=d[o++],pos=d[o++];return{seed,rev,pos,act:true};}
(async()=>{
  console.log("balance:",(await conn.getBalance(W.publicKey))/1e9,"XNT");
  let t=await conn.getAccountInfo(THREAD);
  if(t){console.log("abandoning old thread…");try{await sendLogged([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true}],D.abandon,"abandon");}catch(e){}}
  console.log("entering…");
  await sendLogged([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},{pubkey:EG_CONFIG,isSigner:false,isWritable:true},{pubkey:TREASURY,isSigner:false,isWritable:true},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],D.enter,"enter");
  await new Promise(r=>setTimeout(r,4000));
  await sendLogged([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},{pubkey:SLOT_HASHES,isSigner:false,isWritable:false}],D.reveal,"reveal");
  const th=await gt();const path=tp(th.seed);
  console.log("first correct dir:","NESW"[path[0]],"— attempting step 1 with full logs…");
  const stepKeys=[{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:THREAD,isSigner:false,isWritable:true},{pubkey:EG_CONFIG,isSigner:false,isWritable:true},{pubkey:EG_MINT,isSigner:false,isWritable:true},{pubkey:EG_AUTH,isSigner:false,isWritable:false},{pubkey:ata(W.publicKey),isSigner:false,isWritable:true},{pubkey:HEARTWALKER,isSigner:false,isWritable:true},{pubkey:TREASURY,isSigner:false,isWritable:true},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},{pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false}];
  await sendLogged(stepKeys,[...D.step,path[0]],"step 1");
  console.log("✓ step 1 succeeded!");
})().catch(e=>process.exit(1));
