# Validation notes

## Static checks performed for this deliverable

- Generated Rust intrinsic wrappers are unique and correspond one-to-one with the 271 `Intrinsic` enum variants.
- The `ALL_INTRINSICS` table and metadata table have matching lengths and ordering.
- Every catalog entry occurs in the generated intrinsic reference documentation.
- Every public Rust function has a source-level `///` doc comment and an API reference entry.
- Runtime capabilities and shader stages are modeled explicitly rather than using build-machine GPU detection.
- No `[dependencies]`, build script, external shader compiler, `unsafe`, or platform-specific link requirement is introduced.

## Not yet tested

There is **no Rust toolchain installed in the authoring environment**, so this delivery has **not** been verified with `cargo check`, `cargo test`, or `cargo doc`. **Clang 17 accepted** the provided generic Intel OpenCL C syntax fixture (`tests/intel_emitted_sample.cl`). Intel subgroup extension kernels were not compiled by an Intel driver, and HLSL/GLSL/WGSL code paths were not compiler-validated. Source and generated-data checks are not a substitute for compilation. Some native shader operations require runtime extensions, specific shaders/versions, or more elaborate argument rules than this crate can currently enforce.

## Commands to run on a Rust builder (no GPU required)

```bash
cargo check --all-features
cargo check --no-default-features
cargo test --all-features
cargo doc --all-features --no-deps
cargo run --example generate_shader
cargo run --example intel_opencl
```

## Optional shader compiler validation

After assembling full shader sources from emitted expressions, validate them with the appropriate **external compiler** (WGSL validator, `glslangValidator`, DXC/FXC) for your target shader model, version, extensions and device capabilities. These tools are not included and are not needed to compile this crate.

## Intel OpenCL offline syntax validation performed

```sh
clang -x cl -cl-std=CL2.0 -fsyntax-only tests/intel_emitted_sample.cl
```

This succeeded with the installed Clang 17 OpenCL frontend; it is not an Intel GPU runtime or performance test.

## NVIDIA CUDA C++ backend

The `Backend::NvidiaCuda` target in version 0.3.0 is a safe, device-independent **source emitter**. To check the generated Rust catalog and docs without a GPU:

```sh
python3 tools/validate_catalog.py
python3 tools/validate_nvidia.py
cargo test --all-features
cargo check --no-default-features
cargo run --example nvidia_cuda
```

The unit tests in `tests/nvidia_cuda.rs` validate emitted strings, buffer argument types, warp intrinsics, capability rejection, and that the legacy WGSL/GLSL/HLSL/Intel outputs are unchanged. **They do not execute CUDA.**

On a separate machine with the CUDA toolkit, take the output from `cargo run --example nvidia_cuda` and compile with a suitable installed nvcc/NVRTC configuration. For example, redirect the example's output to a `.cu` file and run an offline CUDA compile targeting the compute capability of the intended NVIDIA GPU. A valid compile is not a performance benchmark or proof of device support. The application remains responsible for GPU allocations, runtime feature queries, kernel launch, error handling, and output verification.
