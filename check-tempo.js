// check-tempo.js — read region-1 (TPS) feed bps to calibrate Pulse bands.
const {Connection,PublicKey}=require("@solana/web3.js");
const NET=process.argv[2]||"mainnet";
const RPC=NET==="testnet"?"https://rpc.testnet.x1.xyz":"https://rpc.mainnet.x1.xyz";
const conn=new Connection(RPC,"confirmed");
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");

const ridBuf=Buffer.alloc(2); ridBuf.writeUInt16LE(1); // region 1
const [FEED]=PublicKey.findProgramAddressSync([Buffer.from("weather"),ridBuf],PID);

// also pull live TPS directly from the chain for comparison
async function liveTps(){
  const perf=await conn.getRecentPerformanceSamples(8);
  if(!perf.length)return null;
  const tps=perf.reduce((a,s)=>a+s.numTransactions/s.samplePeriodSecs,0)/perf.length;
  return tps;
}

function bandOf(bps){
  if(bps<2000)return "💤 DORMANT";
  if(bps<4500)return "🌿 STEADY";
  if(bps<7000)return "⚡ BUSY";
  return "🔥 SURGING";
}

(async()=>{
  console.log(`=== ${NET.toUpperCase()} tempo (region 1 feed) ===`);
  const info=await conn.getAccountInfo(FEED);
  if(!info){console.log("feed account not found");return;}
  const d=info.data;
  // WeatherFeed: 8 disc + 2 region_id + 1 head + 1 len + [u16;64] samples + [u64;64] slots + 1 bump
  let o=8;
  const regionId=d.readUInt16LE(o);o+=2;
  const head=d.readUInt8(o);o+=1;
  const len=d.readUInt8(o);o+=1;
  const samples=[];
  for(let i=0;i<64;i++){samples.push(d.readUInt16LE(o));o+=2;}
  // recent samples (last 10 in ring order)
  console.log("samples in feed:",len);
  const recent=[];
  for(let i=0;i<Math.min(len,15);i++){
    const idx=(head+64-1-i)%64;
    recent.push(samples[idx]);
  }
  console.log("\nMost recent bps (newest first):");
  recent.forEach((b,i)=>console.log(`  ${b} bps  → ${bandOf(b)}`));
  // stats
  const vals=recent.filter(v=>v>0);
  if(vals.length){
    const min=Math.min(...vals),max=Math.max(...vals);
    const avg=Math.round(vals.reduce((a,b)=>a+b,0)/vals.length);
    console.log(`\n  range: ${min}–${max} bps, avg ${avg} bps → ${bandOf(avg)}`);
  }
  const tps=await liveTps();
  if(tps)console.log(`\n  live chain TPS right now: ${tps.toFixed(0)} TPS`);
  console.log(`\n  (bands: <2000 dormant, 2000-4499 steady, 4500-6999 busy, >=7000 surging)`);
})().catch(e=>console.error(e.message));
