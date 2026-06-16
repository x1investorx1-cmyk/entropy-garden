// diag-plant.js — isolate the plant step with full error logs
const {Connection,PublicKey,Keypair,Transaction,TransactionInstruction,SystemProgram,SYSVAR_SLOT_HASHES_PUBKEY}=require("@solana/web3.js");
const fs=require("fs"),os=require("os");
const conn=new Connection("https://rpc.testnet.x1.xyz","confirmed");
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM=new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM=new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const W=Keypair.fromSecretKey(Uint8Array.from(JSON.parse(fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8"))));
const te=s=>new TextEncoder().encode(s);
const pda=s=>PublicKey.findProgramAddressSync(s,PID)[0];
const CONFIG=pda([te("config")]);
const EG_CONFIG=pda([te("eg_config")]), EG_MINT=pda([te("eg_mint")]);
const EG_AUTH=pda([te("eg_mint_auth")]), TREASURY=pda([te("eg_treasury")]);
const ataFor=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];
function u32(n){const b=Buffer.alloc(4);b.writeUInt32LE(n);return b;}
function u16(n){const b=Buffer.alloc(2);b.writeUInt16LE(n);return b;}

(async()=>{
  const cfg=await conn.getAccountInfo(CONFIG);
  const total=cfg.data.readUInt32LE(82);
  // find a plot this wallet owns
  let plot=null, idx=-1;
  for(let i=total-1; i>=Math.max(0,total-4); i--){
    const p=pda([te("plot"),W.publicKey.toBytes(),u32(i)]);
    if(await conn.getAccountInfo(p)){ plot=p; idx=i; break; }
  }
  if(!plot){ console.log("no plot found"); return; }
  console.log("plot index",idx,":",plot.toBase58());
  const plant=pda([te("plant"),plot.toBytes(),new Uint8Array([0])]);
  console.log("plant:",plant.toBase58());
  if(await conn.getAccountInfo(plant)){ console.log("plant already exists at slot 0 — try a fresh plot"); return; }

  // plant_seed WITH the EG accounts (the upgraded version)
  const keys=[
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
  ];
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,
    data:Buffer.concat([Buffer.from([139,66,41,202,41,145,173,204]),Buffer.from([0]),u16(1)]),keys}));
  tx.feePayer=W.publicKey; tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash; tx.sign(W);
  try{
    const sig=await conn.sendRawTransaction(tx.serialize());
    await conn.confirmTransaction(sig,"confirmed");
    console.log("PLANT OK (with EG accounts):",sig);
  }catch(e){
    console.log("PLANT FAILED:",e.message);
    if(e.logs) console.log(e.logs.join("\n"));
  }
})().catch(e=>console.error("ERR:",e.message));
