//! Decode policy and resource limits for the lightweight [`crate::Image`] path.
//!
//! The defaults preserve strict decoding while avoiding metadata work that is not
//! needed to render an image. Limits are deliberately explicit so callers that
//! decode untrusted input can cap the amount of work and memory a file may request.

use crate::ImageError;

/// Controls whether format-specific metadata should be retained by APIs that
/// support optional metadata parsing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetadataMode {
    /// Parse only data required to reconstruct visible pixels. Optional metadata
    /// payloads are skipped and are never decompressed solely for inspection.
    #[default]
    None,
    /// Parse metadata that can influence visible interpretation without retaining
    /// unrelated textual/authoring payloads. Pixel-required structures such as a
    /// PNG palette/transparency table are always parsed regardless of this value.
    Essential,
    /// Parse and retain all metadata supported by the selected decoder/API.
    All,
}

/// Controls how animated containers are decoded by the lightweight image API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AnimationMode {
    /// Decode only the first composited display frame. This is the default for
    /// the lightweight `Image` API so a still-image consumer cannot accidentally
    /// materialize an entire long animation.
    #[default]
    FirstFrame,
    /// Decode every composited display frame.
    AllFrames,
}

/// Packed pixel representation requested from the lightweight decoder.
///
/// `Rgba8888` is currently the crate's canonical representation. Keeping this as
/// an enum allows future output formats to be added without growing a collection
/// of boolean options.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputPixelFormat {
    /// One `u32` per pixel in logical `0xRRGGBBAA` order.
    #[default]
    Rgba8888,
}

/// Resource limits applied before large allocations are made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeLimits {
    /// Maximum number of pixels in one decoded canvas/frame.
    pub max_pixels: usize,
    /// Maximum number of decoded display frames.
    pub max_frames: usize,
    /// Maximum number of output/temporary decompressed bytes a codec may request.
    pub max_decompressed_bytes: usize,
}

impl Default for DecodeLimits {
    fn default() -> Self {
        Self {
            // 268,435,456 pixels == 1 GiB of packed RGBA output. This is high
            // enough for very large legitimate images while still bounding
            // obviously pathological dimensions by default.
            max_pixels: 268_435_456,
            max_frames: 16_384,
            max_decompressed_bytes: 2usize * 1024 * 1024 * 1024,
        }
    }
}

/// Selects the preferred pixel-processing backend for the shared decoder.
///
/// An explicit GPU selection is a **preference**, not a requirement: format
/// parsing and complex entropy decoding (PNG/JPEG/WebP) still run on the CPU.
/// Whole-device pixel decode is implemented for QOI, TGA, WBMP and common BMP
/// subtypes. Large baseline JPEGs can offload inverse-DCT, supported large PNGs can offload 8-bit colour
/// expansion, and supported large WebP images can offload VP8/VP8L pixel
/// reconstruction. Large output resizes may also use OpenCL. Unsupported
/// formats, small images, missing drivers, and GPU failures use the CPU path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum DecodeBackend {
    /// Software decoding, plus existing SIMD optimizations.
    #[default]
    Cpu,
    /// Prefer a GPU with Intel's PCI vendor ID (`0x8086`).
    IntelGpu,
    /// Prefer a GPU with AMD's PCI vendor ID (`0x1002`).
    AmdGpu,
    /// Prefer a GPU with NVIDIA's PCI vendor ID (`0x10de`).
    NvidiaGpu,
}

impl DecodeBackend {
    /// Whether an OpenCL device matching this backend initialized successfully.
    /// The CPU backend is always available. GPU support requires `gpu-opencl`
    /// and a working vendor OpenCL driver at runtime.
    pub fn is_available(self) -> bool {
        crate::gpu::available(self)
    }

    /// Whether this OpenCL GPU compiled the baseline JPEG inverse-DCT kernel.
    /// It is a capability check, not a performance guarantee.
    pub fn supports_jpeg_idct(self) -> bool {
        crate::gpu::jpeg_idct_available(self)
    }

    /// Whether the selected vendor compiled the PNG/WebP reconstruction kernels.
    pub fn supports_png_unpack(self) -> bool {
        crate::gpu::png_unpack_available(self)
    }

    /// Whether the selected vendor compiled the VP8 and VP8L reconstruction kernels.
    pub fn supports_webp_reconstruction(self) -> bool {
        crate::gpu::png_unpack_available(self)
    }

    /// Number of successfully completed GPU PNG colour-expansion kernels.
    pub fn png_dispatches(self) -> u64 { crate::gpu::png_dispatch_count(self) }

    /// Number of successfully completed GPU WebP reconstruction kernels.
    pub fn webp_dispatches(self) -> u64 { crate::gpu::webp_dispatch_count(self) }

    /// Count of successfully completed GPU JPEG-IDCT batches on this backend.
    /// This is process-wide and monotonically increasing, useful for proving
    /// benchmark trials did not silently fall back to software decoding.
    pub fn jpeg_idct_dispatches(self) -> u64 {
        crate::gpu::jpeg_idct_dispatch_count(self)
    }
}

/// GPU execution API used by whole-image device decoders. The preferred
/// hardware vendor is still selected through [`DecodeBackend`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum GpuRuntime {
    /// Prefer native CUDA (NVIDIA) or HIP (AMD), falling back to OpenCL.
    #[default]
    Auto,
    /// Use OpenCL only, with no native API fallback.
    OpenCl,
    /// Use AMD HIP only; an installed compiled HIP bridge is required.
    Hip,
    /// Use NVIDIA CUDA only; an installed compiled CUDA bridge is required.
    Cuda,
}

