// allocate-mainnet.js — IRREVERSIBLE: mint the fixed 5/4/1% allocations on MAINNET.
// Treasury (50M) + Community (40M) locked in PDAs; Dev (10M) -> plot-0 wallet.
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction, SystemProgram,
} = require("@solana/web3.js");
const fs = require("fs"), os = require("os");

const RPC = "https://rpc.mainnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM   = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
// DEV allocation destination — the plot-0 wallet (public, transparent)
const DEV_WALLET = new PublicKey("9znPUpKor3fPvoFhbCuMcD6RFQvNKiXf7erppLLtgKsJ");
const conn = new Connection(RPC, "confirmed");

// authority = mainnet deployer (the eg_config.authority on mainnet)
const secret = Uint8Array.from(JSON.parse(
  fs.readFileSync(os.homedir()+"/.config/solana/x1-mainnet-deployer.json","utf8")));
const wallet = Keypair.fromSecretKey(secret);

const te = s => new TextEncoder().encode(s);
const pda = seeds => PublicKey.findProgramAddressSync(seeds, PID)[0];
const EG_CONFIG   = pda([te("eg_config")]);
const EG_MINT     = pda([te("eg_mint")]);
const EG_MINT_AUTH= pda([te("eg_mint_auth")]);
const TREASURY    = pda([te("eg_treasury")]);
const COMMUNITY   = pda([te("eg_community")]);

const DISC_MINT = Buffer.from([205, 24, 88, 126, 149, 81, 161, 252]);
const EG = 1_000_000_000n;

function ataFor(owner) {
  return PublicKey.findProgramAddressSync(
    [owner.toBytes(), TOKEN_PROGRAM.toBytes(), EG_MINT.toBytes()], ATA_PROGRAM)[0];
}
function createAtaIx(payer, ata, owner) {
  return new TransactionInstruction({
    programId: ATA_PROGRAM, data: Buffer.alloc(0),
    keys: [
      { pubkey: payer, isSigner:true,  isWritable:true  },
      { pubkey: ata,   isSigner:false, isWritable:true  },
      { pubkey: owner, isSigner:false, isWritable:false },
      { pubkey: EG_MINT, isSigner:false, isWritable:false },
      { pubkey: SystemProgram.programId, isSigner:false, isWritable:false },
      { pubkey: TOKEN_PROGRAM, isSigner:false, isWritable:false },
    ],
  });
}
async function sendTx(tx, label) {
  tx.feePayer = wallet.publicKey;
  tx.recentBlockhash = (await conn.getLatestBlockhash()).blockhash;
  tx.sign(wallet);
  const sig = await conn.sendRawTransaction(tx.serialize());
  await conn.confirmTransaction(sig, "confirmed");
  console.log(`  OK ${label}: ${sig}`);
  return sig;
}
async function bal(ata) {
  const b = await conn.getTokenAccountBalance(ata).catch(()=>null);
  return b ? BigInt(b.value.amount) : 0n;
}

(async () => {
  console.log("=== MAINNET ALLOCATION (IRREVERSIBLE) ===");
  console.log("authority wallet:", wallet.publicKey.toBase58());
  console.log("balance:", (await conn.getBalance(wallet.publicKey))/1e9, "XNT\n");

  const treasuryEg  = ataFor(TREASURY);
  const communityEg = ataFor(COMMUNITY);
  const devEg       = ataFor(DEV_WALLET);
  console.log("treasury_eg  (PDA-locked):", treasuryEg.toBase58());
  console.log("community_eg (PDA-locked):", communityEg.toBase58());
  console.log("dev_eg (plot-0 wallet)   :", devEg.toBase58(), "\n");

  // verify config + authority match
  const cfg = await conn.getAccountInfo(EG_CONFIG);
  if (!cfg) { console.log("ERR: eg_config not found — is the program initialized on mainnet?"); return; }
  const cfgAuthority = new PublicKey(cfg.data.slice(8, 40));
  console.log("eg_config.authority:", cfgAuthority.toBase58());
  if (!cfgAuthority.equals(wallet.publicKey)) {
    console.log("ERR: this wallet is NOT the config authority. Cannot mint allocations.");
    console.log("     expected authority:", cfgAuthority.toBase58());
    return;
  }
  const flagOffset = 8+32+32+1+8+8+8+8+32+1+1;
  if (cfg.data[flagOffset] === 1) {
    console.log("\nNOTE: allocations already minted on mainnet. Verify-only.");
  }
  const mintedBefore = cfg.data.readBigUInt64LE(81);
  console.log("total_minted before:", (mintedBefore/EG).toString(), "EG\n");

  // create ATAs
  console.log("creating token accounts...");
  const createTx = new Transaction(); let need = false;
  for (const [ata, owner, name] of [
    [treasuryEg, TREASURY, "treasury"],
    [communityEg, COMMUNITY, "community"],
    [devEg, DEV_WALLET, "dev"],
  ]) {
    if (!(await conn.getAccountInfo(ata))) {
      createTx.add(createAtaIx(wallet.publicKey, ata, owner));
      console.log(`  + ${name}_eg`); need = true;
    } else console.log(`  ${name}_eg exists`);
  }
  if (need) await sendTx(createTx, "ATAs created");

  // mint
  console.log("\nMINTING ALLOCATIONS ON MAINNET...");
  const ix = new TransactionInstruction({
    programId: PID, data: DISC_MINT,
    keys: [
      { pubkey: wallet.publicKey, isSigner:true,  isWritable:true  },
      { pubkey: EG_CONFIG,        isSigner:false, isWritable:true  },
      { pubkey: EG_MINT,          isSigner:false, isWritable:true  },
      { pubkey: EG_MINT_AUTH,     isSigner:false, isWritable:false },
      { pubkey: treasuryEg,       isSigner:false, isWritable:true  },
      { pubkey: communityEg,      isSigner:false, isWritable:true  },
      { pubkey: devEg,            isSigner:false, isWritable:true  },
      { pubkey: TOKEN_PROGRAM,    isSigner:false, isWritable:false },
    ],
  });
  await sendTx(new Transaction().add(ix), "mint_allocations");

  console.log("\n=== VERIFY ===");
  const t = await bal(treasuryEg), c = await bal(communityEg), d = await bal(devEg);
  console.log(`treasury : ${(t/EG).toString()} EG  ${t === 50_000_000n*EG ? "OK" : "FAIL"}`);
  console.log(`community: ${(c/EG).toString()} EG  ${c === 40_000_000n*EG ? "OK" : "FAIL"}`);
  console.log(`dev      : ${(d/EG).toString()} EG  ${d === 10_000_000n*EG ? "OK" : "FAIL"}`);
  const total = (t+c+d)/EG;
  console.log(`sum      : ${total.toString()} EG  ${total === 100_000_000n ? "OK (10% of supply)" : "FAIL"}`);
  console.log("\n=== ALLOCATIONS LIVE ON MAINNET ===");
  console.log("Supply now: 90% mineable / 5% treasury / 4% community / 1% dev");
  console.log("Renounce authority later, once LP-open + airdrop instructions are built.");
})().catch(e => { console.error("ERR:", e.message); process.exit(1); });
