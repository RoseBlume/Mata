"""Generate human-reviewable docs from the same operation data and Rust source."""
from pathlib import Path
from collections import defaultdict
import runpy,re
root=Path(__file__).resolve().parent.parent
ops=runpy.run_path(str(root/'tools/generate_catalog.py'))['ops']
# NVIDIA mappings are intentionally a small, type-checked subset implemented
# directly in nvidia.rs (rather than pretending GLSL/OpenCL calls match CUDA).
backend_impl=(root/'src/nvidia.rs').read_text().split('fn bad(',1)[0]
cuda_names={}
for match in re.finditer(r'([A-Za-z][A-Za-z0-9_|]*)\s*=>\s*"([^"]+)"',backend_impl):
 for token in match.group(1).split('|'):
  cuda_names[token]=match.group(2)
amd_src=(root/'src/amd.rs').read_text().split('fn bad(',1)[0]
amd_names={}
for match in re.finditer(r'([A-Za-z][A-Za-z0-9_|]*)\s*=>\s*"([^"\n]+)"',amd_src):
 for token in match.group(1).split('|'):
  amd_names[token]=match.group(2)
# AMD inherits only CUDA-compatible scalar math/operator mappings.
for name,symbol in cuda_names.items():
 if not name.startswith('Nvidia') and name not in amd_names:
  amd_names[name]=symbol
by_family=defaultdict(list)
for op in ops:by_family[op[1]].append(op)

descriptions={
'Abs':'Absolute value','Sign':'Sign of a number','Sin':'Sine','Cos':'Cosine','Tan':'Tangent',
'Asin':'Inverse sine','Acos':'Inverse cosine','Atan':'Inverse tangent','Atan2':'Two-argument inverse tangent',
'Sinh':'Hyperbolic sine','Cosh':'Hyperbolic cosine','Tanh':'Hyperbolic tangent',
'Asinh':'Inverse hyperbolic sine','Acosh':'Inverse hyperbolic cosine','Atanh':'Inverse hyperbolic tangent',
'Exp':'Natural exponential','Exp2':'Base-two exponential','Log':'Natural logarithm','Log2':'Base-two logarithm',
'Sqrt':'Square root','InverseSqrt':'Reciprocal square root',
'Floor':'Round toward negative infinity','Ceil':'Round toward positive infinity',
'Trunc':'Truncate toward zero','Round':'Round to nearest integer','Fract':'Fractional part',
'Degrees':'Convert radians to degrees','Radians':'Convert degrees to radians',
'Pow':'Raise a value to a power','Min':'Minimum','Max':'Maximum','Clamp':'Clamp within two bounds',
'Fma':'Multiply-add (contraction may depend on target)','Saturate':'Clamp to [0,1] (not valid in every target)',
'Mix':'Interpolate/mix values','Step':'Step at a threshold','Smoothstep':'Smooth Hermite threshold',
'Modf':'Fractional and integral components (multi-value result)','Frexp':'Mantissa and exponent (multi-value result)',
'Ldexp':'Multiply by a power of two','QuantizeToF16':'Quantize numeric precision to 16-bit float',
'Length':'Euclidean vector length','Distance':'Euclidean distance','Dot':'Vector dot product',
'Cross':'3D vector cross product','Normalize':'Unit-length direction',
'Reflect':'Reflection about surface normal','Refract':'Refraction about surface normal',
'FaceForward':'Orient normal against an incident direction',
'Transpose':'Transpose a matrix','Determinant':'Compute a square-matrix determinant','Inverse':'Invert a square matrix',
'Add':'Addition','Sub':'Subtraction','Multiply':'Multiplication','Divide':'Division','Modulo':'Remainder',
'Negate':'Unary negation','Equal':'Equality comparison','NotEqual':'Inequality comparison',
'Less':'Less-than comparison','LessEqual':'Less-than-or-equal comparison',
'Greater':'Greater-than comparison','GreaterEqual':'Greater-than-or-equal comparison',
'LogicalAnd':'Boolean AND','LogicalOr':'Boolean OR','LogicalNot':'Boolean NOT',
'BitAnd':'Bitwise AND','BitOr':'Bitwise OR','BitXor':'Bitwise exclusive OR',
'BitNot':'Bitwise complement','ShiftLeft':'Bit shift left','ShiftRight':'Bit shift right',
'Select':'Choose a value using a Boolean mask',
'All':'Check whether every vector component is true','Any':'Check whether some component is true',
'CountOneBits':'Population count','CountLeadingZeros':'Count high-order zero bits',
'CountTrailingZeros':'Count low-order zero bits','ReverseBits':'Reverse bit order',
'FirstLeadingBit':'First set bit from the high end (target semantics vary)',
'FirstTrailingBit':'First set bit from the low end',
'RotateLeft':'Rotate bits to the left (synthesized expression)',
'RotateRight':'Rotate bits to the right (synthesized expression)',
'Pack2x16Float':'Pack two half-precision floats into u32',
'Unpack2x16Float':'Unpack two half-precision floats',
'Dpdx':'Partial derivative in screen X','Dpdy':'Partial derivative in screen Y',
'Fwidth':'Sum of absolute derivatives',
'TextureSample':'Sample texture at normalized coordinates using a sampler',
'TextureLoad':'Read texture texel using integer coordinates',
'TextureStore':'Write a storage texture texel',
'TextureDimensions':'Read texture dimensions',
'WorkgroupBarrier':'Synchronize compute workgroup memory operations',
'StorageBarrier':'Synchronize storage operations in the workgroup',
'AtomicLoad':'Read an atomic integer','AtomicStore':'Write an atomic integer',
'AtomicCompareExchangeWeak':'Compare and optionally replace atomic value',
'Discard':'Terminate fragment (statement; not callable)',
'SinCos':'Compute sine and cosine via output parameters (catalog-only)',
}
def desc(name,fam):
 if name in descriptions:return descriptions[name]
 if fam=='Texture':return 'Texture '+re.sub('([A-Z])',r' \1',name.removeprefix('Texture')).strip().lower()+' operation (backend overloads vary)'
 if fam=='Subgroup':return 'Subgroup '+name.removeprefix('Subgroup').lower()+' operation (wave/extension support required)'
 if fam=='Packing':return ('Unpack' if name.startswith('Unpack') else 'Pack')+' packed numeric representation'
 if fam=='Atomic':return 'Atomic '+name.removeprefix('Atomic').lower()+' operation'
 if fam=='Barrier':return 'GPU memory synchronization barrier'
 if fam=='Bits':return 'Bit-manipulation operation'
 if fam=='Derivative':return 'Fragment derivative or sample interpolation operation'
 if fam=='Vendor':return 'Vendor/compiler-version-specific operation'
 return fam+' operation'

