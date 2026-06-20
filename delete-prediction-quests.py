#!/usr/bin/env python3
# Surgically remove Sky-Reading and Pulse from the program.
# Idempotent + makes backups. Run from /home/entropy-garden.
import os, re, shutil

SRC = "programs/entropy-garden/src"
mod_path = f"{SRC}/eg/mod.rs"
lib_path = f"{SRC}/lib.rs"
skyread_path = f"{SRC}/eg/skyread.rs"
pulse_path = f"{SRC}/eg/pulse.rs"

def backup(p):
    if os.path.exists(p) and not os.path.exists(p + ".bak-predict"):
        shutil.copy(p, p + ".bak-predict")

# ── 1. eg/mod.rs: remove module declarations ──
backup(mod_path)
m = open(mod_path).read()
before = m
m = re.sub(r'^\s*pub mod skyread;\s*\n', '', m, flags=re.MULTILINE)
m = re.sub(r'^\s*pub mod pulse;\s*\n', '', m, flags=re.MULTILINE)
open(mod_path, 'w').write(m)
print(f"mod.rs: skyread mod removed: {'pub mod skyread' not in m}")
print(f"mod.rs: pulse mod removed:   {'pub mod pulse' not in m}")

# ── 2. lib.rs: remove imports + router fns ──
backup(lib_path)
l = open(lib_path).read()

# remove the use statements
l = re.sub(r'^\s*use eg::skyread::\*;\s*\n', '', l, flags=re.MULTILINE)
l = re.sub(r'^\s*use eg::pulse::\*;\s*\n', '', l, flags=re.MULTILINE)

# remove the 4 router fns. Each is a pub fn { ... } block — match by name.
def remove_fn(src, fn_name):
    # find "pub fn <name>(" and remove through its closing brace at the same indent
    pat = re.compile(r'\n[ \t]*pub fn ' + re.escape(fn_name) + r'\s*\([^)]*\)[^\{]*\{', re.DOTALL)
    mt = pat.search(src)
    if not mt:
        return src, False
    start = mt.start()
    # walk braces from the opening { of this fn
    i = src.index('{', mt.start())
    depth = 0
    while i < len(src):
        if src[i] == '{': depth += 1
        elif src[i] == '}':
            depth -= 1
            if depth == 0:
                break
        i += 1
    end = i + 1
    return src[:start] + src[end:], True

for fn in ["commit_forecast", "resolve_forecast", "commit_pulse", "resolve_pulse"]:
    l, ok = remove_fn(l, fn)
    print(f"lib.rs: removed {fn}: {ok}")

open(lib_path, 'w').write(l)

# verify no dangling references remain in lib.rs
remaining = [w for w in ["skyread","CommitForecast","ResolveForecast","CommitPulse","ResolvePulse","commit_pulse","resolve_pulse","commit_forecast","resolve_forecast"] if w in l]
print(f"lib.rs dangling refs: {remaining if remaining else 'NONE ✓'}")

# ── 3. delete the .rs files (move to backup) ──
for p in [skyread_path, pulse_path]:
    if os.path.exists(p):
        shutil.move(p, p + ".deleted")
        print(f"deleted (→ .deleted): {p}")
    else:
        print(f"already gone: {p}")

# ── 4. confirm error.rs is UNTOUCHED (bloom needs BadForecastWindow/ForecastNotReady) ──
err = open(f"{SRC}/error.rs").read()
print(f"error.rs BadForecastWindow kept: {'BadForecastWindow' in err}")
print(f"error.rs ForecastNotReady kept:  {'ForecastNotReady' in err}")

print("\nDONE. Now run: anchor build")
print("Backups: *.bak-predict (mod.rs, lib.rs) and *.deleted (the .rs files)")
