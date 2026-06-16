# Idempotent patch: wire carbon.rs into the program.
eg_mod = 'programs/entropy-garden/src/eg/mod.rs'
lib = 'programs/entropy-garden/src/lib.rs'

m = open(eg_mod).read()
if 'pub mod carbon;' not in m:
    m = m.rstrip() + '\npub mod carbon;\n'
    open(eg_mod,'w').write(m); print("mod.rs: carbon declared")
else: print("mod.rs: already has carbon")

l = open(lib).read()
if 'use eg::carbon::*;' not in l:
    l = l.replace('use eg::pulse::*;', 'use eg::pulse::*;\nuse eg::carbon::*;')
if 'pub fn init_carbon_sink' not in l:
    anchor = '    pub fn enter_maze('
    l = l.replace(anchor,
'''    pub fn init_carbon_sink(ctx: Context<InitCarbonSink>) -> Result<()> {
        eg::carbon::init_carbon_sink(ctx)
    }
    pub fn sequester(ctx: Context<Sequester>) -> Result<()> {
        eg::carbon::sequester(ctx)
    }
    pub fn harvest_carbon(ctx: Context<HarvestCarbon>) -> Result<()> {
        eg::carbon::harvest_carbon(ctx)
    }

''' + anchor)
    open(lib,'w').write(l); print("lib.rs: carbon router added")
else: print("lib.rs: already has carbon router")

l2 = open(lib).read()
print("use stmt:", "use eg::carbon::*;" in l2)
print("init_carbon_sink:", "pub fn init_carbon_sink" in l2)
print("sequester:", "pub fn sequester" in l2)
print("harvest_carbon:", "pub fn harvest_carbon" in l2)