def code(x):return f'`{x}`' if x else '—'

out=[
'# GPU Intrinsics Reference — all supported operations',
'',
'> **Scope:** This is a Rust **catalog and shader-source expression emitter**, not a list of Rust machine intrinsics or a GPU driver. GPU op availability depends on shader target, compiler version, feature flags, and device at **runtime**. Building this crate never compiles GPU shaders or detects an adapter.',
'',
'## Reading the tables',
'',
'- **Enum** is the `Intrinsic::Variant` used from Rust. **Native name** is the spelling emitted or documented for WGSL / GLSL / HLSL / Intel OpenCL C / NVIDIA CUDA C++ / AMD HIP C++. `—` indicates that the implementation refuses emission for that backend.',
'- Operators and synthetic bit rotates are expanded by `Emitter` even though their catalog native spelling is metadata only.',
'- **Arguments** is a conservative pre-validation range; exact overload and argument types remain the responsibility of the selected shader compiler.',
'- **Stage** `Any` means not restricted by this catalogue. Certain operations still carry target-specific shader-stage restrictions and uniformity requirements.',
'- **Capability** must be enabled with `CapabilitySet::with`; this models a requested capability, **not** a successful runtime device capability query.',
'- Some entries are **catalog-only** (no valid normal function-call emission). Examples include shader statements requiring syntax not representable as an expression or extension-only operations without a safe cross-backend mapping.',
'',
'## Summary',
'',
f'**{len(ops)} total registered operations** across {len(by_family)} families.',
'',
'| Family | Count |', '|---|---:|',
]
for f,items in by_family.items():out.append(f'| {f} | {len(items)} |')
out+=['','## Complete operation catalog','']
for fam,items in by_family.items():
 out.extend([f'### {fam} ({len(items)})','',
 '| Rust enum | Meaning | Args | Capability | Stage | WGSL | GLSL | HLSL | Intel OpenCL C | NVIDIA CUDA C++ | AMD HIP C++ |',
 '|---|---|---:|---|---|---|---|---|---|---|---|'])
 for name,_,sig,lo,hi,cap,stage,wgsl,glsl,hlsl,intel in items:
  arity=str(lo) if lo==hi else f'{lo}–{hi}'
  out.append(f'| `{name}` | {desc(name,fam)} | {arity} | `{cap}` | `{stage}` | {code(wgsl)} | {code(glsl)} | {code(hlsl)} | {code(intel)} | {code(cuda_names.get(name))} | {code(amd_names.get(name))} |')
 out.append('')
out+=['## Compatibility and correctness','',
'1. **Compilation without a GPU:** Rust compiles catalog metadata and string-generation code with zero external dependencies; shader code runs only after your renderer compiles it on the desired device.',
'2. **Optional capabilities:** Derivatives, packing, textures, storage access, atomics, subgroup/wave functions, barriers and vendor features may require GLSL extensions, shader model versions, WebGPU extensions, driver support or specific pipeline stages.',
'3. **No automatic cross-compilation:** WGSL, GLSL and HLSL have differences in sampler/resource types, packing semantics, atomics, wave operations and return structures. An emitted expression is **not** proof a complete shader will validate.',
'4. **Source-only:** The emitter returns expressions and standalone helper-function definitions. It does not compile SPIR-V, DXIL or Metal AIR; it does not construct GPU pipelines or submit commands.',
'5. **Security:** `Expr::from_source_unchecked` accepts caller-provided shader code verbatim. Do not pass untrusted strings.',
'',
'See [Rust API](API.md) and the crate [README](../README.md).','']
(root/'docs/INTRINSICS.md').write_text('\n'.join(out),encoding='utf-8')

