# mata-gpu-intrinsics

A **standalone, dependency-free Rust crate** containing **322 registered GPU shader operations**, 322 documented named Rust wrappers, typed expression emission, and portable CPU reference helpers. It does **not** need an Intel, AMD, NVIDIA, ARM or Apple GPU installed on the **builder's** machine. Building the crate does not require CUDA, Vulkan, DirectX, Metal, a graphics driver or an active display.

**Important distinction:** Shader intrinsics execute on the **target GPU**, after an application compiles and dispatches a WGSL/GLSL/HLSL/OpenCL C shader at runtime. Stable host Rust cannot execute hardware GPU instructions directly. This crate catalogs them and constructs shader expressions in ordinary Rust. It does **not** contain executable machine-specific GPU binaries, an offline shader compiler, or a graphics driver.

## Features

- **322 operation IDs** organized into 15 groups: arithmetic, transcendental math, geometric/vector functions, matrices, bitwise operations, packed formats, derivatives, textures, atomics, barriers, subgroups/waves, vendor/compiler-specific instructions, and more.
- **322 generated Rust functions**, e.g. `ops::sin`, `ops::dot`, `ops::texture_sample`, `ops::atomic_add`, `ops::subgroup_add`.
- **Six shader syntax targets**: WGSL, GLSL, HLSL, Intel OpenCL C, NVIDIA CUDA C++ and AMD HIP C++. Mappings are conservative; an operation without safe syntax returns an explicit `Unsupported` error.
- **Six backend function-pointer tables:** one entry per intrinsic, with stage/capability filtering, named lookups, typed calls and safe bound handles.
- **Type, arity, stage, and declared capability checks** when emitting expressions. The selected shader compiler performs final overload and runtime device validation.
- **Optional CPU reference implementations** for common vector math, packing, matrix operations and bit manipulations.
- No runtime GPU probing, driver loading, `build.rs`, SDK dependency, system linker requirement, procedural macros or external crates.
- `#![forbid(unsafe_code)]`. `no_std` supported with `--no-default-features` and `alloc`.

## Add as a local dependency

```toml
[dependencies]
mata-gpu-intrinsics = { path = "../mata-gpu-intrinsics" }
```

## Usage

### A named intrinsic wrapper

```rust
use mata_gpu_intrinsics::{Backend, CapabilitySet, Emitter, Expr, Stage, Type, ops};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let emitter = Emitter::new(Backend::Hlsl, Stage::Fragment, CapabilitySet::CORE);
    let direction = Expr::variable("direction", Type::VEC3)?;
    let n = ops::normalize(&emitter, &[direction])?;
    let light = Expr::variable("light", Type::VEC3)?;
    let diffuse = ops::dot(&emitter, &[n, light])?;
    assert_eq!(diffuse.code(), "dot(normalize(direction), light)");
    Ok(())
}
```

### Textures and extensions

```rust
use mata_gpu_intrinsics::{Backend, Capability, CapabilitySet, Emitter, Expr, Stage, Type, ops};
let cap = CapabilitySet::CORE.with(Capability::Textures);
let e = Emitter::new(Backend::Wgsl, Stage::Fragment, cap);
let tex = Expr::variable("my_texture", Type::Texture2D).unwrap();
let samp = Expr::variable("my_sampler", Type::Sampler).unwrap();
let uv = Expr::variable("uv", Type::VEC2).unwrap();
let expr = ops::texture_sample(&e, &[tex, samp, uv], Type::VEC4).unwrap();
assert_eq!(expr.code(), "textureSample(my_texture, my_sampler, uv)");
```

Resource declarations, bindings and shader entry points must be supplied by the application. A `CapabilitySet` enables the **catalog** to emit a feature; it does not mean the actual GPU supports it.

### Generate complete helper functions

```bash
cargo run --example generate_shader
cargo run --example intel_opencl
cargo run --example nvidia_cuda
cargo run --example function_table
```

The example emits native WGSL, GLSL and HLSL function definitions. GPU hardware isn't required.

## Backend function-pointer table (new in v0.5.0)

Choose the backend once. The crate constructs a **CPU-side function-pointer table**
with a slot for each catalogued intrinsic. Unsupported operations are represented
by `None`; supported entries are bound to your selected backend, stage and
requested feature mask.