/// Alternative name for [`DecodeBackend`].
pub type HardwareAcceleration = DecodeBackend;

/// Options for the lightweight [`crate::Image`] decoding path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecodeOptions {
    /// Preferred decode/postprocess backend. Defaults to CPU. See [`DecodeBackend`].
    pub backend: DecodeBackend,
    /// GPU execution API for complete GPU decoders (QOI, TGA and WBMP).
    pub gpu_runtime: GpuRuntime,
    /// Metadata retention policy. The lightweight image path normally leaves it
    /// at [`MetadataMode::None`].
    pub metadata: MetadataMode,
    /// Animation decoding policy.
    pub animation: AnimationMode,
    /// Preferred ICO square size. The nearest available entry is selected.
    pub preferred_icon_size: Option<u32>,
    /// Optional final output dimensions. Codecs may eventually use this during
    /// decode; unsupported codecs are rescaled once after decoding.
    pub output_dimensions: Option<(u32, u32)>,
    /// Requested packed output representation.
    pub pixel_format: OutputPixelFormat,
    /// Whether checksums such as PNG CRCs must be verified.
    pub verify_checksums: bool,
    /// Resource guards checked before large output allocations.
    pub limits: DecodeLimits,
    /// Maximum worker threads used by codec stages that support parallel work.
    /// `None` uses the process-wide detected parallelism.
    pub max_threads: Option<usize>,
}

impl Default for DecodeOptions {
    fn default() -> Self {
        Self {
            backend: DecodeBackend::Cpu,
            gpu_runtime: GpuRuntime::Auto,
            metadata: MetadataMode::None,
            animation: AnimationMode::FirstFrame,
            preferred_icon_size: None,
            output_dimensions: None,
            pixel_format: OutputPixelFormat::Rgba8888,
            verify_checksums: true,
            limits: DecodeLimits::default(),
            max_threads: None,
        }
    }
}

impl DecodeOptions {
    /// Sets the preferred backend while leaving other decode policies unchanged.
    #[inline]
    pub fn with_backend(mut self, backend: DecodeBackend) -> Self {
        self.backend = backend;
        self
    }

    /// Choose OpenCL, native HIP, native CUDA, or automatic runtime selection.
    #[inline]
    pub fn with_gpu_runtime(mut self, runtime: GpuRuntime) -> Self {
        self.gpu_runtime = runtime;
        self
    }

    /// Validates dimensions and returns the exact pixel count.
    #[inline]
    pub(crate) fn checked_pixel_count(
        &self,
        width: u32,
        height: u32,
    ) -> Result<usize, ImageError> {
        let count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| ImageError::new("Image dimensions overflow pixel count"))?;
        if count > self.limits.max_pixels {
            return Err(ImageError::limit(format!(
                "Image requests {count} pixels, exceeding decode limit {}",
                self.limits.max_pixels
            )));
        }
        Ok(count)
    }

    /// Validates packed RGBA output size and returns its byte count.
    #[inline]
    pub(crate) fn checked_rgba_bytes(
        &self,
        width: u32,
        height: u32,
    ) -> Result<usize, ImageError> {
        let bytes = self
            .checked_pixel_count(width, height)?
            .checked_mul(core::mem::size_of::<u32>())
            .ok_or_else(|| ImageError::new("Image output byte size overflow"))?;
        if bytes > self.limits.max_decompressed_bytes {
            return Err(ImageError::limit(format!(
                "Image requests {bytes} decompressed bytes, exceeding decode limit {}",
                self.limits.max_decompressed_bytes
            )));
        }
        Ok(bytes)
    }

    /// Validates an animation frame count before retaining more frame buffers.
    #[inline]
    pub(crate) fn check_frame_count(&self, count: usize) -> Result<(), ImageError> {
        if count > self.limits.max_frames {
            return Err(ImageError::limit(format!(
                "Image requests {count} frames, exceeding decode limit {}",
                self.limits.max_frames
            )));
        }
        Ok(())
    }

    /// Validates a codec-specific decompressed temporary/output allocation.
    #[inline]
    pub(crate) fn check_decompressed_bytes(&self, bytes: usize) -> Result<(), ImageError> {
        if bytes > self.limits.max_decompressed_bytes {
            return Err(ImageError::limit(format!(
                "Decoder requests {bytes} decompressed bytes, exceeding decode limit {}",
                self.limits.max_decompressed_bytes
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lightweight_defaults_skip_metadata_and_materialize_only_first_frame() {
        let options = DecodeOptions::default();
        assert_eq!(options.metadata, MetadataMode::None);
        assert_eq!(options.animation, AnimationMode::FirstFrame);
    }

    #[test]
    fn rejects_pixel_or_rgba_size_overflow_before_allocation() {
        let mut options = DecodeOptions::default();
        options.limits.max_pixels = usize::MAX;
        options.limits.max_decompressed_bytes = usize::MAX;
        assert!(options.checked_rgba_bytes(u32::MAX, u32::MAX).is_err());
    }

    #[test]
    fn enforces_frame_count_limit() {
        let mut options = DecodeOptions::default();
        options.limits.max_frames = 2;
        assert!(options.check_frame_count(2).is_ok());
        assert!(options.check_frame_count(3).is_err());
    }

    #[test]
    fn zero_sized_generic_canvas_is_representable_without_allocation() {
        let options = DecodeOptions::default();
        assert_eq!(options.checked_pixel_count(0, 100).unwrap(), 0);
        assert_eq!(options.checked_rgba_bytes(100, 0).unwrap(), 0);
    }
}
