#!/usr/bin/env python3
"""Opt-in native source compiler checks; toolchain and architecture required.

Run after building example source with Cargo; no GPU needs to be present to
compile if the installed toolkit supports the requested target architecture.
"""
from pathlib import Path
import os,shutil,subprocess,tempfile,sys
root=Path(__file__).resolve().parents[2]
if not shutil.which('cargo'):
    sys.exit('SKIP: cargo not installed; cannot generate vendor source')
def generate(target):
    example=subprocess.run(['cargo','run','--quiet','-p','mata-gpu-intrinsics','--example','phase9_vendor','--',target],cwd=root,text=True,capture_output=True)
    if example.returncode:
        print(example.stderr,file=sys.stderr)
        sys.exit(example.returncode)
    return example.stdout
with tempfile.TemporaryDirectory() as d:
    temp=Path(d)
    cuda_file=temp/'cuda.cu'
    cuda_file.write_text('#include <cuda_runtime.h>\n'+generate('cuda')+'\nextern "C" __global__ void smoke(unsigned* out) { unsigned v = _mata_cuda_lane_id(); _mata_cuda_cluster_sync(); if (threadIdx.x == 0) out[0] = v + _mata_cuda_cluster_rank(); }\n')
    hip_file=temp/'amd.cpp'
    hip_file.write_text('#include <hip/hip_runtime.h>\n'+generate('hip')+'\n__global__ void smoke(unsigned* out) { if (threadIdx.x==0) out[0] = _mata_hip_read_first_lane(1); }\n')
    checks=[('nvcc',['nvcc','-std=c++17','-arch=sm_90','-c',str(cuda_file),'-o',str(temp/'cuda.o')]),
        ('hipcc',['hipcc','--offload-arch=gfx950','-c',str(hip_file),'-o',str(temp/'amd.o')])]
    for compiler,argv in checks:
        if not shutil.which(compiler):
            print(f'SKIP: {compiler} not installed')
            continue
        print(f'RUN: {" ".join(argv)}')
        subprocess.run(argv,check=True)
