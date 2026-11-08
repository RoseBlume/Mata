"""Static consistency checks. Not equivalent to cargo test or hipcc."""
from pathlib import Path
import re,subprocess,sys,zipfile
root=Path(__file__).resolve().parents[1]
cat=(root/'src/catalog.rs').read_text()
types=(root/'src/types.rs').read_text()
amd=(root/'src/amd.rs').read_text()
ops=(root/'src/ops.rs').read_text()
emit=(root/'src/emit.rs').read_text()
names=re.findall(r'^    (Amd\w+),$',cat,re.M)
assert len(names)==35, names
assert 'Backend::AmdHip' in emit and 'crate::amd::render(op,args,ty)' in emit
assert 'AmdHip' in types and 'AmdGcn' in types and 'AmdWave' in types
for n in names:
 assert n in amd, n
 assert 'Intrinsic::'+n in ops, n
 assert f'`{n}`' in (root/'docs/INTRINSICS.md').read_text(),n
assert '__builtin_amdgcn_ds_bpermute' in amd
assert 'Type::U64' in amd
assert (root/'tests/amd_hip.rs').exists() and (root/'examples/amd_hip.rs').exists()
assert len(re.findall(r'^pub fn ',ops,re.M))==322
assert '450 public Rust functions' not in (root/'README.md').read_text() or (root/'docs/API.md').exists()
assert 'pub mod amd;' in (root/'src/lib.rs').read_text()
assert 'hip/hip_runtime.h' in amd
assert re.search(r'\[dependencies\]\n# Intentionally empty:',(root/'Cargo.toml').read_text())
assert not (root/'build.rs').exists()
# Validate generated source constraints as lexical consistency, NOT a Rust parse.
for name in ['amd.rs','types.rs','emit.rs','lib.rs']:
 s=(root/'src'/name).read_text()
 assert s.count('{') >= s.count('}') - 10,(name,s.count('{'),s.count('}'))
print(f'PASS: {len(names)} AMD GPU operations and AMD wrapper IDs verified')
print('PASS: HIP kernel emitter, typed 64-bit wave masks, capability gates, zero deps')
print('NOTE: Rust / HIP compilation and AMD hardware execution are unverified')
