# GPU auto-tuning benchmark (`benches/decode_gpu.rs`)

This is a standalone variant of the supplied `decode.rs`; the original
`decode.rs` is preserved. It uses only Rust std plus existing mata-image APIs.

## Running

From the `mata-image` directory, with the same `benches/assets/` fixture set
used by `decode.rs`:

```shell
cargo bench --bench decode_gpu --features gpu-opencl
```

To limit the calibration and measured images to a particular fixture set:

```shell
MATAIMAGE_BENCH_FILTER=jpeg MATAIMAGE_BENCH_TUNE_ROUNDS=5 cargo bench --bench decode_gpu --features gpu-opencl
```

On Windows PowerShell:

```powershell
$env:MATAIMAGE_BENCH_FILTER = "jpeg"
$env:MATAIMAGE_BENCH_TUNE_ROUNDS = "5"
cargo bench --bench decode_gpu --features gpu-opencl
```

## How profiles are selected

1. CPU parallelism and Intel/AMD/NVIDIA OpenCL availability are detected once
   per benchmark process using `OnceLock`. GPU driver/kernel initialization
   is cached internally by mata-image.
2. Available fixture images are grouped by `ImageFormat` and one
   representative valid-size sample per format is selected. Prefer a still
   image of 1-8 MP; skip excessive total decoded frame sizes.
3. Each format's **native decode** is calibrated with CPU thread counts
   `auto, 1, 2, 4, 8, logical_cpu_count` (bounded to CPU availability).
   For JPEG samples of at least 2 MP, eligible Intel/AMD/NVIDIA GPU candidates
   are then measured for **native-size GPU inverse-DCT reconstruction**. A
   candidate must increment its successful IDCT dispatch counter (not merely
   claim that the OpenCL device is available) and produce CPU-identical pixels.
4. If supported, a separate **decode + resize** workload is calibrated at
   roughly 3 MP output (above the 2 MP minimum GPU dispatch threshold).
   Compare CPU `auto`, best native CPU profile, and eligible vendor GPUs
   with the best native thread count. A GPU contender must first prove that
   an actual resize kernel executes, not merely that GPU fallback is safe.
5. Candidates must decode to *exactly* the same image pixels and frames as a
   CPU reference. Each candidate is warmed up and sampled repeatedly; median
   end-to-end wall time determines the winner with 3% hysteresis. This time
   includes CPU entropy decode, GPU transfers, scheduling, and output copy.
6. Profiles are cached in a process-wide `OnceLock<HashMap<ImageFormat, FormatProfiles>>`
   and reused for all primary end-to-end benchmark images. Unsupported formats
   and uncalibrated workloads use CPU fallback.

`MATAIMAGE_BENCH_TUNE_ROUNDS` adjusts median samples (1..=9, default 3).
The existing `MATAIMAGE_BENCH_FILTER` selects fixtures before calibration.
The auxiliary stage/thread/third-party comparisons remain intentionally
independent baseline tests, unless they explicitly use `profile_for`.

## Important limits

* GPU acceleration covers *large JPEG inverse-DCT reconstruction* and
  *large nearest-neighbour resizing*. JPEG entropy/Huffman decoding is CPU;
  PNG DEFLATE, WebP entropy decoding and full GPU bitstream decompression
  are not yet accelerated.
* Device availability is not a guarantee a later operation will run on GPU;
  failures fall back to CPU. The GPU trial qualification checks that the kernel
  ran on its representative test image. JPEG decompression trials separately
  require a successful vendor GPU-IDCT dispatch.
* One profile per format is a useful local heuristic, not a universal speed
  optimum for every image size, frame count, or CPU/GPU load.
* Profiles last only until the benchmark process exits; there is no persistent
  cache, and no change to mata-image's default runtime options.
* The archives used to build this benchmark did **not** include the binary
  `benches/assets/` fixtures or the sibling `../remata-macros` dependency.
  Supply them from your existing local workspace to run the benchmark.
