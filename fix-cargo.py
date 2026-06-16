p = 'programs/entropy-garden/Cargo.toml'
s = open(p).read()
# remove the mpl-token-metadata dependency entirely — not needed since
# metadata.rs uses raw invoke_signed, no mpl types
import re
s = re.sub(r'\nmpl-token-metadata = \{[^}]+\}\n?', '\n', s)
# also remove if it's on a single line without braces
s = re.sub(r'mpl-token-metadata = "[^"]+"\n?', '', s)
open(p,'w').write(s)
print("mpl dependency removed:", 'mpl-token-metadata' not in s)
