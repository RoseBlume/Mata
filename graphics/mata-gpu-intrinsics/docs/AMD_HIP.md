# AMD GPU backend — HIP C++ and AMDGCN

`mata-gpu-intrinsics` v0.4.0 adds **`Backend::AmdHip`** and the source-only `amd::KernelBuilder`. Rust compilation, documentation and code generation require **no AMD GPU, ROCm SDK or driver**. This crate does **not** execute kernels: the application must compile the HIP C++ code with a compatible HIP-Clang / HIPRTC environment and manage device memory and launches at runtime.

## Supported native capabilities

| Family | Intrinsics | Notes |
|---|---|---|
| Wavefront configuration | `AmdWaveSize`, `AmdLaneId` | Runtime `warpSize`, wave32 or wave64; cannot assume fixed width |
| Wave communication | `AmdShuffle`, `AmdShuffleUp`, `AmdShuffleDown`, `AmdShuffleXor` | HIP shuffles use native `warpSize` |
| Wave voting | `AmdBallot`, `AmdActiveMask`, `AmdAny`, `AmdAll`, `AmdWaveSync` | `AmdBallot` and `AmdActiveMask` return `Type::U64`, retaining wave64 masks |
| AMDGCN lanes | `AmdReadFirstLane`, `AmdReadLane`, `AmdWriteLane` | AMDGCN Clang built-ins; `readlane`/`writelane` lane input must be uniform |
| AMDGCN permutations | `AmdDsBpermute`, `AmdDsPermute` | Input index is a **lane number** and emitter multiplies by four to obtain the required byte address. Forwards/permutation collisions and inactive lanes have target-dependent results |
| Fast float | `AmdFastSin`, `AmdFastCos`, `AmdFastExp`, `AmdFastLog`, `AmdFastLog2`, `AmdFastPow`, `AmdFastRcp` | Device-specific approximate/rounded math; precision and speed may vary |
| Integer | `AmdBytePerm`, `AmdFunnelShiftLeft`, `AmdFunnelShiftRight`, `AmdMul24`, `AmdUMul24`, `AmdSad`, `AmdUSad` | 24-bit operations have narrower numerical assumptions |
| 64-bit bits | `AmdPopcount64`, `AmdReverseBits64` | 64-bit data input, typed 32-/64-bit result |
| Memory ordering | `AmdThreadFenceBlock`, `AmdThreadFenceDevice`, `AmdThreadFenceSystem` | A fence orders memory, it is not a global synchronization barrier |
| Shared HIP API | Scalar mathematics, integer atomics, common operator intrinsics, workgroup barriers | Typed lowering is shared with the CUDA-compatible HIP subset; texture and vector-math overloads not mapped |

## Capability gating

`Capability::AmdWave` enables wave-level operations and `Capability::AmdGcn` enables compiler-specific low-level instructions. `AmdGcn` requires `AmdWave` as a dependency. These flags declare what the caller intends to use; they **do not query the GPU**. Backend is compute-only.

```rust
use mata_gpu_intrinsics::{Backend,Capability,CapabilitySet,Emitter,Expr,Intrinsic,Stage,Type};
let caps=CapabilitySet::CORE.with(Capability::AmdWave).with(Capability::AmdGcn);
let emit=Emitter::new(Backend::AmdHip,Stage::Compute,caps);
let source_lane=Expr::uint(7);
let element=Expr::uint(42);
let copied=emit.call_typed(Intrinsic::AmdDsBpermute,&[element,source_lane],Type::U32)?;
assert!(copied.code().contains("__builtin_amdgcn_ds_bpermute"));
# Ok::<(),mata_gpu_intrinsics::Error>(())
```

Every AMD-specific op is exposed through a named function in `ops` (for example `ops::amd_ds_bpermute`) and the standard `Emitter::call_typed` API. AMD-specific operations are `Signature::Special`, so they require an explicit return type; the AMD emitter validates that type.

## Generating a kernel

```rust
use mata_gpu_intrinsics::{amd::KernelBuilder,CapabilitySet,Expr,Intrinsic,Type};
let mut builder=KernelBuilder::new("scale",CapabilitySet::CORE)?;
let source=builder.read_only_buffer("src",Type::F32)?;
let destination=builder.read_write_buffer("dst",Type::F32)?;
let count=builder.value("count",Type::U32)?;
let i=builder.index_1d("i")?;
builder.return_if_out_of_bounds(&i,&count)?;
let input=source.load(&i)?;
let value=builder.emitter().call(Intrinsic::Multiply,&[input,Expr::float(2.0)?])?;
builder.store(&destination,&i,&value)?;
let hip_source=builder.build()?;
assert!(hip_source.starts_with("// AMD HIP"));
# Ok::<(),mata_gpu_intrinsics::Error>(())
```

Run `cargo run --example amd_hip` to print a wave-mask example, or generate Rustdoc with `cargo doc --all-features --no-deps`.

## Wavefront correctness

- AMD devices may use 32 or 64 lanes per wave depending on architecture and mode. Use emitted `warpSize`, **not** a hard-coded warp of 32.
- Ballot and active masks use unsigned 64-bit values; `Type::U64` and `Expr::uint64()` were added for that reason. Low 32-bit truncation would silently lose wave64 lanes.
- The compiler may not guarantee results for inactive source lanes. GCN lane-read selector arguments must be uniform across executing lanes.
- Do not put wave/workgroup barriers after per-thread early returns in a workgroup without ensuring the remaining threads reach valid synchronization points.
- AMDGCN compiler intrinsics are **not** portable to non-AMD HIP targets. The `AmdGcn` declaration is separate so applications can choose a fallback.

## Validation and limitations

- `cargo test`, `cargo test --no-default-features`, `cargo doc --no-deps` are Rust-side correctness checks and require **no GPU**.
- `hipcc -c generated_kernel.cpp` requires the HIP SDK/compiler but does not require the build machine to have a matching physical GPU if the compiler and target architecture are configured appropriately.
- Real device validation on selected `gfx*` targets and HIP versions is required before production use; the builder cannot discover target ISA capabilities or validate compiler output.
- There are **no** HIP memory management, kernel dispatch, Vulkan graphics shaders, native GCN assembly, or host-side ROCm bindings in this crate.

## Documentation references

- [AMD HIP C++ language extensions](https://rocm.docs.amd.com/projects/HIP/en/latest/how-to/hip_cpp_language_extensions.html)
- [AMD HIP math API](https://rocm.docs.amd.com/projects/HIP/en/latest/reference/math_api.html)
- [AMD wavefront shuffle and lane access built-ins](https://rocm-handbook.amd.com/projects/amd-rocm-optimization-guide/en/develop/compiler-builtins/cross-arch/wavefront-ref/shuffle-builtins.html)
- [AMD DS permutation built-ins](https://rocm-handbook.amd.com/projects/amd-rocm-optimization-guide/en/develop/compiler-builtins/cross-arch/wavefront-ref/dpp-builtins.html)
