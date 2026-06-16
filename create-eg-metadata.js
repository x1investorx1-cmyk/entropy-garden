// create-eg-metadata.js — ONE-TIME script to create EG token metadata on mainnet.
// Calls our program's initialize_token_metadata instruction, which CPIs into Metaplex.
const {Connection,PublicKey,Transaction,TransactionInstruction,SystemProgram,SYSVAR_RENT_PUBKEY}=require("@solana/web3.js");
const fs=require("fs"),os=require("os");

const RPC="https://rpc.mainnet.x1.xyz";
const conn=new Connection(RPC,"confirmed");
const PID=new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const METAPLEX=new PublicKey("metaqbxxUerdq28cj1RbAWkYQm3ybzjb6a8bt518x1s");
const W=require("@solana/web3.js").Keypair.fromSecretKey(
  Uint8Array.from(JSON.parse(fs.readFileSync(
    os.homedir()+"/.config/solana/x1-mainnet-deployer.json","utf8"))));

const te=s=>new TextEncoder().encode(s);
const pda=s=>PublicKey.findProgramAddressSync(s,PID)[0];

const EG_CONFIG=pda([te("eg_config")]);
const EG_MINT=new PublicKey("EkaYAgWf6mpDCiqcDP9cMbcvKj5MHxCSutPbdSXJcaQ2");
const EG_MINT_AUTH=pda([te("eg_mint_auth")]);

// Metaplex metadata PDA
const [METADATA_PDA]=PublicKey.findProgramAddressSync(
  [Buffer.from("metadata"), METAPLEX.toBytes(), EG_MINT.toBytes()],
  METAPLEX);

// initialize_token_metadata discriminator
const crypto=require("crypto");
const disc=Array.from(crypto.createHash("sha256")
  .update("global:initialize_token_metadata").digest().slice(0,8));

const NAME="Entropy Garden";
const SYMBOL="EG";
const URI="https://www.entropygarden.xyz/eg-metadata.json";

function encodeString(s){
  const b=Buffer.from(s,"utf8");
  const len=Buffer.alloc(4); len.writeUInt32LE(b.length);
  return Buffer.concat([len,b]);
}

const data=Buffer.concat([
  Buffer.from(disc),
  encodeString(NAME),
  encodeString(SYMBOL),
  encodeString(URI),
]);

(async()=>{
  console.log("=== Creating EG Token Metadata ===");
  console.log("Metadata PDA:",METADATA_PDA.toBase58());
  console.log("Authority:",W.publicKey.toBase58());
  console.log("Name:",NAME,"| Symbol:",SYMBOL);
  console.log("URI:",URI,"\n");

  // check it doesn't already exist
  const existing=await conn.getAccountInfo(METADATA_PDA);
  if(existing){console.log("✓ Metadata already exists! Nothing to do.");return;}

  const ix=new TransactionInstruction({
    programId:PID,
    data,
    keys:[
      {pubkey:W.publicKey,isSigner:true,isWritable:true},       // authority
      {pubkey:EG_CONFIG,isSigner:false,isWritable:false},       // eg_config
      {pubkey:EG_MINT,isSigner:false,isWritable:true},          // eg_mint
      {pubkey:EG_MINT_AUTH,isSigner:false,isWritable:false},    // eg_mint_auth (PDA signer)
      {pubkey:METADATA_PDA,isSigner:false,isWritable:true},     // metadata account
      {pubkey:METAPLEX,isSigner:false,isWritable:false},        // token_metadata_program
      {pubkey:SystemProgram.programId,isSigner:false,isWritable:false},
      {pubkey:SYSVAR_RENT_PUBKEY,isSigner:false,isWritable:false},
    ],
  });

  const tx=new Transaction().add(ix);
  tx.feePayer=W.publicKey;
  tx.recentBlockhash=(await conn.getLatestBlockhash()).blockhash;
  tx.sign(W);

  console.log("Sending transaction...");
  const sig=await conn.sendRawTransaction(tx.serialize());
  console.log("Signature:",sig);
  console.log("Confirming...");
  await conn.confirmTransaction(sig,"confirmed");
  console.log("\n✓ EG token metadata created!");
  console.log("  Wallets and explorers will now show:");
  console.log("  Name: Entropy Garden");
  console.log("  Symbol: EG");
  console.log("  Image: entropygarden.xyz/eg-token.png");
})().catch(e=>{console.error("ERR:",e.message);process.exit(1);});
