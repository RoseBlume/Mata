#!/usr/bin/env python3
"""Dependency-free, reproducible Phase 0 catalog / declared-mapping audit.

IMPORTANT: Declared spellings are not evidence of successful shader compilation.
"""
from __future__ import annotations
import argparse, csv, hashlib, io, json, re, runpy, sys
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
BACKENDS = ('wgsl', 'glsl', 'hlsl', 'intel_opencl', 'nvidia_cuda', 'amd_hip')

def read_existing():
    raw = runpy.run_path(str(ROOT / 'tools' / 'generate_catalog.py'))['ops']
    return raw

def native_names(file):
    source = (ROOT / 'src' / file).read_text()
    start = source.index('pub fn spelling(')
    end = source.index('\nfn bad(', start)
    block = source[start:end]
    # spelling arms have the form Name=>"symbol" or Name|Name=>"symbol".
    names = {}
    for lhs, spelling in re.findall(r'([A-Z][A-Za-z0-9_]*(?:\s*\|\s*[A-Z][A-Za-z0-9_]*)*)\s*=>\s*"([^"]+)"', block):
        for name in re.split(r'\s*\|\s*', lhs):
            names[name] = spelling
    return names

def make_rows():
    ops=read_existing(); nvidia=native_names('nvidia.rs'); amd=native_names('amd.rs')
    rows=[]
    for idx, op in enumerate(ops):
        name, family, signature, mini, maxi, capability, stage, wgsl, glsl, hlsl, intel = op
        native = [wgsl,glsl,hlsl,intel,nvidia.get(name),amd.get(name)]
        if name in amd:
            native[-1] = amd[name]
        elif not name.startswith('Nvidia') and name in nvidia:
            native[-1] = nvidia[name] # HIP delegates to CUDA naming for shared operations.
        for backend, spelling in zip(BACKENDS,native):
            rows.append(dict(ordinal=idx,variant=name,canonical_name=name[:1].lower()+name[1:],family=family,signature=signature,arity_min=mini,arity_max=maxi,capability=capability,stage=stage,backend=backend,declared_spelling=spelling or '',status='declared_unverified' if spelling is not None else 'unmapped',compiler_checked='false',hardware_checked='false',notes='Name mapping only; emitter may reject signatures, types, or target.'))
    return rows

def output_files():
    rows=make_rows(); assert len(rows)==322*6,(len(rows),len(read_existing()))
    base=ROOT/'audit';base.mkdir(exist_ok=True)
    fields=list(rows[0])
    buf=io.StringIO(newline=''); writer=csv.DictWriter(buf,fieldnames=fields,lineterminator='\n');writer.writeheader();writer.writerows(rows)
    csv_body=buf.getvalue()
    (base/'backend_coverage.csv').write_text(csv_body)
    ops=read_existing()
    catalog_hash=hashlib.sha256('\n'.join(x[0] for x in ops).encode()).hexdigest()
    srcfiles=['src/catalog.rs','src/types.rs','src/emit.rs','src/intel.rs','src/nvidia.rs','src/amd.rs','src/dispatch.rs','src/ops.rs']
    manifest={
        'schema_version':1,'catalog_entries':len(ops),'backend_names':list(BACKENDS),'catalog_order_sha256':catalog_hash,
        'audit_policy':'Conservative static declared-spelling inventory; NOT emission or GPU support proof.',
        'sources':{p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in srcfiles},
        'source_references':[
          {'id':'cuda-guide-13.4.2','file':'cuda-programming-guide.pdf','edition':'13.4.2 (Sep 10 2026)','scope':'Programming guide (not exhaustive runtime signature reference)'},
          {'id':'amd-hip-rst-compilation','file':'AMD_HIP_Reference(1).pdf','edition':'RST compilation (Oct 16 2026)','scope':'HIP modules, math, complex and low-precision types; runtime Doxygen signatures missing'},
          {'id':'opencl-3-quick-reference','file':'opencl30-reference-guide.pdf','edition':'Rev. 0922 (2022)','scope':'Condensed OpenCL 3.0 host APIs and C builtins; optional features marked'},
        ],
        'declared_mapping_counts':{b:sum(x['status']=='declared_unverified' and x['backend']==b for x in rows) for b in BACKENDS},
        'unmapped_counts':{b:sum(x['status']=='unmapped' and x['backend']==b for x in rows) for b in BACKENDS},
        'validation':{'python_static':'run tools/phase0_audit.py --check','cargo_checks':'not executed by generator','gpu_compilation':'not executed by generator'}
    }
    (base/'source_manifest.json').write_text(json.dumps(manifest,indent=2,sort_keys=True)+'\n')
    report=['# Phase 0 — Static backend coverage (baseline)','','**322 frozen enum slots × 6 targets = 1,932 rows.** Every row is `declared_unverified` or `unmapped`. This is intentionally conservative: a name mapping is not a successful emitted call, compiler acceptance, supported overload, or hardware capability.','','| Target | Declared mapping (unverified) | Unmapped |','|---|---:|---:|']
    for b in BACKENDS: report.append(f"| `{b}` | {manifest['declared_mapping_counts'][b]} | {manifest['unmapped_counts'][b]} |")
    report += ['','## Interpret the statuses','','- `declared_unverified`: a target spelling is declared by the original registry/native spelling function; it may still fail type checking, render, shader compilation, or hardware validation.','- `unmapped`: no recognized static spelling; may be unsupported or require specialized lowering.','- There is no `verified` status in Phase 0; that requires representative argument types, expression rendering, compiler testing, and device capability checks in later phases.','','The CSV includes the operation family, exact signature strategy, arity, required capability/stage, source spelling, and target name for each operation. Native HIP spelling is inferred from its explicit mappings or documented CUDA-spelling delegation in the implementation.','','## Follow-up verification','', 'Perform per-signature emitter invocation tests, compiler syntax tests for each target version, and hardware conformance tests; record evidence separately rather than promoting mappings to `verified` based on name alone.','','## Frozen catalog fingerprint','','`'+catalog_hash+'` (SHA-256 of the 322 stable variant identifiers joined by newlines).','']
    (base/'BACKEND_COVERAGE.md').write_text('\n'.join(report))
    return rows,manifest,csv_body

def check():
    base=ROOT/'audit'
    old={p:(base/p).read_bytes() for p in ('backend_coverage.csv','source_manifest.json','BACKEND_COVERAGE.md')}
    rows,manifest,_=output_files()
    for p,b in old.items():
        if (base/p).read_bytes()!=b:
            print('STALE:',p,file=sys.stderr);return 1
    assert len(set(x['variant'] for x in rows))==322
    assert [x['variant'] for x in rows[::6]]==[x[0] for x in read_existing()]
    assert manifest['catalog_entries']==322
    print('PASS: baseline 322 slots, 6 backend statuses each, source hashes and generated audit current')
    return 0

if __name__=='__main__':
    parser=argparse.ArgumentParser();parser.add_argument('--check',action='store_true');args=parser.parse_args()
    if args.check:sys.exit(check())
    rows,manifest,_=output_files()
    print('Generated',len(rows),'coverage cells;',manifest['declared_mapping_counts'])
