# Optional GPU-assisted image decoding

## What actually runs on the GPU

`DecodeOptions.backend` accepts four choices: `Cpu` (default), `IntelGpu`,
`AmdGpu`, and `NvidiaGpu`. All internal `decode_image(bytes, &DecodeOptions)`
paths receive this setting through the shared dispatcher. Rich `Decoder`
implementations also accept these policies through `into_image_with_options`.

**Baseline JPEG decompression** can now accelerate the inverse discrete cosine
transform (IDCT) on Intel, AMD, and NVIDIA GPUs. CPU Huffman entropy parsing,
DC prediction, dequantization, scan/restart validation, chroma upsampling,
and colour conversion are retained. Completed dequantized 8×8 coefficient
blocks are batched and reconstructed using an integer OpenCL IDCT kernel.
On supported hardware, this is GPU work **inside** compressed JPEG
reconstruction, not merely a post-decode resize. GPU JPEG IDCT requires a
full-resolution baseline JPEG with **at least 2,000,000 pixels** and a compiled
vendor OpenCL kernel. Native scaled JPEG thumbnails and small files stay on CPU.

A requested `output_dimensions` resize can additionally execute a
**real OpenCL nearest-neighbour compute kernel** for sufficiently large images.
The output uses the existing packed `0xRRGGBBAA` representation with exact
integer-coordinate nearest-neighbour mapping. GPU execution is optional and
observably identical to the CPU path. Calling `Image::rescale_with_backend`
on an already decoded image returns `true` when the GPU performed the resize.

The following format families are eligible for GPU-assisted large output
resizing: **PNG, JPEG, WebP, GIF, BMP, FITS, TIFF and AVIF**. The lightweight
**QOI, PGM, TGA, WBMP**, plus **ICO and SVG**, follow the CPU path by default.
For all formats, a resize below **2,000,000 output pixels**, a resize with
unchanged dimensions, a zero-size input, and more than `u32::MAX` input/output
pixels use the CPU path. Existing PNG scratch/stream decoders use the same
post-decode selection. Device failure, out-of-memory, kernel-build failure,
missing driver, or unsupported operating system also use CPU fallback. JPEG
GPU failure causes the remaining IDCT batches to use the original CPU integer
implementation, without discarding the already-decoded entropy stream.

## Enable and use

The `gpu-opencl` feature is enabled by default, and can be disabled separately
in `Cargo.toml` with `default-features = false` (re-enabling desired formats and
`simd` as needed). Builds use no OpenCL SDK, graphics card or linked graphics
library: only `mata-gpu-intrinsics` is needed for source code generation.

```rust
use mata_image::{DecodeBackend, DecodeOptions, Image};

let options = DecodeOptions {
    backend: DecodeBackend::IntelGpu,
    output_dimensions: Some((2560, 1440)),
    ..DecodeOptions::default()
};
let image = Image::from_bytes_with_options(&encoded_bytes, options)?;
// Or Image::from_bytes_with_backend(&encoded_bytes, DecodeBackend::NvidiaGpu)?;
// Or Image::load_with_backend("file.png", DecodeBackend::AmdGpu)?;

println!("Intel OpenCL backend usable: {}", DecodeBackend::IntelGpu.is_available());
println!("Intel JPEG IDCT kernel: {}", DecodeBackend::IntelGpu.supports_jpeg_idct());
let before = DecodeBackend::IntelGpu.jpeg_idct_dispatches();
let jpeg = Image::from_bytes_with_backend(&jpeg_bytes, DecodeBackend::IntelGpu)?;
println!("JPEG GPU batches executed: {}", DecodeBackend::IntelGpu.jpeg_idct_dispatches() - before);
```

The standalone rich-decoder trait accepts options. The rich `Jpeg` type also
has `new_with_backend(&bytes, backend)` and
`from_bytes_with_backend(bytes, backend)` constructors:

```rust
use mata_image::{DecodeBackend, DecodeOptions, Decoder, Pgm};
let decoder = Pgm::new(b"P5\n2 1\n255\n\x10\x80")?;
let decoded = Box::new(decoder).into_image_with_options(
    DecodeOptions {
        backend: DecodeBackend::AmdGpu,
        output_dimensions: Some((128,128)),
        ..Default::default()
    }
)?;
```

## GPU runtime and portability

On Windows, `OpenCL.dll` is dynamically loaded; on Linux, `libOpenCL.so.1`
(or `libOpenCL.so`) is loaded. The runtime queries GPU-class OpenCL devices
and checks the real device `CL_DEVICE_VENDOR_ID` (Intel `0x8086`, AMD `0x1002`,
NVIDIA `0x10de`). It never silently picks a different manufacturer's GPU.
`mata-gpu-intrinsics::intel::KernelBuilder` is used to generate portable
OpenCL C source for **all three vendors**; vendor-specific CUDA/HIP native
compute would require separate runtime integrations in the future. The kernel,
queue and context are initialized once per vendor, cached and serialized for
safe kernel argument binding. Image buffers are uploaded/downloaded per resize;
for small images this can cost more than CPU SIMD, motivating the threshold.

OpenCL support is currently enabled only on **Windows and Linux**. The
JPEG kernel is compiled independently of the resize kernel so a JPEG compiler
failure does not disable GPU resizing. IDCT reconstruction batches cap encoded
coefficients at **16 MiB**, use up to **16 MCU rows** per dispatch, and retain
original coefficients for exact CPU fallback. The additional allocations and
PCIe/integrated-memory transfers can make the GPU path slower on some devices. On other
platforms, compiling with `gpu-opencl` works, but requests fall back to CPU.
If the selected vendor is unavailable or initialization fails, the process
caches the failure until restart. No GPU is needed to compile or use the CPU
backend. The crate's usual resource/size checks still apply before rescaling.

## Limitations / follow-up work

- GPU JPEG inverse-DCT is supported on large *baseline sequential JPEGs*.
  Progressive JPEG, full GPU Huffman decoding, PNG DEFLATE/inflate, and WebP
  entropy or inverse-transform GPU kernels are **not** implemented.
- Output is returned to CPU-owned memory (`Vec<u32>`), not directly to a
  GPU-resident texture. Zero-copy rendering requires a future GPU texture API.
- GPU resampling of already decoded frames is only worthwhile at large sizes.
  Benchmark individual devices and drivers before changing the cutover.
- No promise is made that selecting a GPU will accelerate every decode. Use
  `DecodeBackend::jpeg_idct_dispatches()` before/after to verify JPEG kernel
  execution, or `rescale_with_backend`'s boolean result for resizing.
  `DecodeBackend::is_available()` only reports initialization success.


## GPU PNG / WebP reconstruction

See [README_GPU_PNG_WEBP.md](../../README_GPU_PNG_WEBP.md) for supported stages, CPU fallback and benchmarks. PNG DEFLATE and WebP entropy still run on CPU.
