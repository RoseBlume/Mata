//! PNG-specific performance harness. No external benchmark dependencies.
//! Run: cargo bench -p mata_image --bench png_hotpath --features gpu-opencl
//! For a large custom image: MATAIMAGE_PNG_BENCH_FILE=/path/to/test.png
#![allow(clippy::missing_panics_doc)]

#[cfg(not(feature = "png"))]
fn main() {
    println!("enable the png feature to run this benchmark");
}

#[cfg(feature = "png")]
mod png_bench {
    use mata_image::{DecodeBackend, DecodeOptions, Image};
    use std::{fs, hint::black_box, io::Cursor, time::{Duration, Instant}};

    fn run_once(bytes: &[u8], backend: DecodeBackend, streaming: bool) {
        let options = DecodeOptions { backend, ..DecodeOptions::default() };
        if streaming {
            let mut reader = Cursor::new(bytes);
            black_box(Image::from_reader_with_options(&mut reader, options).unwrap());
        } else {
            black_box(Image::from_bytes_with_options(black_box(bytes), options).unwrap());
        }
    }

    // Median of independent runs, each amortized over enough decodes to
    // reduce clock granularity/noise on small PNGs.
    fn bench(bytes: &[u8], backend: DecodeBackend, streaming: bool) -> f64 {
        run_once(bytes, backend, streaming);
        let mut times = [0.0; 5];
        for sample in &mut times {
            let start = Instant::now();
            let mut iterations = 0u32;
            loop {
                run_once(bytes, backend, streaming);
                iterations += 1;
                if iterations >= 64 || start.elapsed() >= Duration::from_millis(90) {
                    break;
                }
            }
            *sample = start.elapsed().as_secs_f64() * 1000.0 / iterations as f64;
        }
        times.sort_by(|a, b| a.total_cmp(b));
        times[2]
    }

    fn case(name: &str, bytes: &[u8]) {
        let reference = Image::from_bytes(bytes).expect("benchmark PNG must decode");
        let mut stream = Cursor::new(bytes);
        assert_eq!(reference, Image::from_reader(&mut stream).unwrap(), "{name}");
        let native = bench(bytes, DecodeBackend::Cpu, false);
        let streaming = bench(bytes, DecodeBackend::Cpu, true);
        let megapixels = reference.width as f64 * reference.height as f64 / 1_000_000.0;
        println!("{name:<27} {:>4}x{:<4} native {:>8.4} ms {:>8.1} MP/s | stream {:>8.4} ms | native/stream {:>5.2}x",
            reference.width, reference.height, native, megapixels / native * 1000.0,
            streaming, streaming / native);

        // A GPU candidate is only reported if an actual GPU kernel ran,
        // not merely if a driver happens to be installed.
        for gpu in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
            if !gpu.supports_png_unpack() { continue; }
            let before = gpu.png_dispatches();
            let selected = Image::from_bytes_with_options(bytes,
                DecodeOptions { backend: gpu, ..DecodeOptions::default() }).unwrap();
            if gpu.png_dispatches() <= before { continue; }
            assert_eq!(selected, reference, "{name}: {gpu:?}");
            let ms = bench(bytes, gpu, false);
            println!("    {gpu:?}: {ms:.4} ms ({:.2}x vs native CPU)", native / ms);
        }
    }

    pub(super) fn main() {
        println!("PNG hot path benchmark: CPU single IDAT direct slice vs real Reader/multi IDAT");
        case("rgb8_single", include_bytes!("../tests/fixtures_png_perf/rgb8_single.png"));
        case("rgb8_multi", include_bytes!("../tests/fixtures_png_perf/rgb8_multi.png"));
        case("rgba8_single", include_bytes!("../tests/fixtures_png_perf/rgba8_single.png"));
        case("rgba8_multi", include_bytes!("../tests/fixtures_png_perf/rgba8_multi.png"));
        case("narrow_stored", include_bytes!("../tests/fixtures_png_perf/narrow_0.png"));
        case("wide_stored", include_bytes!("../tests/fixtures_png_perf/wide_stored.png"));
        if let Ok(path) = std::env::var("MATAIMAGE_PNG_BENCH_FILE") {
            let bytes = fs::read(&path).expect("read custom PNG path");
            case("user-provided PNG", &bytes);
        }
    }
}

#[cfg(feature = "png")]
fn main() { png_bench::main(); }
