# Idempotent patch: wire pulse.rs into the program.
eg_mod = 'programs/entropy-garden/src/eg/mod.rs'
lib = 'programs/entropy-garden/src/lib.rs'

m = open(eg_mod).read()
if 'pub mod pulse;' not in m:
    m = m.rstrip() + '\npub mod pulse;\n'
    open(eg_mod,'w').write(m); print("mod.rs: pulse declared")
else: print("mod.rs: already has pulse")

l = open(lib).read()
if 'use eg::pulse::*;' not in l:
    l = l.replace('use eg::thread::*;', 'use eg::thread::*;\nuse eg::pulse::*;')
if 'pub fn commit_pulse' not in l:
    # add after the metadata instruction (or before enter_maze if metadata absent)
    anchor = '    pub fn enter_maze('
    l = l.replace(anchor,
'''    pub fn commit_pulse(ctx: Context<CommitPulse>, predicted_band: u8, window_slots: u64, commit_slot: u64) -> Result<()> {
        eg::pulse::commit_pulse(ctx, predicted_band, window_slots, commit_slot)
    }
    pub fn resolve_pulse(ctx: Context<ResolvePulse>) -> Result<()> {
        eg::pulse::resolve_pulse(ctx)
    }

''' + anchor)
    open(lib,'w').write(l); print("lib.rs: pulse router added")
else: print("lib.rs: already has pulse router")

# verify
l2 = open(lib).read()
print("use stmt:", "use eg::pulse::*;" in l2)
print("commit_pulse:", "pub fn commit_pulse" in l2)
print("resolve_pulse:", "pub fn resolve_pulse" in l2)
