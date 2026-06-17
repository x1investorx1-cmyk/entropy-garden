// test-bloom-full.js — complete cycle with SHORT windows (testnet fast mode)
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
const feeRid=new Uint8Array([0,0]),tpsRid=new Uint8Array([1,0]);
const FEED0=pda([te("weather"),tpsRid]);
const FEED1=pda([te("weather"),feeRid]);
const TREASURY_EG=new PublicKey("9yZyVZoxXG7TW572MMYibCWpWdx3Mu9h7bULxtTJZHDz");
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];
const D={open:[66,235,123,240,8,35,185,159],stake:[106,104,54,20,55,8,192,151],resolve:[165,114,237,158,1,36,70,254],claim:[161,215,24,59,14,236,242,221],closeLosing:[229,174,138,138,43,28,152,68]};
function enc64(n){const b=Buffer.alloc(8);b.writeBigUInt64LE(BigInt(n));return b;}
function enc8(n){return Buffer.from([n]);}
const roundPda=id=>pda([te("bloom_round"),enc64(id)]);
const vaultAuth=id=>pda([te("bloom_vault"),enc64(id)]);
const stakePda=(id,pk)=>pda([te("bloom_stake"),enc64(id),pk.toBytes()]);
function decodeRound(d){let o=8;const id=d.readBigUInt64LE(o);o+=8;const phase=d[o];o+=1;const commitEnd=d.readBigUInt64LE(o);o+=8;const growEnd=d.readBigUInt64LE(o);o+=8;const b0=d.readUInt16LE(o);o+=2;const b1=d.readUInt16LE(o);o+=2;const s0=d.readBigUInt64LE(o);o+=8;const s1=d.readBigUInt64LE(o);o+=8;const pool=d.readBigUInt64LE(o);o+=8;const winner=d[o];o+=1;const resolved=d[o]===1;return{id,phase,commitEnd,growEnd,b0,b1,s0,s1,pool,winner,resolved};}
async function send(keys,data,label){
  const tx=new Transaction().add(new TransactionInstruction({programId:PID,keys,data:Buffer.from(data)}));
  tx.feePayer=W.publicKey;tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;tx.sign(W);
  try{const sig=await conn.sendRawTransaction(tx.serialize());await conn.confirmTransaction(sig,"confirmed");console.log(`  ✓ ${label}`);return sig;}
  catch(e){console.log(`  ✗ ${label}:`,e.message.slice(0,120));if(e.getLogs){try{console.log((await e.getLogs(conn)).filter(l=>l.includes("log")||l.includes("Error")).slice(-5).join("\n"));}catch(_){}}throw e;}
}
async function egBal(pk){try{return(await conn.getTokenAccountBalance(ata(pk))).value.uiAmount;}catch(_){return 0;}}

