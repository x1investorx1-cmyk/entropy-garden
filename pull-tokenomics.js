const {Connection,PublicKey}=require("@solana/web3.js");
const conn=new Connection("https://rpc.mainnet.x1.xyz","confirmed");
const EG_MINT=new PublicKey("EkaYAgWf6mpDCiqcDP9cMbcvKj5MHxCSutPbdSXJcaQ2");
const TREASURY=new PublicKey("7WrZ9gKMygVBDNi2nk3ibZcc7RS4CeaGQnKb4tYy4hbZ");
const EG_CONFIG=new PublicKey("8pMeHLSU3XSTipyeRMcvuD4m5Yav2VrjMtLT5t53poFj");
(async()=>{
  // total EG supply actually minted (from the mint account)
  const mintInfo=await conn.getParsedAccountInfo(EG_MINT);
  const supply=mintInfo.value?.data?.parsed?.info?.supply;
  const decimals=mintInfo.value?.data?.parsed?.info?.decimals;
  const eg=Number(supply)/Math.pow(10,decimals);
  console.log("=== ENTROPY GARDEN — live mainnet numbers ===");
  console.log("Total EG minted (circulating from play + allocations):", eg.toLocaleString(), "EG");
  // Note: this INCLUDES the 100M locked (treasury+community+dev) since they were minted to PDAs
  console.log("  (includes 100M locked in treasury/community/dev PDAs)");
  const playMinted = eg - 100_000_000;
  console.log("  EG mined purely by PLAY (minus 100M allocations):", playMinted.toLocaleString(), "EG");
  // treasury XNT (the fee stream — backs staking yield + potential LP)
  const tBal=await conn.getBalance(TREASURY);
  console.log("\nTreasury XNT balance (fee stream collected):", (tBal/1e9).toFixed(4), "XNT");
  console.log("  → this is real value accrued from all gardening/quest fees");
  // how many wallets hold EG (holders) — approximate via largest accounts
  try{
    const largest=await conn.getTokenLargestAccounts(EG_MINT);
    console.log("\nTop token accounts (concentration check):");
    largest.value.slice(0,10).forEach((a,i)=>{
      console.log(`  #${i+1}: ${(Number(a.amount)/1e9).toLocaleString()} EG`);
    });
  }catch(e){console.log("largest accounts:",e.message);}
})().catch(e=>console.error(e.message));
