#!/usr/bin/env python3
"""Static preservation and Phase 2 API/source sanity checks (NOT a Rust compiler)."""
from pathlib import Path
import re, sys, runpy

root=Path(__file__).resolve().parent.parent
source=(root/'src/ir_safety.rs').read_text()
required=['pub enum Feature','pub struct DeviceLimits','pub struct TargetProfile',
    'pub enum DiagnosticKind','pub struct Diagnostic','pub struct AtomicSpec',
    'pub struct MemoryFenceSpec','pub fn required_features','pub fn validate_for',
    'pub fn emit_for','DivergentBarrier','UnknownDeviceLimit','InvalidAtomicOrder',
    'fn analyze_calls','fn analyze_block','fn validate_kernel_limits']
for needle in required:
    if needle not in source:
        sys.exit(f'FAIL: Phase 2 implementation missing {needle}')
assert 'include!("ir_safety.rs")' in (root/'src/ir.rs').read_text()
assert 'forbid(unsafe_code)' in (root/'src/lib.rs').read_text()
assert not (root/'Cargo.lock').exists(), 'source-only crate should not bundle Cargo.lock'
manifest=runpy.run_path(str(root/'tools/phase0_audit.py'))
rows=manifest['make_rows']()
assert len(rows)==1932 and len(set(row['variant'] for row in rows))==322
text=(root/'tests/phase2_validation.rs').read_text()
tests=re.findall(r'#\[test\]\s*fn\s+([A-Za-z_0-9]+)',text)
assert len(tests)>=20, f'Need 20 or more Phase 2 regression tests, got {len(tests)}'
assert '[dependencies]' in (root/'Cargo.toml').read_text()
dependencies=(root/'Cargo.toml').read_text().split('[dependencies]',1)[1]
assert not re.search(r'^\s*[\w-]+\s*=\s*',dependencies,re.M), 'Unexpected new dependency'
print(f'PASS: 322 frozen intrinsics, 1,932 original backend mapping cells, {len(tests)} Phase 2 tests, no new dependencies')
print('NOTE: this is structural validation; Rust compilation and GPU feature queries are not performed')
