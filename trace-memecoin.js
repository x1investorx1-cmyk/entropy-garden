const { Connection, PublicKey } = require("@solana/web3.js");
const conn = new Connection("https://rpc.mainnet.x1.xyz", "confirmed");
const MINT = new PublicKey("6gYK5WWWmpfxSnQtBoYd8xoSF7iphw2uBGiTdFkXww8V");
const short = a => a ? a.slice(0,4)+"…"+a.slice(-4) : a;
(async () => {
  console.log("\n=== MINT", MINT.toBase58(), "===");
  const info = await conn.getParsedAccountInfo(MINT);
  if (!info.value) { console.log("Mint not found on X1 mainnet."); return; }
  console.log("owner program:", info.value.owner.toBase58());
  const d = info.value.data?.parsed?.info;
  if (d) { console.log("supply:", d.supply, "| decimals:", d.decimals);
    console.log("mintAuthority :", d.mintAuthority || "(renounced/none)");
    console.log("freezeAuthority:", d.freezeAuthority || "(none)"); }
  const sigs = await conn.getSignaturesForAddress(MINT, { limit: 100 });
  console.log(`\n=== ${sigs.length} transactions, oldest first ===`);
  sigs.reverse();
  for (const s of sigs) {
    const t = s.blockTime ? new Date(s.blockTime*1000).toISOString() : "?";
    console.log(`\n● ${s.signature}`);
    console.log(`  slot ${s.slot}  ${s.err?"FAILED":"ok"}  ${t}`);
    try {
      const tx = await conn.getParsedTransaction(s.signature, {maxSupportedTransactionVersion:0});
      if (!tx) { console.log("  (no detail)"); continue; }
      const signers = tx.transaction.message.accountKeys.filter(k=>k.signer).map(k=>short(k.pubkey.toBase58()));
      console.log("  signer(s):", signers.join(", "));
      const pre = tx.meta?.preTokenBalances||[], post = tx.meta?.postTokenBalances||[];
      const rel = [...new Set([...pre,...post].filter(b=>b.mint===MINT.toBase58()).map(b=>b.owner))];
      for (const owner of rel) {
        const p = pre.find(b=>b.owner===owner&&b.mint===MINT.toBase58());
        const q = post.find(b=>b.owner===owner&&b.mint===MINT.toBase58());
        const a0 = p? Number(p.uiTokenAmount.uiAmountString||0):0;
        const a1 = q? Number(q.uiTokenAmount.uiAmountString||0):0;
        if (a1-a0!==0) console.log(`    ${short(owner)}: ${a0} → ${a1}  (${a1-a0>0?"+":""}${a1-a0})`);
      }
      const progs = [...new Set(tx.transaction.message.instructions.map(i=>i.program||i.programId?.toBase58?.()||"?"))];
      console.log("  programs:", progs.join(", "));
    } catch(e){ console.log("  detail error:", e.message); }
  }
})();
