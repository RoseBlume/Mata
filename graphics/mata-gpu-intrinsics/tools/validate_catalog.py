"""Reproducible dependency-free static checks (not a substitute for cargo test)."""
import re,runpy
from pathlib import Path
root=Path(__file__).resolve().parents[1]
ops=runpy.run_path(str(root/'tools/generate_catalog.py'))['ops']
assert len(ops)>=250
names=[x[0] for x in ops]
assert len(names)==len(set(names)), 'duplicate intrinsic ID'
cat=(root/'src/catalog.rs').read_text()
w=(root/'src/ops.rs').read_text()
api=(root/'docs/API.md').read_text()
doc=(root/'docs/INTRINSICS.md').read_text()
variants=re.findall(r'^    (\w+),$',cat,re.M)
assert variants==names,'enum/catalog ordering mismatch'
wrappers=re.findall(r'^pub fn (\w+)\(',w,re.M)
assert len(wrappers)==len(ops) and len(set(wrappers))==len(ops)
assert sum(x[-1] is not None for x in ops)>=90
for name in names:
    assert f'`{name}`' in doc
    assert f'Intrinsic::{name}' in w
    assert f'Intrinsic::{name}' in cat
expected_api=sum(len(re.findall(r'\bpub\s+(?:const\s+)?fn\s+\w+\s*\(', (root/'src'/filename).read_text())) for filename in ['catalog.rs','types.rs','emit.rs','dispatch.rs','intel.rs','nvidia.rs','amd.rs','ops.rs','cpu.rs'])
assert len(re.findall(r'^#### `',api,re.M))==expected_api,(expected_api,'API signature count mismatch')
assert 'Public function; see Rustdoc for behavior' not in api
assert 'Intel OpenCL C' in doc and 'Intel OpenCL' in api
assert 'NVIDIA CUDA C++' in doc and 'NVIDIA CUDA C++' in api
assert 'AMD HIP C++' in doc and 'AMD HIP C++' in api
assert (root/'docs/AMD_HIP.md').exists()
assert len(re.findall(r'\bNvidia[A-Za-z]+,', cat))>=16
cargo=(root/'Cargo.toml').read_text()
assert '[dependencies]' in cargo
assert not re.search(r'^(?!#)\s*[a-z_-]+\s*=\s*\{.*version.*\}',cargo,re.M)
assert 'build.rs' not in [f.name for f in root.iterdir()]
assert '#![forbid(unsafe_code)]' in (root/'src/lib.rs').read_text()
print(f'PASS: {len(ops)} intrinsic variants and {len(wrappers)} documented wrappers')
print('PASS: generated Markdown reflects catalog and source')
print('PASS: no Cargo dependencies/build scripts and unsafe forbidden')
print('NOTE: cargo check/test and actual GPU shader validation are separate checks.')
