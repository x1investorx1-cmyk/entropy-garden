p = 'programs/entropy-garden/src/eg/bloom.rs'
s = open(p).read()

# Fix: only consider blooms with stakers as potential winners
old = '''    // winner = bloom whose signal grew the most (absolute delta)
    let mut winner: u8 = 0;
    let mut best_delta: u32 = 0;
    for i in 0..BLOOM_COUNT {
        let base = r.baseline_bps[i] as u32;
        let cur  = current[i] as u32;
        let delta = cur.saturating_sub(base);
        if delta > best_delta { best_delta = delta; winner = i as u8; }
    }'''

new = '''    // winner = bloom whose signal grew the most, among blooms that HAVE backers
    // (a bloom with 0 stakers cannot win — the pool would be unclaimable)
    let mut winner: u8 = u8::MAX;
    let mut best_delta: u32 = 0;
    for i in 0..BLOOM_COUNT {
        if r.total_staked[i] == 0 { continue; } // skip unbacked blooms
        let base = r.baseline_bps[i] as u32;
        let cur  = current[i] as u32;
        let delta = cur.saturating_sub(base);
        if winner == u8::MAX || delta > best_delta {
            best_delta = delta;
            winner = i as u8;
        }
    }
    // require at least one backed bloom
    require!(winner != u8::MAX, GardenError::PoolExhausted);'''

if old in s:
    s = s.replace(old, new)
    open(p,'w').write(s)
    print("winner fix applied: only backed blooms can win")
else:
    print("PATTERN NOT FOUND")
    
# verify
print("u8::MAX check:", "winner == u8::MAX" in s)
print("skip unbacked:", "total_staked[i] == 0" in s)
