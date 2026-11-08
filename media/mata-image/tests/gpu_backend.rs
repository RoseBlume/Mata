//! GPU selection must preserve decoded pixels even without a GPU or OpenCL.
use mata_image::{DecodeBackend, DecodeOptions, HardwareAcceleration, Image};

#[test]
fn cpu_is_the_default_and_alias_is_usable() {
    let options = DecodeOptions::default();
    assert_eq!(options.backend, DecodeBackend::Cpu);
    assert!(HardwareAcceleration::Cpu.is_available());
    let explicit = options.with_backend(DecodeBackend::IntelGpu);
    assert_eq!(explicit.backend, DecodeBackend::IntelGpu);
    assert_eq!(options.backend, DecodeBackend::Cpu);
}

#[cfg(feature = "pgm")]
#[test]
fn pgm_uses_identical_cpu_output_for_all_backends() {
    let image = b"P5\n3 2\n255\n\x01\x02\x03\x04\x05\x06";
    let cpu = Image::from_bytes(image).unwrap();
    for backend in [DecodeBackend::Cpu, DecodeBackend::IntelGpu,
                    DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
        assert_eq!(Image::from_bytes_with_backend(image, backend).unwrap(), cpu);
    }
}

#[cfg(feature = "pgm")]
#[test]
fn cpu_only_decoders_ignore_gpu_for_requested_resize() {
    let bytes = b"P5\n3 2\n255\n\x01\x02\x03\x04\x05\x06";
    let options = DecodeOptions {
        backend: DecodeBackend::AmdGpu,
        output_dimensions: Some((11, 9)),
        ..Default::default()
    };
    let image = Image::from_bytes_with_options(bytes, options).unwrap();
    let mut reference = Image::from_bytes(bytes).unwrap();
    reference.rescale(11, 9);
    assert_eq!(image, reference);
}

#[cfg(feature = "bmp")]
#[test]
fn bmp_acceleration_preference_preserves_small_image_pixels() {
    // One 1x1 24-bit BMP, BGR=10,20,30 (with row padding).
    let mut bytes = vec![0u8; 58];
    bytes[0..2].copy_from_slice(b"BM");
    bytes[2..6].copy_from_slice(&(58u32).to_le_bytes());
    bytes[10..14].copy_from_slice(&(54u32).to_le_bytes());
    bytes[14..18].copy_from_slice(&(40u32).to_le_bytes());
    bytes[18..22].copy_from_slice(&(1u32).to_le_bytes());
    bytes[22..26].copy_from_slice(&(1u32).to_le_bytes());
    bytes[26..28].copy_from_slice(&(1u16).to_le_bytes());
    bytes[28..30].copy_from_slice(&(24u16).to_le_bytes());
    bytes[34..38].copy_from_slice(&(4u32).to_le_bytes());
    bytes[54..57].copy_from_slice(&[10, 20, 30]);
    let mut opts = DecodeOptions::default();
    opts.output_dimensions = Some((37, 19));
    let cpu = Image::from_bytes_with_options(&bytes, opts).unwrap();
    for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
        opts.backend = backend;
        assert_eq!(Image::from_bytes_with_options(&bytes, opts).unwrap(), cpu);
    }
}

#[test]
fn rich_image_rescale_with_backend_has_cpu_fallback() {
    let source = Image { width: 4, height: 2, pixels: (0..8u32).collect(), frames: vec![] };
    let mut reference = source.clone();
    reference.rescale(11, 7);
    for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
        let mut result = source.clone();
        assert!(!result.rescale_with_backend(11, 7, backend));
        assert_eq!(result, reference);
    }
}

#[cfg(feature = "pgm")]
#[test]
fn rich_decoder_trait_accepts_backend_options() {
    use mata_image::{Decoder, Pgm};
    let bytes = b"P5\n2 1\n255\n\x01\xff";
    let rich = Pgm::new(bytes).unwrap();
    let image = Box::new(rich).into_image_with_options(
        DecodeOptions { backend: DecodeBackend::NvidiaGpu,
            output_dimensions: Some((17, 9)), ..Default::default() }
    ).unwrap();
    assert_eq!((image.width,image.height), (17,9));
}

// Run explicitly with `cargo test --test gpu_backend hardware_gpu_rescale -- --ignored`
// on a machine with a vendor OpenCL driver. The result must match CPU output.
#[cfg(feature = "gpu-opencl")]
#[test]
#[ignore = "requires a supported hardware OpenCL GPU"]
fn hardware_gpu_rescale_matches_cpu_exactly() {
    let width = 2048u32;
    let height = 1024u32;
    let source = Image {
        width, height,
        pixels: (0..(width as usize * height as usize))
            .map(|v| (v as u32).wrapping_mul(0x9e37_79b9))
            .collect(),
        frames: vec![],
    };
    let mut cpu = source.clone();
    cpu.rescale(2051, 1027);
    for backend in [DecodeBackend::IntelGpu, DecodeBackend::AmdGpu, DecodeBackend::NvidiaGpu] {
        if backend.is_available() {
            let mut gpu = source.clone();
            assert!(gpu.rescale_with_backend(2051, 1027, backend), "GPU failed: {backend:?}");
            assert_eq!(gpu, cpu, "GPU pixel mismatch: {backend:?}");
        }
    }
}
