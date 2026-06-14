const { Connection, PublicKey } = require("@solana/web3.js");
const conn = new Connection("https://rpc.testnet.x1.xyz", "confirmed");
const PID = new PublicKey("8gTX3w2mAkKhGip9Mmvhb3gkcETugkfLEvmT4BNTh1By");
const te = s => new TextEncoder().encode(s);
const pda = (seeds) => PublicKey.findProgramAddressSync(seeds, PID)[0];

(async () => {
  const checks = {
    config:   pda([te("config")]),
    pool:     pda([te("compost")]),
    region0:  pda([te("region"), new Uint8Array([0,0])]),
    region1:  pda([te("region"), new Uint8Array([1,0])]),
    feed0:    pda([te("weather"), new Uint8Array([0,0])]),
    feed1:    pda([te("weather"), new Uint8Array([1,0])]),
    eg_config:pda([te("eg_config")]),
    eg_mint:  pda([te("eg_mint")]),
  };
  console.log("=== TESTNET program state ===");
  for (const [name, key] of Object.entries(checks)) {
    const info = await conn.getAccountInfo(key);
    console.log(`${name.padEnd(10)} ${info ? "EXISTS ("+info.data.length+" bytes)" : "MISSING"}  ${key.toBase58()}`);
  }
  // any plots?
  const plots = await conn.getProgramAccounts(PID, { filters:[{memcmp:{offset:0, bytes:"6AScpHJ8oWP8"}}] }).catch(()=>[]);
  console.log("\nplots found (rough):", plots.length);
})().catch(e => console.error("ERR:", e.message));
