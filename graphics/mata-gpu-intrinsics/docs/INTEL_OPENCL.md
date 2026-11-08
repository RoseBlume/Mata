# Intel GPU backend — OpenCL C

The `Backend::IntelOpenCl` dialect generates **OpenCL C compute-kernel source** for Intel GPU drivers. The Rust crate stays *completely hardware-independent*: building/testing, emitting expressions, and generating `.cl` source never accesses a GPU and never links against an OpenCL library. This backend is **not Intel GPU machine code, a SPIR-V compiler, a D3D11 renderer, or a runtime executor**.

An application can use Intel's OpenCL implementation at *runtime* to compile the generated source, allocate buffers, and enqueue kernels. The caller must check the actual device's OpenCL C version and extension strings before enabling vendor-specific operations. `CapabilitySet` is caller-supplied metadata, **not** automatic feature detection.

## Source generation example

Run the bundled example on any machine with Rust:

```sh
cargo run --example intel_opencl
```

It generates code equivalent to:

```c
// Intel OpenCL C device kernel; compile on the target runtime GPU.

__kernel void scale_and_bias(const __global float* input, __global float* output, uint count, float scale) {
    uint i = (uint)get_global_id(0);
    if (i >= count) return;
    output[i] = ((input[i] * scale) + 0.25);
}
```

The `count` bounds check is important if the work-group grid is rounded up. `KernelBuilder::index_1d` casts the OpenCL `size_t` work-item ID to 32 bits, so a kernel using that helper must not exceed `u32::MAX` global indices. For wider workloads, construct a `size_t` index manually in the embedding application.

### Emitting Intel shuffle operations

```rust
use mata_gpu_intrinsics::{intel::KernelBuilder,ops,Capability,CapabilitySet,Expr,Type};

let features = CapabilitySet::CORE.with(Capability::IntelSubgroups);
let mut k = KernelBuilder::new("my_kernel", features)?;
let e = k.emitter();
let value = Expr::variable("value", Type::U32)?;
let lane = Expr::uint(1);
let result = ops::subgroup_shuffle(&e, &[value, lane], Type::U32)?;
assert_eq!(result.code(), "intel_sub_group_shuffle(value, 1u)");
```

The generated kernel contains `#pragma OPENCL EXTENSION cl_intel_subgroups : enable` when the capability is enabled. On a device without that extension, the GPU compilation must fail; source-generation success is **not** a promise of runtime execution.

## Supported operations and restrictions

The catalog defines **271 operations across 4 shader-language targets**, of which **102** currently have an Intel OpenCL C mapping. Consult [INTRINSICS.md](INTRINSICS.md) for the authoritative per-intrinsic compatibility table, [API.md](API.md) for the complete Rust API, and the inline `rustdoc` descriptions for the kernel/buffer helper methods.

- Common compute math: trigonometry, `exp`, `log`, `sqrt`, `rsqrt`, `fma`, `min`, `max`, `clamp`, interpolation, etc.
- Vector geometry: `length`, `normalize`, `dot`, `cross`, `reflect`, `refract`, etc.
- Arithmetic/bitwise: infix math operators, bit operations, bit rotations, `clz`, `popcount`, casts.
- Synchronization: workgroup barrier becomes `barrier(CLK_LOCAL_MEM_FENCE)`; storage barrier becomes `barrier(CLK_GLOBAL_MEM_FENCE)`. Both have workgroup synchronization semantics; do not use these for inter-workgroup synchronization.
- Intel extension shuffles: `intel_sub_group_shuffle`, `_xor`, `_up`, `_down` through `Capability::IntelSubgroups`.
- Intel extension 32-bit subgroup block memory: `intel_sub_group_block_read`, `read2`, `read4`, and `block_write`, `write2`, `write4`, for `__global uint*` buffers. **All lanes must use the same pointer**; do not make divergent calls.
- Unavailable: native OpenCL matrix types, fragment-stage derivatives, unmodelled texture sampler signatures, and OpenCL atomics that require pointer/address-space lowering. The catalog returns `Unsupported` rather than emit a questionable shader expression.
- Extended `uint8` and `ushortN` Intel block I/O are **listed for reference but not emitted yet**, since the original expression type model only handles 2–4 vector lanes and 32-bit scalar types.
- OpenCL vector comparisons have integer mask semantics; operations that would falsely return a `boolN` vector are rejected.
- Only the **compute** stage is supported by this backend. Other targets' vertex/fragment emission remains unchanged.

## Validation

```sh
cargo test
cargo check --no-default-features
cargo doc --no-deps
python3 tools/validate_catalog.py
```

Optional offline OpenCL syntax validation with an external compiler (no GPU needed):

```sh
cargo run --example intel_opencl > /tmp/mata-intel.cl
clang -x cl -cl-std=CL2.0 -fsyntax-only /tmp/mata-intel.cl
```

This checks generic OpenCL C syntax. It **does not** verify Intel-only extension support; that requires a device-capable compiler/runtime. No benchmark or hardware execution is claimed.

## References

- [Khronos `cl_intel_subgroups` extension specification](https://registry.khronos.org/OpenCL/extensions/intel/cl_intel_subgroups.html)
- [Khronos `cl_intel_subgroups_short` extension specification](https://registry.khronos.org/OpenCL/extensions/intel/cl_intel_subgroups_short.html)
- [Intel OpenCL graphics extension overview](https://www.intel.com/content/www/us/en/developer/articles/technical/opencl-intel-graphics-extensions.html)
