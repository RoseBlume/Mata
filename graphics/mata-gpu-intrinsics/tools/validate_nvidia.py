"""Dependency-free static consistency checker for the NVIDIA backend.

This cannot replace cargo test, nvcc/NVRTC compilation, or device testing.
"""
from pathlib import Path
import re
root=Path(__file__).resolve().parents[1]
cat=(root/'src/catalog.rs').read_text()
nv=(root/'src/nvidia.rs').read_text()
docs=(root/'docs/INTRINSICS.md').read_text()
readme=(root/'README.md').read_text()
types=(root/'src/types.rs').read_text()
emit=(root/'src/emit.rs').read_text()
known=set(re.findall(r'^    (\w+),$',cat,re.M))
assert 'NvidiaCuda' in types and 'NvidiaWarp' in types and 'NvidiaSm80' in types
assert 'crate::nvidia::spelling(self.op)' in cat
assert 'crate::nvidia::render(op,args,ty,self.backend)' in emit
assert 'cargo run --example nvidia_cuda' in readme
assert 'NVIDIA CUDA C++' in docs
assert len(known)>=287
mapped={}
for m in re.finditer(r'([A-Za-z][A-Za-z0-9_|]*)\s*=>\s*"([^"]+)"',nv.split('fn bad(',1)[0]):
    for n in m.group(1).split('|'):
        mapped[n]=m.group(2)
assert 75<=len(mapped)<=150, len(mapped)
assert set(mapped) <= known, sorted(set(mapped)-known)
assert sum(f'| `{name}` |' in docs for name in mapped)==len(mapped)
for name in ('NvidiaBallotSync','NvidiaAnySync','NvidiaAllSync','NvidiaShuffleSync',
             'NvidiaShuffleUpSync','NvidiaShuffleDownSync','NvidiaShuffleXorSync',
             'NvidiaMatchAnySync','NvidiaSyncWarp','NvidiaActiveMask',
             'NvidiaReduceAddSync','NvidiaReduceMinSync','NvidiaReduceMaxSync',
             'NvidiaReduceAndSync','NvidiaReduceOrSync','NvidiaReduceXorSync'):
    assert name in known and name in mapped
for f in ('tests/nvidia_cuda.rs','examples/nvidia_cuda.rs','docs/NVIDIA_CUDA.md'):
    assert (root/f).exists(),f
cargo=(root/'Cargo.toml').read_text()
assert '[dependencies]\n# Intentionally empty:' in cargo
print(f'PASS: CUDA source mappings: {len(mapped)}, operation catalog: {len(known)}')
print('PASS: CUDA capability IDs, emitters, docs and example exist')
print('PASS: no dependency changes and no GPU SDK required to build Rust')
print('NOTE: these checks do not compile Rust or execute any CUDA code')