```rust
use mata_gpu_intrinsics::{
    Backend, CapabilitySet, Expr, FunctionTable, Intrinsic, Stage, Type,
};

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let table = FunctionTable::nvidia(CapabilitySet::CORE);
let x = Expr::variable("x", Type::F32)?;

// Indexed by enum, with prefiltered function pointers:
let sin = table.get(Intrinsic::Sin).expect("supported");
assert_eq!(sin.call(&[x.clone()])?.code(), "sinf(x)");

// Or look up by stable catalog name:
assert_eq!(table.call_named("sin", &[x])?.code(), "sinf(x)");

// Swap implementation without changing your application call sites:
let intel = FunctionTable::intel(CapabilitySet::CORE);
assert_eq!(intel.call(Intrinsic::Sin, &[Expr::float(0.5)?])?.code(), "sin(0.5)");

// GLSL/WGSL/HLSL take an explicit shader stage:
let glsl = FunctionTable::new(Backend::Glsl, Stage::Fragment, CapabilitySet::CORE);
assert!(glsl.contains(Intrinsic::Sin));
# Ok(())
# }
```

For opaque return types, use `table.call_typed(...)` or
`table.get(op).unwrap().call_typed(...)`. You can enumerate available operations
with `table.entries()` or retrieve an actual function pointer with
`table.pointer(op)` (call it with `table.emitter()`). Every backend has a
compile-time pointer array of 322 operations, filtered at table creation time.

These are **Rust function pointers to GPU shader-source emitters**, not CUDA,
HIP, OpenCL, Vulkan, or graphics-driver function pointers; they do not launch
kernels. No GPU is needed to construct them.

See [Dispatch table guide](docs/DISPATCH.md) for all public functions and
performance/portability details.

Run the no-GPU example with `cargo run --example function_table`.

## Intel GPU OpenCL C backend

Use `Backend::IntelOpenCl` to generate Intel GPU-targeted OpenCL C compute expressions and `intel::KernelBuilder` to assemble complete buffer-based kernels. A physical Intel GPU is **not required for `cargo build`, `cargo test`, or source generation**. The target Intel driver compiles the kernel later at runtime.

```bash
cargo run --example intel_opencl
```

Supported paths include core math/vector functions, workgroup barriers, subgroup operations, and Intel-specific shuffle and 32-bit block-memory intrinsics guarded by `Capability::IntelSubgroups`. The capability masks are **not device-detection results**. Unsupported operations return `Error::Unsupported`. See [Intel backend guide](docs/INTEL_OPENCL.md) for restrictions, extension names, and an offline Clang syntax-check example.

## NVIDIA GPU CUDA C++ backend

Use `Backend::NvidiaCuda` to generate NVIDIA CUDA C++ **device-side source code** on any machine, without installing the CUDA toolkit or driver to build the Rust crate. `nvidia::KernelBuilder` creates typed compute kernels and `nvidia::Buffer::atomic_address_at` supports CUDA 32-bit integer atomics. NVIDIA-specific warp operations require explicit thread masks and declaration of `Capability::NvidiaWarp`. The `Capability::NvidiaSm80` flag unlocks newer warp reductions; it does not detect the GPU's compute capability. The external application must compile source with NVRTC/nvcc and allocate/launch with the CUDA runtime/driver.

```bash
cargo run --example nvidia_cuda
cargo run --example function_table
```

The CUDA emitter deliberately rejects generic CUDA vector math and shader-style texture operations that do not translate correctly. See [NVIDIA backend guide](docs/NVIDIA_CUDA.md) for the supported operations, threading constraints, sample kernels, and limitations.

## AMD GPU ROCm HIP backend

`Backend::AmdHip` generates HIP C++ compute source **on any build machine**. The
`amd::KernelBuilder` produces complete source-only kernels with typed global
buffers, 1D indexing, scalar math, integer atomics and HIP wavefront functions.
AMDGCN low-level intrinsics are guarded by `Capability::AmdGcn` (which also
requires `Capability::AmdWave`). The emitter preserves full **64-bit wavefront
ballot masks**, unlike a CUDA-oriented 32-bit `u32` mask.

```bash
cargo run --example amd_hip
```

See [AMD HIP backend guide](docs/AMD_HIP.md) for all 35 new operations,
wave32/wave64 behavior, type constraints, compiler requirements and caveats.
Generating HIP source requires no GPU, but compiling and executing that source
later requires the appropriate ROCm/HIP toolchain and supported device.

## Documentation

