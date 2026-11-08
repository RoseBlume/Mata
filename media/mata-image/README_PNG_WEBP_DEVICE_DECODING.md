# PNG and WebP compressed-data GPU decoding (OpenCL / HIP / CUDA)

## Implemented scope (important)

**This is a substantial GPU codec implementation, not complete GPU support for every PNG/WebP variant.**

The shared decoder now attempts an actual on-device compressed-bitstream decoder for supported inputs. CUDA and HIP kernels run in native shared libraries; OpenCL kernels are compiled by the installed driver. No CPU DEFLATE or VP8L Huffman decoder is involved in a successful full-device dispatch. Headers, image limits, RIFF/PNG chunk parsing and optional PNG CRCs are validated on the CPU, as is normal for GPU decoding.

| Format | Device-only stages | Supported file profile | CPU fallback |
| --- | --- | --- | --- |
| PNG | zlib header, stored/fixed/dynamic DEFLATE, LZ77, Adler-32, PNG filters 0-4, 8-bit RGBA packing | Non-interlaced 8-bit grayscale, gray+alpha, RGB, RGBA; single/multi-IDAT | Indexed/palette and tRNS, sub-8-bit, 16-bit, unsupported/failed GPU |
| WebP lossless VP8L | Huffman trees (simple and complex), entropy literals, LZ77, WebP backward distance mapping, small colour cache, ARGB-to-RGBA | Simple static RIFF/VP8L images, no transforms or meta-Huffman-group image, cache size <= 256 | VP8L predictors, colour transforms, colour-index transforms, meta-Huffman groups, larger caches, animation |
| WebP lossy VP8 | Previously implemented GPU YUV420->RGBA conversion | Existing eligible large VP8 samples, after **CPU entropy/inverse prediction** | CPU when unsupported |

**VP8 lossy WebP is not fully GPU decompressed.** Full VP8 lossy arithmetic range decoding and macroblock reconstruction, VP8L transforms/meta-Huffman groups and less common PNG pixel layouts are remaining work. Do not present these cases as whole-GPU decoding. Multi-frame WebP continues to use its existing animation path.

## Performance reality

Both new compressed-data GPU kernels deliberately use **one work-item per image** to respect entropy and inter-row dependencies. They prove end-to-end device decoding for the above profiles; they are **not** optimized massively parallel decoders and can be much slower than mata-image's optimized CPU implementation. PCIe copies, kernel compilation, device setup and output readback are included in benchmarks. Use the existing `decode_gpu` auto-tuner to choose the faster verified profile. It compares output pixels and checks actual dispatch counts, including these new PNG/VP8L paths.

## Rust API

```rust
use mata_image::{DecodeBackend, DecodeOptions, GpuRuntime, Image, ImageFormat};

let options = DecodeOptions::default()
    .with_backend(DecodeBackend::NvidiaGpu)
    .with_gpu_runtime(GpuRuntime::Cuda);
let before = Image::full_gpu_dispatches(ImageFormat::Png);
let image = Image::from_bytes_with_options(&png_bytes, options)?;
println!("whole-device PNG calls: {}", Image::full_gpu_dispatches(ImageFormat::Png)-before);
```

For WebP, use `ImageFormat::WebP`. The dispatch counter is global, so concurrent decodes could also increment it. Explicit HIP/CUDA selection does not silently switch to OpenCL; `Auto` chooses native then OpenCL when available.

## Build

```sh
cargo test -p mata-image --test gpu_entire_png_webp --features gpu-opencl,gpu-hip,gpu-cuda
cargo bench -p mata-image --bench full_gpu --features gpu-opencl,gpu-hip,gpu-cuda
cargo bench -p mata-image --bench decode_gpu --features gpu-opencl,gpu-hip,gpu-cuda
```

Native GPU bridges are optional. Build separately from `mata-image/native-gpu/` with `./build.sh cuda` (nvcc) or `./build.sh hip` (hipcc), or `build.ps1` on Windows. Ensure the resulting shared library is on the dynamic loader's search path. Pure `gpu-opencl` works without those toolkits.

## Validation completed here

- Clang OpenCL 1.2 syntax validation for both new kernels.
- CUDA/HIP shared decoding code compiled together under a CPU-execution shim (not nvcc/hipcc).
- PNG: 100 generated variants spanning all 4 common 8-bit channel layouts, filters 0-4, 5 compression level/strategy combinations; 19 existing real PNG fixtures agreed with an independent PNG decoder.
- WebP: 5 synthetic RIFF/VP8L images, accepted by Pillow's WebP decoder and matched byte-for-byte, including two-symbol Huffman cases.
- Native host-bridge simulation produced matching pixels for representative PNG and VP8L fixtures.

**Not verified:** Cargo build/tests (Rust toolchain unavailable), native nvcc/hipcc build, actual GPU OpenCL/HIP/CUDA execution, cross-vendor performance. Treat the implementation as experimental until those are run on target devices.
