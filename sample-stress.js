// sample-stress.js — read cumulative_stress from all Plant accounts on a cluster,
// so we can calibrate STORM_STRESS_DIVISOR from the real distribution.
const {Connection,PublicKey}=require("@solana/web3.js");
const RPC = process.argv[2] || "https://rpc.mainnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const conn = new Connection(RPC,"confirmed");

// Plant discriminator (anchor account): first 8 bytes of sha256("account:Plant")
const crypto = require("crypto");
const disc = crypto.createHash("sha256").update("account:Plant").digest().slice(0,8);

(async()=>{
  console.log("cluster:", RPC);
  // getProgramAccounts filtered by the Plant discriminator
  const accts = await conn.getProgramAccounts(PID, {
    filters: [{ memcmp: { offset: 0, bytes: require("bs58").encode(disc) } }],
  }).catch(async e=>{
    // some RPCs need dataSize or dislike memcmp; fall back to scanning all + filter
    console.log("(memcmp filter failed, scanning all program accounts)");
    const all = await conn.getProgramAccounts(PID);
    return all.filter(a=>a.account.data.slice(0,8).equals(disc));
  });
  console.log("plant accounts found:", accts.length, "\n");
  const stresses = [];
  for(const a of accts){
    const d = a.account.data;
    // cumulative_stress at byte 104 (u64 LE)
    if(d.length >= 112){
      const stress = Number(d.readBigUInt64LE(104));
      const stage = d.readUInt8(103);
      const health = d.readUInt16LE(93);
      stresses.push({stress, stage, health, key:a.pubkey.toBase58().slice(0,8)});
    }
  }
  stresses.sort((a,b)=>a.stress-b.stress);
  console.log("stress values (sorted):");
  for(const s of stresses) console.log(`  ${String(s.stress).padStart(10)}  stage ${s.stage} ❤${s.health}  ${s.key}`);
  if(stresses.length){
    const vals = stresses.map(s=>s.stress);
    const sum = vals.reduce((a,b)=>a+b,0);
    const med = vals[Math.floor(vals.length/2)];
    console.log(`\n  count ${vals.length} | min ${vals[0]} | median ${med} | max ${vals[vals.length-1]} | mean ${Math.round(sum/vals.length)}`);
    console.log("\n  To put the MEDIAN survivor at ~2x: divisor = median =", med);
    console.log("  To reserve 3x cap for top survivors: divisor ~= max/2 =", Math.round(vals[vals.length-1]/2));
  }
})().catch(e=>console.error("ERR:",e.message));