- **[AMD HIP C++ backend](docs/AMD_HIP.md)**: AMD GPU wavefront, AMDGCN lane/DS permutations, memory ordering, 64-bit ballots and HIP kernel construction.
- **[NVIDIA CUDA C++ backend](docs/NVIDIA_CUDA.md)**: native compute kernels, warp operations, 32-bit atomics, and SM80 gating.
- **[Intel OpenCL C backend](docs/INTEL_OPENCL.md)**: full kernel source, vendor extension requirements, supported and intentionally unsupported operations.
- **[Backend dispatch table](docs/DISPATCH.md)**: function-pointer lookup, bound handles, capability filtering and error behavior.
- **[Every GPU intrinsic](docs/INTRINSICS.md)**: descriptions, arity, shader stage, requested capabilities, WGSL/GLSL/HLSL/OpenCL/CUDA/HIP spellings, and unsupported mappings for all 322 operation IDs.
- **[Every public Rust function](docs/API.md)**: exact source signatures and function-level descriptions, including the 322 named wrappers and dispatch API.
- `cargo doc --all-features --no-deps`: generated Rustdoc, including source comments on exported functions.

## Portability and constraints

| Stage | Needs a physical GPU? |
|---|---|
| `cargo check`, `cargo build`, `cargo test`, `cargo doc` | **No** |
| Running the shader-source generator example | **No** |
| Validating generated shader source with an external offline compiler | Not necessarily; compiler is **not bundled** |
| Creating a GPU pipeline and executing emitted shader | **Yes**, at runtime |

Catalogued shader intrinsics are **not universally portable**. For example, wave/subgroup functions may require Shader Model 6 or GLSL extensions; certain WGSL texture and derivative intrinsics require fragment stage or uniform control flow. The emitter rejects unsupported dialect spellings and missing declared capability flags. It does not emulate unavailable hardware opcodes or guarantee acceptance by any given compiler.

Do **not** feed attacker-controlled shader snippets through `Expr::from_source_unchecked`.

## Test commands

```bash
cargo test --all-features
cargo check --no-default-features
cargo doc --all-features --no-deps
cargo run --example generate_shader
cargo run --example intel_opencl
cargo run --example nvidia_cuda
cargo run --example function_table
```

## Regenerate catalogs and documentation

All generators use only Python 3's standard library. They are **development tools**, not build dependencies:

```bash
python3 tools/generate_catalog.py
python3 tools/generate_wrappers.py
python3 tools/generate_dispatch.py
python3 tools/generate_docs.py
```

## Possible integration with Mata

Your `mata-3d` renderer can use `Emitter` to generate shader helper functions for vector transforms, illumination, texture sampling, or compute kernels; integrate returned expressions into existing D3D11 shader source before passing the completed shader to the operating system's shader compiler. The actual runtime program still owns mesh buffers, shader entry points, bindings, and GPU synchronization.

This crate is **independent** of your existing `mata-3d` and `mata-win` sources and can be compiled on a headless CI runner.

## Typed compute program IR (Phase 1)

The additive `ir` module provides typed programs, kernels, records, tuples,
scoped statements, portable work-item coordinates, and backend-specific layout
calculators. Use `Program::emit(Backend)` to produce native compute source,
or `Program::emit_kernel(name, Backend)` to isolate one shader entrypoint.
See [`docs/PHASE_1_COMPLETION.md`](docs/PHASE_1_COMPLETION.md) and
[`examples/phase1_kernel.rs`](examples/phase1_kernel.rs) for coverage,
layout behavior, capabilities, and validation commands.

## Phase 2 target-aware verification (opt-in)

The typed `ir::Program` supports offline, user-supplied device profiles using
`TargetProfile::offline(backend)`, `DeviceLimits`, and `Feature`. Use
`program.validate_for(&profile)` to check types, feature requirements,
workgroup sizes, shader resource layouts, memory permissions and barrier
convergence; `program.emit_for(&profile)` checks before emitting. These calls
**do not query GPU hardware**. See [`docs/PHASE_2.md`](docs/PHASE_2.md) and
[`tests/phase2_validation.rs`](tests/phase2_validation.rs) for coverage and
limitations. The original `Program::emit` remains backward-compatible.

## Phase 3 portable compute kernels

The typed IR now supports common F32/F64 compute math (`ir::compute`), static
workgroup arrays (`ir::shared`, `Statement::Shared`), and 32-bit native atomic
addition (`ir::atomic_add`, `Statement::AtomicAdd`). CUDA, HIP, and OpenCL C
source is emitted from the same validated IR. See [`docs/PHASE_3.md`](docs/PHASE_3.md)
for the exact supported subset, safety/feature requirements, and compilation
smoke-test procedure. The original intrinsic catalog and crate dependencies
remain unchanged.

## Phase 7 — Synchronization and workgroup collectives

The additive typed IR supports explicit atomic-order/scope nodes, memory fences, and feature-gated OpenCL asynchronous workgroup copies. Portable workgroup reductions, scans, and fixed-tile partitions are implemented by the `collectives` module using shared memory and barriers, with no assumed warp size. See the workspace root `PHASE_7.md` for backend limitations and test commands.