# Extract exported fn signatures from the actual Rust source, including associated methods.
# Only exported API is documented here, not internal test helpers and generator functions.
public=[]
for src in ['catalog.rs','types.rs','emit.rs','dispatch.rs','intel.rs','nvidia.rs','amd.rs','ops.rs','cpu.rs']:
 path=root/'src'/src
 text=path.read_text()
 for match in re.finditer(r'\bpub\s+(?:const\s+)?fn\s+[a-zA-Z_][a-zA-Z_0-9]*\s*\(',text):
  begin=match.start();offset=match.end();depth=1
  while offset<len(text) and depth>0:
   if text[offset]=='(':depth+=1
   elif text[offset]==')':depth-=1
   offset+=1
  while offset<len(text) and text[offset]!='{':offset+=1
  signature=text[begin:offset].strip().replace('\n',' ')
  signature=re.sub(r'\s+',' ',signature)
  leading=text[:text.rfind("\n",0,begin)+1].splitlines()
  comments=[]
  for line in reversed(leading):
   if line.strip().startswith('///'):comments.append(line.strip().removeprefix('///').strip())
   elif line.strip().startswith('#['):continue
   elif not line.strip():break
   else:break
  comments.reverse()
  public.append((src,signature,' '.join(comments) or 'Public function; see Rustdoc for behavior.'))
api=[
'# Rust API Reference — every exported function',
'',
'This document is generated from the crate source by `python3 tools/generate_docs.py`. Each entry lists its actual Rust signature and its source-level documentation. For the `Intrinsic` variants, see [INTRINSICS.md](INTRINSICS.md).',
'',
'## Public types',
'',
'| Type | Responsibility |', '|---|---|',
'| `Backend` | Select WGSL, GLSL, HLSL, Intel OpenCL C, NVIDIA CUDA C++, or AMD HIP C++ syntax |',
'| `Stage` | Vertex, fragment, compute validity context |',
'| `Scalar`, `Type` | Shader scalar/vector/matrix/resource type metadata |',
'| `Capability`, `CapabilitySet` | Shader features requested by the caller, not GPU detection |',
'| `Intrinsic`, `IntrinsicInfo`, `Family`, `Signature`, `Arity` | Operation registry and native language spellings |',
'| `Expr` | Typed shader-source expression |',
'| `Emitter` | Validated expression emitter |',
'| `FunctionTable`, `BoundFunction`, `IntrinsicFn`, `TableEntry` | Per-backend, capability-filtered source-emission function-pointer dispatch |',
'| `FunctionBuilder` | Shader helper-function serialization |',
'| `Error` | Invalid identifier/type/literal, stage, capability, arity or backend |',
'| `nvidia::KernelBuilder` | Generate standalone CUDA device-kernel source without linking a CUDA SDK |',
'| `amd::KernelBuilder` | Generate HIP device kernel source without linking ROCm or HIP |',
'| `amd::Buffer` | Typed HIP global buffer parameter and atomic addresses |',
'| `nvidia::Buffer` | Typed CUDA device buffer parameter and atomic pointer expressions |',
'| `cpu::Float4` | Four-lane portable CPU reference vector (`std` feature) |',
'',
'## Public functions and methods (including every named intrinsic wrapper)',
'',
]
for src in ['catalog.rs','types.rs','emit.rs','dispatch.rs','intel.rs','nvidia.rs','amd.rs','ops.rs','cpu.rs']:
 fns=[item for item in public if item[0]==src]
 api+=[f'### `src/{src}` — {len(fns)} exported functions','']
 for _,sig,doc in fns:
  api+=[f'#### `{sig.split("(",1)[0].split()[-1]}`','',f'```rust\n{sig}\n```','',doc,'']
api+=['## Trait method behavior','',
'`cpu::Float4` implements `Add`, `Sub`, `Mul` and `Div`, each performing four component-wise scalar operations. These are Rust operator trait methods, not exported inherent functions. Their internal method names are `add`, `sub`, `mul` and `div`.',
'',
'## Errors and guarantees','',
'- Shader expression emission is CPU-only. No GPU is required for `cargo check`, `cargo test`, `cargo doc`, or building the library.',
'- `Emitter::call` validates argument count and well-understood type families; `Emitter::call_typed` explicitly specifies a return type for opaque operations. Neither replaces shader compiler validation.',
'- `Expr::from_source_unchecked` is deliberately unchecked; only use it for trusted input.',
'- `FunctionBuilder` creates helper functions. Constructing full shader modules, resource bindings and pipeline declarations is the caller’s job.',
'- `CapabilitySet::ALL` enables catalog checking but **does not** guarantee runtime hardware or driver capabilities.',
'',
'## Generate Rustdoc','',
'```bash\ncargo doc --all-features --no-deps --open\n```','']
(root/'docs/API.md').write_text('\n'.join(api),encoding='utf-8')
print('Wrote docs for',len(ops),'operations and',len(public),'public Rust functions/methods')
