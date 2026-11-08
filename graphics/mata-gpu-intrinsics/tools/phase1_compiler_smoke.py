#!/usr/bin/env python3
"""Optional actual compiler syntax checks for generated Phase 1 kernels.

Exits nonzero on an actual compiler failure. Absent tools are reported SKIPPED,
not PASS; the Rust/Cargo prerequisite is mandatory to run this generator.
"""
import pathlib, shutil, subprocess, sys, tempfile
root=pathlib.Path(__file__).resolve().parent.parent
if not shutil.which('cargo'):
    sys.exit('UNAVAILABLE: cargo must be installed before compiler smoke tests')
checks=[
 ('cl','clang',['clang','-x','cl','-cl-std=CL3.0','-fsyntax-only','{file}'],'.cl'),
 ('glsl','glslangValidator',['glslangValidator','-S','comp','{file}'],'.comp'),
 ('hlsl','dxc',['dxc','-T','cs_6_0','-E','compute','{file}','-Fo','{out}'],'.hlsl'),
 ('cuda','nvcc',['nvcc','-std=c++14','-x','cu','-c','{file}','-o','{out}'],'.cu'),
 ('hip','hipcc',['hipcc','-std=c++14','-c','{file}','-o','{out}'],'.hip.cpp'),
]
passed=0
skipped=0
with tempfile.TemporaryDirectory() as work:
    for target,tool,template,suffix in checks:
        if not shutil.which(tool):
            print(f'SKIP {target}: {tool} unavailable'); skipped+=1;continue
        generated=subprocess.run(['cargo','run','--quiet','--example','phase1_kernel','--',target],cwd=root,text=True,capture_output=True)
        if generated.returncode:
            print(f'FAIL generate {target}: {generated.stderr}');sys.exit(1)
        source=pathlib.Path(work)/('compute'+suffix);source.write_text(generated.stdout)
        output=pathlib.Path(work)/('compute'+target+'.bin')
        command=[s.replace('{file}',str(source)).replace('{out}',str(output)) for s in template]
        check=subprocess.run(command,cwd=root,text=True,capture_output=True)
        if check.returncode:
            print(f'FAIL {target}: {check.stdout}\n{check.stderr}');sys.exit(1)
        print(f'PASS {target}: {tool} syntax/compile');passed+=1
print(f'Compiler smoke: {passed} PASS, {skipped} SKIPPED (WGSL requires a separate validator)')
