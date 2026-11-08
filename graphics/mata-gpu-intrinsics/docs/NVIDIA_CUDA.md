# NVIDIA CUDA C++ backend

`Backend::NvidiaCuda` generates **CUDA device code as text**. Compiling the **Rust crate** needs **no NVIDIA card, CUDA SDK, driver, external Rust crate, or build script**. Executing emitted kernels does require a CUDA-capable runtime GPU, a compatible NVIDIA driver and a CUDA compiler such as NVRTC or nvcc. This library **does not provide a CUDA host runtime or compiler**.

## Workflow

```rust
use mata_gpu_intrinsics::{
    nvidia::KernelBuilder, CapabilitySet, Expr, Intrinsic, Type,
};
fn create_source() -> Result<String, mata_gpu_intrinsics::Error> {
    let mut k = KernelBuilder::new("scale", CapabilitySet::CORE)?;
    let input = k.read_only_buffer("input", Type::F32)?;
    let output = k.read_write_buffer("output", Type::F32)?;
    let count = k.value("count", Type::U32)?;
    let idx = k.index_1d("i")?;
    k.return_if_out_of_bounds(&idx, &count)?;
    let x = input.load(&idx)?;
    let y = k.emitter().call(Intrinsic::Multiply, &[x, Expr::float(2.0)?])?;
    k.store(&output, &idx, &y)?;
    k.build()
}
```

Sample output (shortened):

```cuda
extern "C" __global__ void scale(const float* __restrict__ input,
                                     float* __restrict__ output,
                                     unsigned int count) {
    unsigned int i = blockIdx.x * blockDim.x + threadIdx.x;
    if (i >= count) return;
    output[i] = (input[i] * 2.0);
}
```

The caller must provide matching device buffers, decide launch shape, compile the kernel and check device-side errors.

## Supported operations

- Scalar FP32 math, selected scalar integer/Boolean expressions, bit rotations, popcount/reverse, and selected scalar reductions (for example min/max).
- CUDA 32-bit integer atomics: `atomicAdd`, `atomicMin`, `atomicMax`, `atomicAnd`, `atomicOr`, `atomicXor`, `atomicExch`, and `atomicCAS`. Generate pointer operands using `nvidia::Buffer::atomic_address_at`.
- CTA-wide barrier: `WorkgroupBarrier` emits `__syncthreads()`. Do **not** insert it after a divergent early return: all participating CTA threads must reach it.
- Warp-vote/mask/shuffle operations: `NvidiaActiveMask`, `NvidiaBallotSync`, `NvidiaAnySync`, `NvidiaAllSync`, `NvidiaShuffleSync`, `NvidiaShuffleUpSync`, `NvidiaShuffleDownSync`, `NvidiaShuffleXorSync`, `NvidiaSyncWarp`, and `NvidiaMatchAnySync`.
- SM 8.0+ warp reductions: `NvidiaReduceAddSync`, `NvidiaReduceMinSync`, `NvidiaReduceMaxSync`, `NvidiaReduceAndSync`, `NvidiaReduceOrSync`, `NvidiaReduceXorSync` (conservatively restricted to `u32`).

## Warp participation and compute capabilities

For warp operations, opt in with `CapabilitySet::CORE.with(Capability::NvidiaWarp)`. For SM 8.0+ warp reductions, additionally enable `.with(Capability::NvidiaSm80)`. **These flags only affect source generation.** They do not probe or verify the device. Check the target compute capability before compilation/launch. Warp `*_sync` intrinsics take an explicit 32-bit participation mask as their **first argument**, then a value and lane/delta as appropriate. The mask must match threads executing the same intrinsic, and referenced source lanes must be active. Never assume all 32 lanes participate after branching. Some warp functions require specific GPU generations; match-any and SM80 reductions have stricter availability than basic shuffles.

```rust
use mata_gpu_intrinsics::{Backend, Capability, CapabilitySet, Emitter, Expr, Intrinsic, Stage, Type};
let flags = CapabilitySet::CORE.with(Capability::NvidiaWarp);
let emitter = Emitter::new(Backend::NvidiaCuda, Stage::Compute, flags);
let mask = Expr::uint(u32::MAX); // only valid at runtime if the entire warp participates!
let data = Expr::variable("val", Type::U32).unwrap();
let lane = Expr::uint(0);
let res = emitter.call_typed(Intrinsic::NvidiaShuffleSync, &[mask, data, lane], Type::U32).unwrap();
assert!(res.code().starts_with("__shfl_sync"));
```

## Intentional non-support

CUDA `float2/3/4` are C++ structs and do not have the same math operators as GLSL or WGSL vectors. The emitter **rejects** vector math rather than output incorrect CUDA code. CUDA textures/samplers require a separate handle and API representation; they are **not** automatically lowered from shader `Texture2D`/`Sampler` operations. Threadfences are memory-ordering operations, **not** equivalent to a CTA or warp barrier. CUDA devices are not assumed to support modern warp instructions without querying their capabilities. Inline PTX and Tensor Core MMA are not part of this version. The API emits source only, does not emit PTX, does not compile shaders, and cannot guarantee runtime GPU support.

## Reference material

- [NVIDIA CUDA C++ Programming Guide — warp intrinsics](https://docs.nvidia.com/cuda/cuda-c-programming-guide/)
- [NVIDIA CUDA Math API — device functions and integer intrinsics](https://docs.nvidia.com/cuda/cuda-math-api/)

See the full intrinsic catalog [INTRINSICS.md](INTRINSICS.md) and Rust API documentation [API.md](API.md).
