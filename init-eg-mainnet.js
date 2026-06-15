// init-eg-mainnet.js — one-time: create EG mint on MAINNET
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction,
  SystemProgram, SYSVAR_RENT_PUBKEY,
} = require("@solana/web3.js");
const fs = require("fs"), os = require("os");

const RPC = "https://rpc.mainnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const conn = new Connection(RPC, "confirmed");

const secret = Uint8Array.from(JSON.parse(
  fs.readFileSync(os.homedir()+"/.config/solana/x1-mainnet-deployer.json","utf8")));
const wallet = Keypair.fromSecretKey(secret);

const FEE     = 25_000_000n; // 0.025 XNT
const FEE_CAP = 50_000_000n; // 0.05 XNT hard cap
const DISC = Buffer.from([137, 198, 67, 34, 67, 151, 59, 81]);

const te = s => new TextEncoder().encode(s);
const pda = seeds => PublicKey.findProgramAddressSync(seeds, PID)[0];

(async () => {
  console.log("=== EG MAINNET INIT ===");
  console.log("wallet :", wallet.publicKey.toBase58());
  console.log("balance:", (await conn.getBalance(wallet.publicKey))/1e9, "XNT\n");

  const egConfig   = pda([te("eg_config")]);
  const egMint     = pda([te("eg_mint")]);
  const egMintAuth = pda([te("eg_mint_auth")]);
  const treasury   = pda([te("eg_treasury")]);

  console.log("eg_config  :", egConfig.toBase58());
  console.log("eg_mint    :", egMint.toBase58());
  console.log("mint_auth  :", egMintAuth.toBase58());
  console.log("treasury   :", treasury.toBase58(), "\n");

  const existing = await conn.getAccountInfo(egConfig);
  if (existing) {
    console.log("✅ EG already initialized on mainnet!");
    console.log("   eg_mint:", egMint.toBase58());
    return;
  }

  const data = Buffer.concat([
    DISC,
    (() => { const b=Buffer.alloc(8); b.writeBigUInt64LE(FEE); return b; })(),
    (() => { const b=Buffer.alloc(8); b.writeBigUInt64LE(FEE_CAP); return b; })(),
  ]);

  const keys = [
    { pubkey: wallet.publicKey, isSigner:true,  isWritable:true  },
    { pubkey: egConfig,         isSigner:false, isWritable:true  },
    { pubkey: egMint,           isSigner:false, isWritable:true  },
    { pubkey: egMintAuth,       isSigner:false, isWritable:false },
    { pubkey: treasury,         isSigner:false, isWritable:true  },
    { pubkey: SystemProgram.programId, isSigner:false, isWritable:false },
    { pubkey: TOKEN_PROGRAM,    isSigner:false, isWritable:false },
    { pubkey: SYSVAR_RENT_PUBKEY, isSigner:false, isWritable:false },
  ];

  const ix = new TransactionInstruction({ programId:PID, keys, data });
  const tx = new Transaction().add(ix);
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  tx.sign(wallet);

  console.log("sending init_eg_mint to mainnet…");
  const sig = await conn.sendRawTransaction(tx.serialize());
  await conn.confirmTransaction(sig, "confirmed");

  console.log("\n🌱 EG TOKEN IS LIVE ON MAINNET!");
  console.log("   sig      :", sig);
  console.log("   eg_mint  :", egMint.toBase58());
  console.log("   treasury :", treasury.toBase58());
  console.log("   fee      : 0.025 XNT/action (cap: 0.05 XNT)");
  console.log("   genesis bonus: 1.5× active for 7 days from this slot");
  console.log("\nNext: update garden.html tend/plant accounts and deploy site.");
})().catch(e => { console.error("ERR:", e.message); process.exit(1); });
