// check-bloom-round.js — inspect a Bloom Race round's full state
const {Connection,PublicKey}=require("@solana/web3.js");
const RPC="https://rpc.mainnet.x1.xyz";
const conn=new Connection(RPC,"confirmed");
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM=new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM=new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const EG_MINT=new PublicKey("EkaYAgWf6mpDCiqcDP9cMbcvKj5MHxCSutPbdSXJcaQ2");
const te=s=>new TextEncoder().encode(s);
const pda=s=>PublicKey.findProgramAddressSync(s,PID)[0];
function enc64(n){const b=Buffer.alloc(8);b.writeBigUInt64LE(BigInt(n));return b;}
const ata=o=>PublicKey.findProgramAddressSync([o.toBytes(),TOKEN_PROGRAM.toBytes(),EG_MINT.toBytes()],ATA_PROGRAM)[0];

const ROUND_ID=process.argv[2]||"57952917";

function decodeRound(d){
  let o=8;
  const id=d.readBigUInt64LE(o);o+=8;
  const phase=d[o];o+=1;
  const commitEnd=d.readBigUInt64LE(o);o+=8;
  const growEnd=d.readBigUInt64LE(o);o+=8;
  const b0=d.readUInt16LE(o);o+=2;
  const b1=d.readUInt16LE(o);o+=2;
  const s0=d.readBigUInt64LE(o);o+=8;
  const s1=d.readBigUInt64LE(o);o+=8;
  const pool=d.readBigUInt64LE(o);o+=8;
  const winner=d[o];o+=1;
  const resolved=d[o]===1;
  return{id,phase,commitEnd,growEnd,b0,b1,s0,s1,pool,winner,resolved};
}

(async()=>{
  console.log(`=== BLOOM RACE ROUND #${ROUND_ID} ===\n`);
  const ROUND=pda([te("bloom_round"),enc64(ROUND_ID)]);
  const VA=pda([te("bloom_vault"),enc64(ROUND_ID)]);
  const VAULT=ata(VA);
  console.log("Round PDA:",ROUND.toBase58());
  console.log("Vault:",VAULT.toBase58());

  const info=await conn.getAccountInfo(ROUND);
  if(!info){console.log("\n❌ Round account not found. Either wrong ID or never created.");return;}
  const r=decodeRound(info.data);
  const PHASE=['Commit','Growing','Resolved'];
  const BLOOMS=['🌧️ rainbloom (TPS)','🔥 emberpetal (fees)'];
  console.log("\n--- ROUND STATE ---");
  console.log("Phase:",PHASE[r.phase]??r.phase);
  console.log("Resolved:",r.resolved);
  console.log("Commit ended at slot:",r.commitEnd.toString());
  console.log("Grow ends at slot:",r.growEnd.toString());
  const now=await conn.getSlot();
  console.log("Current slot:",now,now>=Number(r.growEnd)?"(grow window PASSED — can resolve)":"(still growing)");
  console.log("\n--- STAKES ---");
  console.log("Baselines: rainbloom",r.b0,"bps · emberpetal",r.b1,"bps");
  console.log("Staked on rainbloom:",Number(r.s0)/1e9,"EG");
  console.log("Staked on emberpetal:",Number(r.s1)/1e9,"EG");
  console.log("Total pool:",Number(r.pool)/1e9,"EG");
  if(r.resolved){
    console.log("\n--- RESULT ---");
    console.log("WINNER:",BLOOMS[r.winner]);
  } else {
    console.log("\n⚠️  NOT YET RESOLVED — needs resolve_round called after grow window");
  }

  // check the vault balance — is the EG still sitting there?
  console.log("\n--- VAULT BALANCE ---");
  try{
    const vb=await conn.getTokenAccountBalance(VAULT);
    console.log("EG currently in vault:",vb.value.uiAmount,"EG");
    if(vb.value.uiAmount>0 && r.resolved){
      console.log("→ EG is in the vault waiting to be CLAIMED by winners");
    } else if(vb.value.uiAmount>0 && !r.resolved){
      console.log("→ EG is staked in the vault; round not resolved yet");
    } else if(vb.value.uiAmount===0){
      console.log("→ Vault empty — all EG has been claimed/distributed");
    }
  }catch(e){console.log("vault read error:",e.message);}

  // find all stakes for this round
  console.log("\n--- STAKE ACCOUNTS (who backed what) ---");
  const crypto=require("crypto");
  const stakeDisc=crypto.createHash("sha256").update("account:BloomStake").digest().slice(0,8);
  const all=await conn.getProgramAccounts(PID);
  let found=0;
  for(const a of all){
    const d=a.account.data;
    if(d.length<8)continue;
    if(!Buffer.from(d.slice(0,8)).equals(stakeDisc))continue;
    let o=8;
    const player=new PublicKey(d.slice(o,o+32));o+=32;
    const rid=d.readBigUInt64LE(o);o+=8;
    if(rid.toString()!==ROUND_ID.toString())continue;
    const bloom=d[o];o+=1;
    const amt=d.readBigUInt64LE(o);o+=8;
    const claimed=d[o]===1;
    found++;
    console.log(`  ${player.toBase58().slice(0,8)}… backed ${BLOOMS[bloom]} with ${Number(amt)/1e9} EG ${claimed?"[CLAIMED]":"[unclaimed]"}`);
  }
  if(!found)console.log("  (no stake accounts found — they may have been closed/claimed already)");
})().catch(e=>console.error("ERR:",e.message));
