# Phase 4 — optional GPU host execution companion

Phase 4 adds a sibling Rust crate `mata-gpu-runtime` rather than introducing
native dynamic-loading and unsafe FFI into this `#![forbid(unsafe_code)]`
source-generation crate. The existing 322 operation identifiers, pure emitters,
`no_std + alloc` behavior, and dependency policy are retained.

The new runtime crate provides optional OpenCL, CUDA, HIP discovery, native
JIT compilation, device allocation, synchronous host transfers, asynchronous
kernel launches, and completion events. The public safe API checks context
identity and retains resources across queued work; unsafe platform FFI is
isolated in the runtime package. No GPU or vendor SDK is needed at build time.

Run `cargo test --workspace --all-targets` and enable the live runtime suite
with `MATA_GPU_LIVE_TESTS=1` on a machine that has compatible device drivers and
compilers. This environment does not contain Cargo or any verified working
GPU device; Rust build and execution validation is therefore pending.
