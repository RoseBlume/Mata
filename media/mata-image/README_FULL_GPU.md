# GPU decode update: see [PNG/VP8L device decoding](README_PNG_WEBP_DEVICE_DECODING.md) for new GPU bitstream paths.

# Whole-image GPU decoding: OpenCL, AMD HIP, NVIDIA CUDA

## Status and exact scope

This release **does not implement every image format entirely on the GPU**. It adds
whole-image **pixel decoding** for four specific codec families, including their
encoded pixels and (where present) RLE/other dependent opcode loops. Host CPU
code still validates headers, checks resource limits, chooses devices, transfers
buffers, and constructs the public `Image` value.

| Format | Whole-GPU subset | Existing path for other variants |
| --- | --- | --- |
| QOI | QOI RGB/RGBA, INDEX, DIFF, LUMA, RUN opcodes | CPU if GPU fails |
| TGA | 24/32-bit type-2 raw and type-10 RLE, all origin flags | CPU for unsupported forms |
| WBMP | Type-0 1-bit rows | CPU fallback |
| BMP | BI_RGB 24/32-bit, including bottom-up/top-down rows | CPU for palette, RLE, bitfields, etc. |
| PNG | **Not whole-GPU**: CPU DEFLATE and scanline filters, optional GPU colour expansion | CPU |
| JPEG | **Not whole-GPU**: CPU Huffman/marker parsing, optional GPU inverse DCT | CPU |
| WebP | **Not whole-GPU**: CPU VP8/VP8L entropy, optional GPU colour transforms | CPU |
| GIF | Not whole-GPU: CPU LZW and compositing | CPU |
| TIFF | Not whole-GPU | CPU |
| FITS | Not whole-GPU | CPU |
| ICO | Not whole-GPU | CPU |
| AVIF | Not whole-GPU | existing CPU/rav1d decoder |
| PGM | Not whole-GPU | CPU |
| SVG | Not a conventional compressed raster; existing CPU rasterizer | CPU |

### GPU runtime choices

- `GpuRuntime::OpenCl`: OpenCL 1.2 compiled by the GPU driver at runtime; no SDK at Rust build time.
- `GpuRuntime::Hip`: vendor-native AMD HIP bridge (`gpu-hip` feature); requires independently compiled shared library.
- `GpuRuntime::Cuda`: NVIDIA CUDA bridge (`gpu-cuda` feature); requires independently compiled shared library.
- `GpuRuntime::Auto`: for AMD try HIP then OpenCL; for NVIDIA try CUDA then OpenCL; Intel uses OpenCL.
- `DecodeBackend::Cpu` always uses CPU. Intel/AMD/NVIDIA backend selection does not change.
- Native GPU runtimes are used only for **whole-image paths** currently. Existing partial GPU stages (JPEG/PNG/WebP/rescale) still use the established OpenCL runtime.
- An explicit HIP or CUDA selection does not secretly change to OpenCL; it falls back to the CPU codec if that native bridge is unavailable.

### Example

```rust
use mata_image::{DecodeBackend, DecodeOptions, GpuRuntime, Image, ImageFormat};

let options = DecodeOptions::default()
    .with_backend(DecodeBackend::NvidiaGpu)
    .with_gpu_runtime(GpuRuntime::Cuda);
let before = Image::full_gpu_dispatches(ImageFormat::Qoi);
let image = Image::from_bytes_with_options(&qoi_data, options)?;
let used_gpu = Image::full_gpu_dispatches(ImageFormat::Qoi) > before;
println!("{}x{}, GPU kernel executed: {used_gpu}", image.width, image.height);
```

Counters are **process-wide per format** and may advance from concurrent decodes,
so they are diagnostics and benchmark hints, not a per-image proof when parallel
requests are active. Request results always return an `Image`, not a GPU allocation.

### Build and install

Rust library/OpenCL-only:

```shell
cargo test -p mata-image --features gpu-opencl --test full_gpu_decoders
cargo bench -p mata-image --bench full_gpu --features gpu-opencl
```

On Linux, from `mata-image/native-gpu`:

```shell
./build.sh cuda    # requires nvcc / CUDA toolkit
./build.sh hip     # requires hipcc / ROCm HIP SDK
# e.g. before launching your application:
export LD_LIBRARY_PATH="$(pwd):${LD_LIBRARY_PATH:-}"
```

On Windows PowerShell (run in `mata-image/native-gpu`):

```powershell
.\build.ps1 -Runtime cuda   # requires NVIDIA CUDA toolkit
.\build.ps1 -Runtime hip    # requires AMD HIP SDK
$env:PATH = "$PWD;$env:PATH"
```

Then rebuild/check your Rust project with `--features gpu-cuda,gpu-hip`
(or add those features in your dependent crate). Native bridges are deliberately
loaded from system library paths **at runtime**. The build does not dynamically
compile HIP/CUDA sources or require GPU toolkits at Cargo build time.

### Performance and limitations

- QOI and RLE TGA contain dependent streams. Their device kernels intentionally
  execute with **one work-item per image** to preserve correctness. These versions
  may be dramatically slower than optimized CPU decoders because the data is
  copied to and from GPU memory and there is minimal internal parallelism.
- WBMP, raw TGA and uncompressed BMP execute independent pixels in parallel.
- For realistic speed, measure end-to-end times and let `decode_gpu` choose CPU
  when launch+transfer time exceeds CPU decode time; never assume a GPU wins.
- On a failed device initialization, launch, data transfer or device decoder
  status check, the existing CPU decoder handles the original encoded input.
- QOI/truncated TGA errors are still surfaced by the CPU decoder after fallback;
  successful whole-device output is not accepted if the kernel reports an error.
- The CUDA and HIP source files are supplied, but there is no available CUDA/HIP
  toolchain or physical GPU in the build environment. **Real hardware execution
  and Rust compilation are not verified.** OpenCL 1.2 source syntax and both
  device-algorithm implementations were checked with native CPU simulations.

### Remaining implementation work for entire GPU decoders

Actual complete GPU versions of PNG/JPEG/WebP require independent GPU implementations
of DEFLATE and scanline dependency resolution, JPEG entropy/Huffman and restart
interval decoding, and the several VP8/VP8L entropy/prediction pipelines respectively.
GIF, TIFF, AVIF and other formats each need separate container and entropy
implementations, independent golden-image tests, and performance profiling.
These are not implemented in this archive; selecting a GPU must not suggest otherwise.
