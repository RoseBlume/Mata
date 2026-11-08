#!/usr/bin/env python3
"""Compile actual emitted CL3 source if Cargo and Clang are available.

This script is opt-in, not a GPU execution or NVIDIA/HIP validation.
"""
from pathlib import Path
import subprocess,shutil,sys,tempfile
root=Path(__file__).resolve().parents[2]
if not shutil.which('cargo') or not shutil.which('clang'):
    print('SKIP: cargo and clang are both needed for emitted CL3 source validation')
    sys.exit(2)
src=subprocess.run(['cargo','run','-q','-p','mata-gpu-intrinsics','--example','phase7_emitted_source'],cwd=root,capture_output=True,text=True)
if src.returncode:print(src.stderr);sys.exit(src.returncode)
with tempfile.TemporaryDirectory() as tmp:
    file=Path(tmp)/'mata_phase7.cl';file.write_text(src.stdout)
    checked=subprocess.run(['clang','-x','cl','-fsyntax-only','-cl-std=CL3.0',str(file)],capture_output=True,text=True)
    if checked.returncode:print(checked.stderr);sys.exit(checked.returncode)
print('PASS: the actual emitted Phase 7 OpenCL C source compiles with clang')
