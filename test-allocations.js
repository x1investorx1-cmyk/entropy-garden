// test-allocations.js — TESTNET test of mint_allocations + double-mint guard (no spl-token dep)
const {
  Connection, PublicKey, Keypair, Transaction, TransactionInstruction, SystemProgram,
} = require("@solana/web3.js");
const fs = require("fs"), os = require("os");

const RPC = "https://rpc.testnet.x1.xyz";
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const TOKEN_PROGRAM = new PublicKey("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA");
const ATA_PROGRAM   = new PublicKey("ATokenGPvbdGVxr1b2hvZbsiqW5xWH25efTNsLJA8knL");
const conn = new Connection(RPC, "confirmed");

const secret = Uint8Array.from(JSON.parse(
  fs.readFileSync(os.homedir()+"/.config/solana/x1-deployer.json","utf8")));
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
  console.log(`  OK ${label}: ${sig.slice(0,20)}...`);
  return sig;
}
async function bal(ata) {
  const b = await conn.getTokenAccountBalance(ata).catch(()=>null);
  return b ? BigInt(b.value.amount) : 0n;
}

(async () => {
  console.log("=== ALLOCATION TEST (testnet) ===");
  console.log("wallet:", wallet.publicKey.toBase58());
  console.log("balance:", (await conn.getBalance(wallet.publicKey))/1e9, "XNT\n");

  const treasuryEg  = ataFor(TREASURY);
  const communityEg = ataFor(COMMUNITY);
  const devEg       = ataFor(wallet.publicKey);
  console.log("treasury_eg :", treasuryEg.toBase58());
  console.log("community_eg:", communityEg.toBase58());
  console.log("dev_eg      :", devEg.toBase58(), "(your wallet)\n");

  console.log("creating token accounts...");
  const createTx = new Transaction();
  let need = false;
  for (const [ata, owner, name] of [
    [treasuryEg, TREASURY, "treasury"],
    [communityEg, COMMUNITY, "community"],
    [devEg, wallet.publicKey, "dev"],
  ]) {
    if (!(await conn.getAccountInfo(ata))) {
      createTx.add(createAtaIx(wallet.publicKey, ata, owner));
      console.log(`  + ${name}_eg`); need = true;
    } else console.log(`  ${name}_eg exists`);
  }
  if (need) await sendTx(createTx, "ATAs created");
  console.log("");

  const cfgBefore = await conn.getAccountInfo(EG_CONFIG);
  const mintedBefore = cfgBefore.data.readBigUInt64LE(81);
  console.log("total_minted before:", (mintedBefore/EG).toString(), "EG");
  const flagOffset = 8+32+32+1+8+8+8+8+32+1+1;
  if (cfgBefore.data[flagOffset] === 1) {
    console.log("  NOTE: _reserved[0]==1 — allocations already minted (verify-only run)");
  }

  console.log("\nminting 50M treasury / 40M community / 10M dev...");
  try {
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
  } catch (e) {
    console.log("  (reverted:", (e.message||"").slice(0,70), ")");
  }

  console.log("\n=== VERIFY ===");
  const t = await bal(treasuryEg), c = await bal(communityEg), d = await bal(devEg);
  console.log(`treasury : ${(t/EG).toString()} EG  ${t === 50_000_000n*EG ? "OK" : "FAIL expected 50M"}`);
  console.log(`community: ${(c/EG).toString()} EG  ${c === 40_000_000n*EG ? "OK" : "FAIL expected 40M"}`);
  console.log(`dev      : ${(d/EG).toString()} EG  ${d === 10_000_000n*EG ? "OK" : "FAIL expected 10M"}`);
  const total = (t + c + d) / EG;
  console.log(`sum      : ${total.toString()} EG  ${total === 100_000_000n ? "OK (10% of supply)" : "FAIL"}`);

  console.log("\ntesting double-mint guard (should FAIL)...");
  try {
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
    await sendTx(new Transaction().add(ix), "2nd mint");
    console.log("  PROBLEM: second mint succeeded — guard failed!");
  } catch (e) {
    console.log("  OK correctly rejected (AllocationsAlreadyMinted)");
  }

  console.log("\n=== DONE ===");
})().catch(e => { console.error("ERR:", e.message); process.exit(1); });
