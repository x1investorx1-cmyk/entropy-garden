// init-eg.js — one-time: create the EG mint + EgConfig + treasury on TESTNET.
// Run on the server:  node init-eg.js
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction,
  SystemProgram, SYSVAR_RENT_PUBKEY,
} = require("@solana/web3.js");
const fs = require("fs");
const os = require("os");

const RPC = "https://rpc.testnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");

// load the testnet deployer wallet (the 30-XNT one)
const kpPath = os.homedir() + "/.config/solana/x1-deployer.json";
const secret = Uint8Array.from(JSON.parse(fs.readFileSync(kpPath, "utf8")));
const wallet = Keypair.fromSecretKey(secret);

// fee params: 0.025 XNT launch, 0.05 XNT hard cap (lamports = XNT * 1e9)
const FEE = 25_000_000n;       // 0.025 XNT
const FEE_CAP = 50_000_000n;   // 0.05 XNT
const DISC = Buffer.from([137, 198, 67, 34, 67, 151, 59, 81]);

function pda(seed) { return PublicKey.findProgramAddressSync([Buffer.from(seed)], PID)[0]; }

(async () => {
  const conn = new Connection(RPC, "confirmed");
  console.log("wallet:", wallet.publicKey.toBase58());
  console.log("balance:", (await conn.getBalance(wallet.publicKey)) / 1e9, "XNT\n");

  const egConfig  = pda("eg_config");
  const egMint    = pda("eg_mint");
  const egMintAuth= pda("eg_mint_auth");
  const treasury  = pda("eg_treasury");
  console.log("eg_config :", egConfig.toBase58());
  console.log("eg_mint   :", egMint.toBase58());
  console.log("mint_auth :", egMintAuth.toBase58());
  console.log("treasury  :", treasury.toBase58(), "\n");

  // already initialized?
  const existing = await conn.getAccountInfo(egConfig);
  if (existing) { console.log("EG already initialized on testnet. Nothing to do."); return; }

  // data = disc + fee (u64 LE) + fee_cap (u64 LE)
  const data = Buffer.concat([
    DISC,
    (() => { const b = Buffer.alloc(8); b.writeBigUInt64LE(FEE); return b; })(),
    (() => { const b = Buffer.alloc(8); b.writeBigUInt64LE(FEE_CAP); return b; })(),
  ]);

  const keys = [
    { pubkey: wallet.publicKey, isSigner: true,  isWritable: true },
    { pubkey: egConfig,         isSigner: false, isWritable: true },
    { pubkey: egMint,           isSigner: false, isWritable: true },
    { pubkey: egMintAuth,       isSigner: false, isWritable: false },
    { pubkey: treasury,         isSigner: false, isWritable: true },
    { pubkey: SystemProgram.programId, isSigner: false, isWritable: false },
    { pubkey: TOKEN_PROGRAM,    isSigner: false, isWritable: false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner: false, isWritable: false },
  ];

  const ix = new TransactionInstruction({ programId: PID, keys, data });
  const tx = new Transaction().add(ix);
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  tx.sign(wallet);

  console.log("sending init_eg_mint…");
  const sig = await conn.sendRawTransaction(tx.serialize());
  await conn.confirmTransaction(sig, "confirmed");
  console.log("✅ EG mint live on testnet!");
  console.log("   sig:", sig);
  console.log("   EG mint address:", egMint.toBase58());
})().catch(e => { console.error("ERR:", e.message); process.exit(1); });
