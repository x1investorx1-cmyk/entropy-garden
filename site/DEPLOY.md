# Deploying entropygarden.xyz

## 1. Add to the repo (on the server)
    mkdir -p /home/entropy-garden/site
    # upload index.html into that folder (scp from your PC), then:
    cd /home/entropy-garden
    git add site && git commit -m "observatory v1: live weather frontend" && git push

## 2. Vercel
- vercel.com → Add New → Project → Import `entropy-garden`
- Root Directory: `site`
- Framework Preset: **Other** (no build command, no output dir — it's static)
- Deploy. You get a *.vercel.app URL immediately — open it, the weather should be live.

## 3. Domain
- Project → Settings → Domains → add `entropygarden.xyz` (and `www.entropygarden.xyz`)
- Vercel shows the DNS records (one A record: 76.76.21.21, one CNAME for www) —
  set those at your registrar. SSL is automatic. Propagation: minutes to an hour.

## 4. Updating the site forever after
Edit site/index.html → git push → live in ~60 seconds.

## Notes
- The page reads mainnet directly (getMultipleAccounts every 20s) — no backend,
  no API keys. Your server's only job remains the crank.
- If the public RPC ever rate-limits the page under heavy traffic, swap RPC
  const at the top of index.html for a dedicated endpoint.
