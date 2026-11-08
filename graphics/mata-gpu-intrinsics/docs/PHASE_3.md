# Phase 3 — Core compute lowering

> **Implemented in source; Rust and vendor-compiler verification must run on a machine with the respective toolchains.** This crate emits GPU source and does not launch kernels, allocate GPU memory, or assert that any GPU is present.

## Delivery

Phase 3 adds semantic (not untrusted source-text) operations for common GPU computation on **OpenCL C 3.0, NVIDIA CUDA C++, and AMD HIP C++**. All changes are additive to Phase 0–2; the original `Intrinsic` enum still has its 322 entries, and `Cargo.toml` remains dependency-free.

| Capability | New IR/API | OpenCL C | CUDA C++ | HIP C++ |
|---|---|---|---|---|
| Indexing | Existing `Value::Builtin` for global/local/group IDs and size/count | `get_*` | `threadIdx`/`blockIdx` | `threadIdx`/`blockIdx` |
| Typed global buffers | Existing `Parameter`, `DataType::Pointer`, `Value::Load`, `Statement::Store` | `__global` pointers | kernel pointer parameters | kernel pointer parameters |
| Typed floating math | `Value::Compute`, `ComputeOp`, `compute()` | overloadable built-ins | `*f` and double built-ins | `*f` and double built-ins |
| Vector floating math | `ComputeOp` with vectors 2–4 | native vector built-ins | component-wise `make_floatN`/`make_doubleN` | component-wise `make_floatN`/`make_doubleN` |
| Workgroup shared memory | `Statement::Shared` plus `shared()` | `__local` static arrays | `__shared__` arrays | `__shared__` arrays |
| Workgroup synchronization | Existing `Statement::Barrier` and Phase 2 uniformity analysis | `barrier(CLK_LOCAL_MEM_FENCE)` | `__syncthreads()` | `__syncthreads()` |
| Histogram-style atomics | `Statement::AtomicAdd` plus `atomic_add()` | legacy `atomic_add` | native `atomicAdd` | native `atomicAdd` |

### Strict restrictions

* Phase 3 math is **F32 and F64**, scalar or native vec2/3/4. FP64 requires an explicit Phase 2 device-profile feature assertion; the source generator itself cannot inspect real GPU capabilities.
* Supported math: `Abs`, `Sqrt`, `Sin`, `Cos`, `Tan`, `Exp`, `Log`, `Floor`, `Ceil`, `Min`, `Max`, `Clamp`, `Fma`, `Pow`, `Dot`, and `Length`. `Dot`/`Length` require vectors. CUDA/HIP vector types do **not** have arithmetic overloads, so math is emitted component-by-component. This is intentional to avoid invalid native vector expressions.
* CUDA/HIP vector fallback currently requires **named, stable operands** (variables or record fields). Materialize a vector expression in a local variable before passing it to another vector math call; this avoids inadvertently evaluating memory loads or side-effecting device helper calls multiple times.
* `Shared` declares a **statically sized scalar array at the kernel's top level**. Dynamic shared memory, cluster memory, subgroup collectives, and memory-address-space conversions are not implied. Any shared pointer expression must refer to a validated `Statement::Shared` declaration, and duplicate names are rejected.
* `AtomicAdd` supports **signed and unsigned 32-bit integers only**, global/workgroup read-write memory, and discards the old value. It uses native atomic RMW functions with backend-defined legacy ordering; it does **not** expose cross-device visibility or configurable memory order/scope. Those semantics belong to Phase 7.
* The shared-array and basic-atomic IR nodes are currently unsupported by the three shader source targets (WGSL, GLSL, HLSL); the Phase 3 acceptance gate specifically targets OpenCL C, CUDA C++, and HIP C++.
* Invocations outside the logical buffer extent must be guarded. No implicit bounds checks, host memory transfers, device detection, allocation, runtime launch API, or zero-initialization of histogram output buffers are performed.
* Workgroup `Shared` declarations do not currently enforce **device-local memory byte capacity**, since actual device limits are not available to the source-only crate. The caller must verify the required bytes against hardware before launching. This is an explicit limitation until the Phase 4 runtime provides capability probing.
* Math accuracy, NaN behavior, normalization semantics, signed zero, and floating-point contraction are not asserted to be bit-identical across CUDA, HIP, and OpenCL. FMA is explicitly fused where the vendor compiler honors the documented `fma` intrinsic; mathematical equality does not imply identical ULPs or rounding under all compiler flags.

## Real source-generation regression suite

`tests/support/phase3_kernels.rs` constructs four complete programs **through the typed public IR**, not textual shader templates. The same programs are used by `tests/phase3_compute.rs` and by `examples/phase3_kernel_suite.rs`:

1. **SAXPY:** bounded float arrays and explicit fused multiply-add, `out[i] = alpha*x[i] + y[i]`.
2. **Workgroup reduction:** static 64-element float scratch, uniform barriers, masked last block, one output sum per group (additional host reduction required for a single total).
3. **Three-point stencil:** bounds-check both neighbors; each interior output uses `0.25*left + 0.5*center + 0.25*right` (boundary values are intentionally **untouched**).
4. **Integer histogram:** filter by global item count and bin count, then atomically increment the selected bin. **Output bins must be initialized to zero by the caller.**

`tests/phase3_cpu_oracle.rs` includes a separate, deterministic CPU reference corpus for all four algorithms. These tests verify expected outcomes, **not** the generated kernels.

### Validation commands

```bash
cargo check --all-targets --all-features
cargo check --all-targets --no-default-features
cargo test --all-targets
python3 tools/phase0_audit.py --check
python3 tools/phase2_audit.py
python3 tools/phase3_compiler_smoke.py
```

The compiler smoke script **first runs the Rust kernel example and compiles its actual emitted source files**. It uses `clang -x cl -cl-std=CL3.0 -fsyntax-only` for OpenCL and, **when installed**, `nvcc` and `hipcc` for CUDA/HIP. Add `--require-opencl`, `--require-cuda`, or `--require-hip` to make a missing compiler a hard failure rather than a skip. The script does not substitute manually written examples for compiler verification.

To inspect generated output directly:

```bash
cargo run --example phase3_kernel_suite -- target/phase3-generated
```

Tests should be run with real target toolchains and then GPU devices to establish ABI, numerical, race and performance guarantees; this container does not have a Rust toolchain, NVIDIA SDK, AMD SDK, or a usable GPU runtime, so these stages cannot be claimed as passed here. A standalone OpenCL syntax fixture was checked with Clang, but **it was not emitted by the Rust crate**, so it cannot replace the above acceptance tests.

## Phase 3 versus subsequent phases

This phase completes the **source-layer implementation of a portable starter set** for the four required kernel patterns; it is not an implementation of every catalog overload on all GPU targets. Phase 4 provides device selection/launch/buffer runtime, Phase 5 expands math and numeric parity, Phase 6 handles complex resources and memory systems, and Phase 7 supplies full atomic ordering, fences, cooperative groups and async synchronization. The source remains fully source-only, no extra Cargo dependencies, and hardware-independent at build time.
