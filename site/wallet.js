// wallet.js — shared wallet connection for all Entropy Garden pages.
// Handles: X1 Wallet, Backpack, any Wallet Standard wallet.
// Mobile-aware: longer polling, correct app store links, persistent session.
// Usage: include this script, then call EGWallet.init(onConnected) on page load.

window.EGWallet = (function(){
  // ── internal state ───────────────────────────────────────────────
  const wallets = new Map();
  let _active = null;       // { pk, sign, name }
  let _onConnected = null;
  let _conn = null;

  function isMobile(){
    return /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
  }

  // ── Wallet Standard registration ─────────────────────────────────
  function registerWallet(w){
    try{
      if(!w?.features?.["standard:connect"]) return;
      const chains = w.chains||[];
      const ok = !chains.length ||
        chains.some(c=>{ const s=String(c); return s.startsWith("solana")||s.startsWith("x1"); });
      if(ok) wallets.set(w.name, w);
    }catch(e){}
  }
  window.addEventListener("wallet-standard:register-wallet", ev=>{
    if(ev.detail) registerWallet(ev.detail);
  });
  try{
    window.dispatchEvent(new CustomEvent("wallet-standard:app-ready",{
      detail:{ register: registerWallet }
    }));
  }catch(e){}

  // ── base58 helper ─────────────────────────────────────────────────
  function base58(bytes){
    const A="123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz";
    let x=0n; for(const b of bytes) x=(x<<8n)+BigInt(b);
    let s=""; while(x>0n){ s=A[Number(x%58n)]+s; x/=58n; }
    return s||"1";
  }

  // ── connect methods ───────────────────────────────────────────────
  async function connectStandard(w){
    const res = await w.features["standard:connect"].connect();
    const acct = res.accounts[0];
    const pk = new solanaWeb3.PublicKey(acct.publicKey ?? acct.address);
    const sast = w.features["solana:signAndSendTransaction"];
    const st   = w.features["solana:signTransaction"];
    const allChains = acct.chains?.length ? acct.chains : (w.chains||[]);
    const chain = allChains.find(c=>String(c).startsWith("x1")) ?? "solana:mainnet";
    return {
      pk, name: w.name,
      sign: async tx => {
        tx.feePayer = pk;
        tx.recentBlockhash = (await _conn.getLatestBlockhash()).blockhash;
        if(sast){
          const ser = tx.serialize({requireAllSignatures:false,verifySignatures:false});
          const [out] = await sast.signAndSendTransaction({transaction:ser,account:acct,chain});
          return typeof out.signature==="string" ? out.signature : base58(out.signature);
        }
        const [signed] = await st.signTransaction({
          transaction: tx.serialize({requireAllSignatures:false,verifySignatures:false}),
          account: acct
        });
        return _conn.sendRawTransaction(signed.signedTransaction);
      }
    };
  }

  async function connectBackpack(){
    const bp = window.backpack?.solana ?? window.backpack ?? window.xnft?.solana;
    if(!bp) throw new Error("Backpack not found");
    const resp = await bp.connect();
    const pk = new solanaWeb3.PublicKey((resp?.publicKey ?? bp.publicKey).toString());
    return {
      pk, name: "Backpack",
      sign: async tx => {
        tx.feePayer = pk;
        tx.recentBlockhash = (await _conn.getLatestBlockhash()).blockhash;
        const res = await bp.signAndSendTransaction(tx);
        return res?.signature ?? res;
      }
    };
  }

  // ── discover wallets (mobile-aware) ───────────────────────────────
  async function discoverWallets(targetName){
    // poll longer on mobile — wallets inject slower
    const polls = isMobile() ? 16 : 8;
    const delay = isMobile() ? 300 : 250;
    for(let i=0; i<polls; i++){
      try{
        window.dispatchEvent(new CustomEvent("wallet-standard:app-ready",{
          detail:{ register: registerWallet }
        }));
      }catch(e){}
      // also check direct window properties
      if(window.x1wallet && !wallets.has("X1 Wallet")){
        try{ registerWallet(window.x1wallet); }catch(e){}
      }
      if(targetName && wallets.has(targetName)) return;
      if(!targetName && (wallets.size > 0 || window.backpack)) return;
      await new Promise(r=>setTimeout(r, delay));
    }
  }

  // ── build the wallet options list ─────────────────────────────────
  function buildOptions(){
    const opts = [];
    const seen = new Set([...wallets.keys()].map(n=>n.toLowerCase()));
    for(const [name, w] of wallets){
      opts.push({ label: name, go: ()=> connectStandard(w) });
    }
    if((window.backpack || window.xnft?.solana) && !seen.has("backpack")){
      opts.push({ label: "Backpack", go: ()=> connectBackpack() });
    }
    return opts;
  }

  // ── show wallet picker dialog ──────────────────────────────────────
  function showPicker(dialogEl, listEl){
    const opts = buildOptions();
    listEl.innerHTML = "";
    if(!opts.length){
      const mobile = isMobile();
      if(mobile){
        // On mobile, wallets only inject inside their in-app browser.
        const url = "entropygarden.xyz" + location.pathname;
        listEl.innerHTML = `
          <p style="color:#E8E4DA;font-size:14px;margin-bottom:12px;line-height:1.5">
            📱 <b>On mobile, open this page inside your wallet app.</b>
          </p>
          <p style="color:#8A9A84;font-size:13px;margin-bottom:14px;line-height:1.6">
            Mobile browsers can't see wallet apps directly. To connect:
          </p>
          <ol style="color:#8A9A84;font-size:13px;line-height:1.7;padding-left:20px;margin-bottom:14px">
            <li>Open your <b style="color:#9FD27E">X1 Wallet</b> or <b style="color:#C8A06A">Backpack</b> app</li>
            <li>Find the in-app <b>Browser</b> tab</li>
            <li>Go to <b style="color:#E8E4DA">${url}</b></li>
            <li>Connect there — it just works ✓</li>
          </ol>
          <button onclick="navigator.clipboard?.writeText('https://${url}').then(()=>{this.textContent='✓ link copied — paste in wallet browser'})"
            style="display:block;width:100%;padding:12px 16px;border:1px solid #9FD27E;background:transparent;color:#9FD27E;font-size:13px;cursor:pointer;font-family:inherit">
            📋 copy link for wallet browser
          </button>`;
      } else {
        listEl.innerHTML = `
          <p style="color:#8A9A84;font-size:13px;margin-bottom:14px">
            No wallet detected. Install a wallet extension to continue.
          </p>
          <a href="https://chromewebstore.google.com/detail/x1-wallet/kcfmcpdmlchhbikbogddmgopmjbflnae" target="_blank"
            style="display:block;padding:12px 16px;border:1px solid #243224;color:#9FD27E;font-size:14px;margin-bottom:10px;text-decoration:none">
            ⚡ Install X1 Wallet →
          </a>
          <a href="https://backpack.app/download" target="_blank"
            style="display:block;padding:12px 16px;border:1px solid #243224;color:#C8A06A;font-size:14px;text-decoration:none">
            🎒 Install Backpack →
          </a>`;
      }
    } else {
      opts.forEach(o=>{
        const b = document.createElement("button");
        b.style.cssText="display:block;width:100%;text-align:left;margin-bottom:10px;padding:12px 16px;font-size:14px;border:1px solid #243224;background:transparent;color:#9FD27E;cursor:pointer;font-family:inherit";
        b.textContent = o.label;
        b.onmouseenter = ()=> b.style.background="#1E2A1E";
        b.onmouseleave = ()=> b.style.background="transparent";
        b.onclick = async ()=>{
          b.textContent = "connecting…";
          try{
            _active = await o.go();
            _saveSession(_active.name, _active.pk.toBase58());
            try{ dialogEl.close(); }catch(e){}
            if(_onConnected) _onConnected(_active);
          }catch(e){
            b.textContent = o.label + " (failed — try again)";
          }
        };
        listEl.appendChild(b);
      });
    }
    if(!dialogEl.open){ try{ dialogEl.showModal(); }catch(e){ try{ dialogEl.setAttribute("open",""); }catch(_){} } }
  }

  // ── session persistence ───────────────────────────────────────────
  // Store both wallet name AND pubkey so we can show the address
  // instantly before the wallet is ready, and verify on reconnect.
  function _saveSession(name, pubkey){
    try{
      localStorage.setItem("eg_wallet", name);
      localStorage.setItem("eg_pubkey", pubkey);
    }catch(e){}
  }
  function _loadSession(){
    try{
      return {
        name:   localStorage.getItem("eg_wallet"),
        pubkey: localStorage.getItem("eg_pubkey"),
      };
    }catch(e){ return {}; }
  }
  function _clearSession(){
    try{ localStorage.removeItem("eg_wallet"); localStorage.removeItem("eg_pubkey"); }catch(e){}
  }

  // ── auto-reconnect ────────────────────────────────────────────────
  async function tryAutoConnect(){
    const { name, pubkey } = _loadSession();
    if(!name) return false;
    await discoverWallets(name);
    // try Wallet Standard wallet
    if(wallets.has(name)){
      try{
        _active = await connectStandard(wallets.get(name));
        if(_onConnected) _onConnected(_active);
        return true;
      }catch(e){ _clearSession(); return false; }
    }
    // try Backpack
    if(name === "Backpack" && (window.backpack || window.xnft?.solana)){
      try{
        _active = await connectBackpack();
        if(_onConnected) _onConnected(_active);
        return true;
      }catch(e){ _clearSession(); return false; }
    }
    return false;
  }

  // ── public API ────────────────────────────────────────────────────
  return {
    // Call once per page with the RPC connection and an onConnected callback.
    init(conn, onConnected){
      _conn = conn;
      _onConnected = onConnected;
      // kick off wallet discovery immediately
      try{
        window.dispatchEvent(new CustomEvent("wallet-standard:app-ready",{
          detail:{ register: registerWallet }
        }));
      }catch(e){}
      if(window.x1wallet) try{ registerWallet(window.x1wallet); }catch(e){}
    },

    // Returns the saved session pubkey immediately (for showing address before reconnect).
    getSavedPubkey(){
      return _loadSession().pubkey || null;
    },

    // Attempt silent auto-reconnect. Call on page load.
    async autoConnect(){
      return tryAutoConnect();
    },

    // Show the wallet picker. Pass dialog and list DOM elements.
    async showPicker(dialogEl, listEl){
      if(!dialogEl || !listEl){ console.error("EGWallet: dialog/list element missing"); return; }
      // open immediately with a detecting state (feels instant on mobile)
      listEl.innerHTML = '<p style="color:#8A9A84;font-size:13px;padding:8px 0">detecting wallets…</p>';
      try{ dialogEl.showModal(); }catch(e){ try{ dialogEl.setAttribute("open",""); }catch(_){} }
      // now discover and populate
      await discoverWallets();
      showPicker(dialogEl, listEl);
    },

    // The active wallet object { pk, sign, name }, or null.
    get active(){ return _active; },

    // Disconnect.
    disconnect(){
      _active = null;
      _clearSession();
    },

    isMobile,
    base58,
  };
})();
