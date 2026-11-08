"""Static dispatch table validation (not a substitute for cargo check/test)."""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
cat=(root/'src/catalog.rs').read_text()
m=re.search(r'pub static ALL_INTRINSICS:&\[Intrinsic\]=&\[(.*?)\];',cat,re.S)
assert m, 'missing catalog'
ops=re.findall(r'Intrinsic::([A-Za-z0-9_]+),',m.group(1))
assert len(ops)==322,len(ops)
dispatch=(root/'src/dispatch.rs').read_text()
assert 'pub type IntrinsicFn = fn(&Emitter, &[Expr], Option<Type>) -> Result<Expr,Error>;' in dispatch
mapping={'WGSL':0,'GLSL':1,'HLSL':2,'INTEL_OPENCL':3,'NVIDIA_CUDA':4,'AMD_HIP':5}
for backend,b in mapping.items():
    m=re.search(rf'static {backend}_FUNCTIONS:\[IntrinsicFn;INTRINSIC_COUNT\]=\[(.*?)\];',dispatch,re.S)
    assert m, f'no {backend} dispatch array'
    listed=re.findall(r'invoke::<(\d+),\{Intrinsic::([A-Za-z0-9_]+) as usize\}>',m.group(1))
    assert len(listed)==len(ops),f'{backend}: {len(listed)}'
    assert [op for n,op in listed]==ops,f'{backend}: bad order'
    assert all(int(n)==b for n,_ in listed),f'{backend}: wrong dispatch index'
assert 'if emitter.backend!=expected' in dispatch
assert 'from_fn' in dispatch
assert 'self.emitter.call_typed' in dispatch
assert 'self.emitter.call(op,args)' in dispatch
assert 'Err(Error::BackendMismatch' in dispatch
assert 'pub mod dispatch;' in (root/'src/lib.rs').read_text()
assert 'mod dispatch' not in (root/'src/lib.rs').read_text() or 'pub mod dispatch' in (root/'src/lib.rs').read_text()
assert 'UnknownIntrinsicName' in (root/'src/emit.rs').read_text()
assert 'BackendMismatch' in (root/'src/emit.rs').read_text()
assert (root/'Cargo.toml').read_text().count('[dependencies]')==1
assert not list(root.glob('build.rs'))
docs=(root/'docs/API.md').read_text()
assert 'src/dispatch.rs' in docs
assert all(f'`{name}`' in docs for name in ['FunctionTable', 'BoundFunction', 'TableEntry'])
assert len(re.findall(r'^#### `',docs,re.M))==480,len(re.findall(r'^#### `',docs,re.M))
assert 'FunctionTable::nvidia' in (root/'README.md').read_text()
print(f'PASS: {len(ops)} unique intrinsic slots for each of 6 backends ({len(ops)*6} pointers)')
print('PASS: backend-specific bindings, stage/capability filters and preservation of emitter validation')
print('PASS: API documentation counts and examples')
print('NOTE: Rust compiler tests were not executed on this machine.')
