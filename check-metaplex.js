const {Connection,PublicKey}=require("@solana/web3.js");
const conn=new Connection("https://rpc.mainnet.x1.xyz","confirmed");

const METAPLEX_PROGRAM=new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const EG_MINT=new PublicKey("EkaYAgWf6mpDCiqcDP9cMbcvKj5MHxCSutPbdSXJcaQ2");

async function getMetadataPDA(mint){
  const [pda]=PublicKey.findProgramAddressSync(
    [Buffer.from("metadata"),METAPLEX_PROGRAM.toBytes(),mint.toBytes()],
    METAPLEX_PROGRAM);
  return pda;
}

(async()=>{
  console.log("=== Checking X1 mainnet for Metaplex ===");
  // check if Metaplex program exists on X1
  const mpInfo=await conn.getAccountInfo(METAPLEX_PROGRAM);
  console.log("Metaplex Token Metadata program:",mpInfo?"EXISTS ✓":"NOT FOUND ✗");
  
  // check if EG already has metadata
  const metaPDA=await getMetadataPDA(EG_MINT);
  console.log("EG Metadata PDA:",metaPDA.toBase58());
  const metaInfo=await conn.getAccountInfo(metaPDA);
  console.log("EG metadata account:",metaInfo?"EXISTS (already has metadata)":"does not exist yet");
  
  // check the current mint authority
  const mintInfo=await conn.getParsedAccountInfo(EG_MINT);
  const ma=mintInfo.value?.data?.parsed?.info?.mintAuthority;
  const fa=mintInfo.value?.data?.parsed?.info?.freezeAuthority;
  console.log("\nEG mint authority:",ma);
  console.log("EG freeze authority:",fa);
})().catch(e=>console.error(e.message));
