#!/usr/bin/env python3
"""Compile actual emitted Phase 3 kernels, requiring no GPU hardware.

Usage: python3 tools/phase3_compiler_smoke.py
Options: --require-cuda / --require-hip / --require-opencl
For CUDA/HIP, the respective installed toolchain and SDK headers are needed.
OpenCL syntax is checked with clang's bundled OpenCL C frontend.
"""
import argparse
import pathlib
import shutil
import subprocess
import sys
import tempfile

BASE=pathlib.Path(__file__).resolve().parents[1]
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument('--require-cuda',action='store_true')
parser.add_argument('--require-hip',action='store_true')
parser.add_argument('--require-opencl',action='store_true')
a=parser.parse_args()
if not shutil.which('cargo'):
    sys.exit('ERROR: cargo is required to generate and check real kernel output')
with tempfile.TemporaryDirectory(prefix='mata-gpu-phase3-') as tmp:
    out=pathlib.Path(tmp)
    commands=[['cargo','run','--quiet','--example','phase3_kernel_suite','--',str(out)]]
    providers={
        'opencl': ('clang', ['clang','-x','cl','-cl-std=CL3.0','-fsyntax-only']),
        'cuda': ('nvcc', ['nvcc','-std=c++14','-c','-o']),
        'hip': ('hipcc', ['hipcc','-std=c++14','-fsyntax-only'])
    }
    subprocess.run(commands[0],cwd=BASE,check=True)
    for family,(executable,args) in providers.items():
        available=shutil.which(executable)
        require=getattr(a,'require_'+family)
        if not available:
            if require:sys.exit(f'ERROR: {executable} was required but unavailable')
            print(f'SKIP {family}: {executable} not installed')
            continue
        for name in ('saxpy','reduction','stencil','histogram'):
            suffix={'opencl':'cl','cuda':'cu','hip':'hip.cpp'}[family]
            source=out/(name+'.'+suffix)
            if family=='cuda':
                argv=args+[str(out/(name+'.o')),str(source)]
            elif family=='hip':
                argv=args+[str(source)]
            else:argv=args+[str(source)]
            print('CHECK',family,name,flush=True)
            subprocess.run(argv,cwd=BASE,check=True)
    print('Phase 3 installed-compiler checks finished. SKIPs are not passes.')
