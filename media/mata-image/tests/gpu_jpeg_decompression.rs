//! Validates actual GPU participation in JPEG reconstruction and CPU fallback.
//! Fixtures are generated in memory, so benchmark binary assets are not needed.
#![cfg(feature = "jpeg")]

use mata_image::{DecodeBackend, DecodeOptions, Image};

/// 8-bit baseline grayscale JPEG containing a chosen 8x8 DCT coefficient.
/// EOB-only uses DC=0; AC mode adds coefficient (0,1) to every block.
fn synthetic_baseline_jpeg(width: u16, height: u16, ac: bool) -> Vec<u8> {
    assert_eq!(width % 8, 0);
    assert_eq!(height % 8, 0);
    let blocks = (width as usize / 8) * (height as usize / 8);
    assert_eq!(blocks % 4, 0);
    let mut output = vec![0xff, 0xd8];
    // DQT, 8-bit table zero. Nonzero AC coefficients visibly affect IDCT.
    output.extend_from_slice(&[0xff, 0xdb, 0, 67, 0]);
    output.extend(std::iter::repeat_n(50u8, 64));
    // SOF0: grayscale 8-bit, 1x1 sampling, table 0.
    output.extend_from_slice(&[0xff, 0xc0, 0, 11, 8]);
    output.extend_from_slice(&height.to_be_bytes());
    output.extend_from_slice(&width.to_be_bytes());
    output.extend_from_slice(&[1, 1, 0x11, 0]);
    // DC Huffman table: one 1-bit code '0' meaning size 0.
    output.extend_from_slice(&[0xff, 0xc4, 0, 20, 0]);
    output.push(1);
    output.extend(std::iter::repeat_n(0u8, 15));
    output.push(0);
    // AC table: EOB code '0' alone, or EOB '00' and AC(0,1) '01'.
    output.extend_from_slice(&[0xff, 0xc4, 0, if ac { 21 } else { 20 }, 0x10]);
    if ac {
        output.extend_from_slice(&[0, 2]);
        output.extend(std::iter::repeat_n(0u8, 14));
    } else {
        output.push(1);
        output.extend(std::iter::repeat_n(0u8, 15));
    }
    output.push(0);
    if ac { output.push(0x01); }
    // SOS: component 1 uses DC0 / AC0; baseline full spectral selection.
    output.extend_from_slice(&[0xff, 0xda, 0, 8, 1, 1, 0, 0, 63, 0]);
    // Each AC block: DC '0' + AC '01' + magnitude '1' + EOB '00'
    // = 001100 (six bits). Pack four blocks into three bytes.
    if ac {
        for _ in 0..blocks / 4 { output.extend_from_slice(&[0x30, 0xc3, 0x0c]); }
    } else {
        // Each DC/EOB-only block consumes two bits: '0' + '0'.
        output.extend(std::iter::repeat_n(0u8, blocks / 4));
    }
    output.extend_from_slice(&[0xff, 0xd9]);
    output
}

#[test]
fn synthetic_baseline_jpeg_decodes_with_cpu_and_gpu_preferences() {
    for ac in [false, true] {
        let bytes = synthetic_baseline_jpeg(32, 32, ac);
        let reference = Image::from_bytes(&bytes).unwrap();
        assert_eq!((reference.width, reference.height), (32, 32));
        if ac { assert_ne!(reference.pixels[0], reference.pixels[1]); }
        for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
            let actual = Image::from_bytes_with_options(&bytes, DecodeOptions {
                backend, ..DecodeOptions::default()
            }).unwrap();
            assert_eq!(actual, reference);
        }
    }
}

#[test]
fn gpu_jpeg_capability_and_dispatch_counters_are_consistent() {
    assert!(!DecodeBackend::Cpu.supports_jpeg_idct());
    assert_eq!(DecodeBackend::Cpu.jpeg_idct_dispatches(), 0);
    for vendor in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
        if vendor.supports_jpeg_idct() { assert!(vendor.is_available()); }
    }
}

// On machines with an Intel/AMD/NVIDIA OpenCL GPU, this test checks that
// reconstruction truly went through the GPU AND matches the CPU pixel buffer.
#[test]
#[ignore = "requires a vendor OpenCL GPU and driver"]
fn large_jpeg_gpu_inverse_dct_matches_cpu_exactly() {
    for ac in [false, true] {
        let bytes = synthetic_baseline_jpeg(2048, 1024, ac);
        let cpu = Image::from_bytes(&bytes).unwrap();
        for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
            if !backend.supports_jpeg_idct() { continue; }
            let before = backend.jpeg_idct_dispatches();
            let actual = Image::from_bytes_with_options(&bytes, DecodeOptions {
                backend, ..DecodeOptions::default()
            }).unwrap();
            assert!(backend.jpeg_idct_dispatches() > before,
                "{backend:?}: kernel did not execute");
            assert_eq!(actual, cpu, "{backend:?} differed from CPU output");
        }
    }
}
