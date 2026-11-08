#!/usr/bin/env python3
"""Compile exact Phase 5 emitted sources with available native SDKs.

Run with: python3 tools/phase56_compiler_smoke.py [--require-opencl] ...
The OpenCL built-ins reference fixture is checked separately using clang.
"""
import argparse,pathlib,shutil,subprocess,sys,tempfile
base=pathlib.Path(__file__).resolve().parents[1]
p=argparse.ArgumentParser();p.add_argument('--require-opencl',action='store_true');p.add_argument('--require-cuda',action='store_true');p.add_argument('--require-hip',action='store_true');args=p.parse_args()
if not shutil.which('cargo'):
    sys.exit('ERROR: a Rust compiler and Cargo are required to generate the actual emitted source')
with tempfile.TemporaryDirectory(prefix='mata-phase56-') as temp:
    out=pathlib.Path(temp)
    subprocess.run(['cargo','run','--quiet','--example','phase56_kernel_suite','--',str(out)],cwd=base,check=True)
    for target,tool,suffix,commands in [
        ('opencl','clang','cl',lambda f:['clang','-x','cl','-cl-std=CL3.0','-fsyntax-only',str(f)]),
        ('cuda','nvcc','cu',lambda f:['nvcc','-std=c++14','-c','-o',str(out/(f.stem+'.o')),str(f)]),
        ('hip','hipcc','hip.cpp',lambda f:['hipcc','-std=c++14','-fsyntax-only',str(f)]),
    ]:
        if not shutil.which(tool):
            if getattr(args,'require_'+target):sys.exit('ERROR: required tool '+tool+' missing')
            print('SKIP',target,':',tool,'is not installed');continue
        for name in ['log1p','erf']:
            filename=out/(name+'.'+suffix)
            print('CHECK',target,filename.name,flush=True)
            subprocess.run(commands(filename),check=True,cwd=base)
    if shutil.which('clang'):
        subprocess.run(['clang','-x','cl','-cl-std=CL3.0','-fsyntax-only',str(base/'tests/phase56_numeric_smoke.cl')],check=True)
