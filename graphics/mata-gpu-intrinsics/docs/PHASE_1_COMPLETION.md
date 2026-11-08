# Phase 1 — Typed semantic IR and six-language source emission

This archive is the additive Phase 1 implementation, built from the expanded
Phase 1 ZIP. No existing `Intrinsic` variants, catalog slot numbering,
`Emitter`, `Expr`, `ops`, or `FunctionTable` APIs have been removed or renamed.
There are **no new Cargo dependencies** and no required GPU at build time.

## Implemented changes

- Preserved earlier scalar numeric types, extended 2/3/4/8/16-lane vectors,
  void, arrays, typed pointers with global/workgroup/constant/private/generic
  address spaces and read/write modes, plus named records.
- Added typed tuple-valued functions with constructed tuple results and
  indexed tuple projections, record and array constructors, vector constructors,
  cast expressions, 64-bit unsigned literals, FP64 literals, and indexing.
- Added indexed writes, blocks, local declarations/assignment, conditionals,
  loops, returns, typed calls, and synchronization statements.
- Normalized **portable work-item builtins to U32** for shader compatibility;
  native OpenCL `size_t` IDs are explicitly narrowed to U32. Callers that need
  work ranges exceeding U32 must split work or add separate target-specific
  64-bit indexing (a Phase 2+ capability).
- Added whole-program function parameter and return-type checks, record-member
  validation, forward record dependency ordering, unknown name detection,
  lexical scope checks, immutable argument enforcement, full return-path
  checks for nonvoid helpers, type-valid stores/loads, and cyclic call rejection.
- Added checked layout policies: `Storage430`, `Uniform140`, `WgslUniform`,
  `HlslConstant`, `HlslStructured`, `OpenClNatural`, `CudaNative`, including
  `Program::layout_with`, `record_layout_with`, `layout_for_backend`, and
  `kernel_uniform_layout`. These policies are **distinct**; no universal
  record-layout equivalence is assumed.
- Added stable deterministic tuple types, helper prototypes and HLSL tuple
  and record constructors.
- Completed all six target emit *entrypoint shapes*: WGSL bindings and builtins,
  GLSL 450 compute and storage resources, HLSL compute and structured buffers,
  OpenCL C `__kernel` declaration, CUDA C++ and HIP C++ global kernels.
- Added `Program::emit_kernel(name, backend)` for selecting one entrypoint in
  shader languages, where emitting several kernels as one translation unit
  would cause name or binding collisions.
- Added optional native syntax-compilation harness and 19 focused Rust
  integration tests (`tests/phase1_complete.rs`) without GPU dependencies.

## Boundaries and guaranteed failure behavior

- This is **source generation, not a runtime**. Device discovery, allocation,
  queues, driver/compiler loading, and launch are Phase 4 or later.
- GPU hardware type availability is not assumed. A type such as `F64` or a
  vendor-specific wide vector can fail with `Unsupported` on languages where
  it cannot be represented. This is intentional rather than silent narrowing.
- `no_std + alloc` remains supported. The `cpu` module continues to require
  the optional default `std` feature.
- OpenCL C extension pragmas and WGSL `enable f16` are emitted for typed
  half-precision uses. Actual extension availability belongs to runtime
  capability negotiation in Phase 2.
- GLSL shader emission uses a single `main`; HLSL uses named entrypoints and
  optional synthesized dispatch-uniform data on `b1` if global workgroup counts
  are referenced. Callers must bind dispatch metadata correctly.
- Resource buffer layouts must be chosen **per backend and address space**;
  passing bytes packed for std430 directly to an HLSL cbuffer is not valid.
- WGSL uniform arrays and nested records may need host-shareable wrapper types
  and explicit strides; full cross-backend uniform ABI interchange is **not
  guaranteed** by a byte-size calculator. Use flat host-shareable scalar/vector
  parameters for the Phase 1 portable subset and validate advanced layouts
  with the actual shader compiler before dispatch.
- No vendor compiler or GPU runtime is linked by the crate. Optional compile
  tests are separate and report missing toolchains as SKIPPED.
- Phase 2 is still responsible for device-specific capability predicates,
  memory ordering, uniformity and divergence proofs, and safety validation
  beyond the structural/type validation implemented here.

## Local validation

```sh
cargo check --all-targets --all-features
cargo check --all-targets --no-default-features
cargo test --all-targets --all-features
cargo test --all-targets --no-default-features
cargo test --test phase1_complete
cargo run --example phase1_kernel -- cl
python3 tools/phase1_compiler_smoke.py
```

The last command compiles actual generated source when `clang`,
`glslangValidator`, `dxc`, `nvcc`, or `hipcc` are installed; absence of an
optional compiler is not treated as a passing check. A WGSL validator should
be run separately; its CLI and API vary by implementation.

## Validation evidence in the generation environment

- Existing static baseline tests passed (`tools/validate_catalog.py`,
  `validate_dispatch.py`, `validate_nvidia.py`, `validate_amd.py`, and
  `phase0_audit.py --check`).
- A hand-written representative OpenCL C kernel using the generated kernel
  shapes passed Clang's `-x cl -cl-std=CL3.0 -fsyntax-only` check.
- **Rust compilation and the new 19 test cases were not run**, because
  `cargo`/`rustc` are not available in the generation environment.
- No CUDA, HIP, WGSL, GLSL, or HLSL target compiler validation was run here.

Do not treat the new unexecuted integration tests as a successful CI run; the
first acceptance gate on a Rust-equipped machine is `cargo test --all-targets`.