(async()=>{
  console.log("=== BLOOM RACE FULL CYCLE TEST (testnet fast mode) ===\n");
  const ROUND_ID=await conn.getSlot();
  const ROUND=roundPda(ROUND_ID),VA=vaultAuth(ROUND_ID),VAULT=ata(VA),STAKE=stakePda(ROUND_ID,W.publicKey);
  console.log(`Round #${ROUND_ID}`);
  const before=await egBal(W.publicKey);
  console.log(`EG before: ${before}\n`);

  // 1. Open
  console.log("[1] open_round");
  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:ROUND,isSigner:false,isWritable:true},{pubkey:VA,isSigner:false,isWritable:false},{pubkey:VAULT,isSigner:false,isWritable:true},{pubkey:FEED0,isSigner:false,isWritable:false},{pubkey:FEED1,isSigner:false,isWritable:false},{pubkey:EG_MINT,isSigner:false,isWritable:false},{pubkey:EG_CONFIG,isSigner:false,isWritable:false},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},{pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],Buffer.concat([Buffer.from(D.open),enc64(ROUND_ID)]),"open_round");
  let rd=decodeRound((await conn.getAccountInfo(ROUND)).data);
  console.log(`  commit_end=${rd.commitEnd} grow_end=${rd.growEnd} baselines=[${rd.b0},${rd.b1}]`);

  // 2. Stake bloom 0 (20 EG)
  console.log("\n[2] stake_bloom(0, 20 EG) — rainbloom/TPS");
  const AMT=20n*1_000_000_000n;
  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:ROUND,isSigner:false,isWritable:true},{pubkey:STAKE,isSigner:false,isWritable:true},{pubkey:VA,isSigner:false,isWritable:false},{pubkey:VAULT,isSigner:false,isWritable:true},{pubkey:ata(W.publicKey),isSigner:false,isWritable:true},{pubkey:EG_MINT,isSigner:false,isWritable:false},{pubkey:EG_CONFIG,isSigner:false,isWritable:false},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},{pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],Buffer.concat([Buffer.from(D.stake),enc64(ROUND_ID),enc8(0),enc64(Number(AMT))]),"stake_bloom");
  rd=decodeRound((await conn.getAccountInfo(ROUND)).data);
  console.log(`  pool=${Number(rd.pool)/1e9} EG staked[0]=${Number(rd.s0)/1e9} staked[1]=${Number(rd.s1)/1e9}`);

  // 3. Wait for grow window
  console.log("\n[3] waiting for grow window to pass (~8 seconds on fast testnet)…");
  let slot=await conn.getSlot();
  while(slot<=Number(rd.growEnd)){await new Promise(r=>setTimeout(r,2000));slot=await conn.getSlot();process.stdout.write(`\r  slot ${slot} / ${rd.growEnd}`)}
  console.log(`\n  grow window passed at slot ${slot}`);

  // 4. Resolve
  console.log("\n[4] resolve_round");
  await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:ROUND,isSigner:false,isWritable:true},{pubkey:VA,isSigner:false,isWritable:false},{pubkey:VAULT,isSigner:false,isWritable:true},{pubkey:TREASURY_EG,isSigner:false,isWritable:true},{pubkey:FEED0,isSigner:false,isWritable:false},{pubkey:FEED1,isSigner:false,isWritable:false},{pubkey:EG_MINT,isSigner:false,isWritable:false},{pubkey:EG_CONFIG,isSigner:false,isWritable:false},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},{pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],Buffer.concat([Buffer.from(D.resolve),enc64(ROUND_ID)]),"resolve_round");
  rd=decodeRound((await conn.getAccountInfo(ROUND)).data);
  const BLOOMS=["🌧️ rainbloom (TPS)","🔥 emberpetal (fees)"];
  console.log(`  winner: ${BLOOMS[rd.winner]} (bloom ${rd.winner})`);

  // 5. Claim or close
  if(rd.winner===0){
    console.log("\n[5] claim_winnings (we backed the winner!)");
    await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:ROUND,isSigner:false,isWritable:false},{pubkey:STAKE,isSigner:false,isWritable:true},{pubkey:VA,isSigner:false,isWritable:false},{pubkey:VAULT,isSigner:false,isWritable:true},{pubkey:ata(W.publicKey),isSigner:false,isWritable:true},{pubkey:EG_MINT,isSigner:false,isWritable:false},{pubkey:EG_CONFIG,isSigner:false,isWritable:false},{pubkey:TOKEN_PROGRAM,isSigner:false,isWritable:false},{pubkey:ATA_PROGRAM,isSigner:false,isWritable:false},{pubkey:SystemProgram.programId,isSigner:false,isWritable:false}],Buffer.concat([Buffer.from(D.claim),enc64(ROUND_ID)]),"claim_winnings");
  } else {
    console.log("\n[5] close_losing_stake (we backed the loser — reclaim rent)");
    await send([{pubkey:W.publicKey,isSigner:true,isWritable:true},{pubkey:ROUND,isSigner:false,isWritable:false},{pubkey:STAKE,isSigner:false,isWritable:true}],Buffer.concat([Buffer.from(D.closeLosing),enc64(ROUND_ID)]),"close_losing_stake");
  }
  const after=await egBal(W.publicKey);
  console.log(`\n=== RESULT ===`);
  console.log(`EG before: ${before}  →  after: ${after}  (${(after-before)>=0?'+':''}${(after-before).toFixed(2)} EG)`);
  console.log(`Winner: ${BLOOMS[rd.winner]}`);
  console.log("\n✓ BLOOM RACE FULL CYCLE PROVEN");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
