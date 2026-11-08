//! Shared decoder interface and format-specific image decoder modules.
//!
//! The generic [`crate::Image`] loading path uses static format dispatch and does
//! not allocate trait objects. [`Decoder`] remains as an opt-in polymorphic API
//! for already-constructed rich decoder values. Ownership transfer is consuming
//! so crossing this boundary never requires cloning a full decoded image.

/// Implements the common [`Decoder`] surface for rich formats that retain one
/// decoded RGBA8 frame in `width`, `height`, and `data` fields.
#[cfg(any(feature = "pgm", feature = "tga", feature = "wbmp", feature = "qoi"))]
macro_rules! impl_rgba8_decoder {
    ($type:ty) => {
        impl $crate::decoder::Decoder for $type {
            #[inline]
            fn width(&self) -> u32 {
                self.width
            }

            #[inline]
            fn height(&self) -> u32 {
                self.height
            }

            fn into_image(
                self: Box<Self>,
            ) -> Result<$crate::Image, $crate::ImageError> {
                let this = *self;
                let pixels = $crate::simd::rgba8_to_vec(&this.data);
                $crate::Image::from_packed_pixels(
                    this.width,
                    this.height,
                    pixels,
                    &$crate::DecodeOptions::default(),
                )
            }
        }
    };
}

#[cfg(any(feature = "pgm", feature = "tga", feature = "wbmp", feature = "qoi"))]
pub(crate) use impl_rgba8_decoder;

/// Shared low-level bitstream helpers used by compatible decoders.
mod bit_reader;
/// Shared fixed-width byte-slice readers.
mod byte_reader;
/// Shared helpers for DIB-based BMP/ICO layouts.
mod dib;
#[cfg(feature = "bmp")]
/// Implements BMP image decoding.
pub mod bmp;
#[cfg(feature = "pgm")]
/// Implements PGM image decoding.
pub mod pgm;
#[cfg(feature = "tga")]
/// Implements TGA image decoding.
pub mod tga;
#[cfg(feature = "png")]
/// Implements PNG image decoding.
pub mod png;
#[cfg(feature = "wbmp")]
/// Implements WBMP image decoding.
pub mod wbmp;
#[cfg(feature = "tiff")]
/// Implements TIFF image decoding.
pub mod tiff;
#[cfg(feature = "qoi")]
/// Implements QOI image decoding.
pub mod qoi;
#[cfg(feature = "jpeg")]
/// Implements JPEG image decoding.
pub mod jpeg;
#[cfg(feature = "webp")]
/// Implements WebP image decoding.
pub mod webp;
#[cfg(feature = "avif")]
/// Implements AVIF/AV1 image decoding.
pub mod avif;
#[cfg(feature = "gif")]
/// Implements GIF image decoding.
pub mod gif;
#[cfg(feature = "fits")]
/// Implements FITS image decoding.
pub mod fits;
#[cfg(feature = "svg")]
/// Implements SVG parsing and rasterization.
pub mod svg;
#[cfg(feature = "ico")]
/// Implements ICO container and icon-frame decoding.
pub mod ico;

/// Polymorphic ownership-transfer interface for rich decoder values.
///
/// Unlike the old `pixels(&self) -> Vec<ImageFrame>` API, `into_image` consumes
/// the decoder. Decoders that already own packed pixels can therefore move the
/// allocation directly into [`crate::Image`]. The normal `Image::from_bytes`
/// path does not use this trait; it dispatches directly to lightweight
/// per-format decode functions.
pub trait Decoder {
    /// Returns the decoded image or primary canvas width in pixels.
    fn width(&self) -> u32;
    /// Returns the decoded image or primary canvas height in pixels.
    fn height(&self) -> u32;
    /// Identifies the rich decoder's format when supported, allowing the
    /// shared backend policy to choose between GPU postprocessing and CPU.
    /// Returning `None` conservatively disables automatic GPU selection.
    fn image_format(&self) -> Option<crate::ImageFormat> { None }

    /// Consumes the rich decoder and transfers its decoded output into a
    /// lightweight image without cloning full pixel buffers.
    fn into_image(self: Box<Self>) -> Result<crate::Image, crate::ImageError>;

    /// Converts the rich decoder to a shared image with the requested backend.
    ///
    /// The per-format compressed-data decoder retains its existing CPU path;
    /// large output resizes may run on the selected GPU. Unsupported devices
    /// and unprofitable workloads fall back to CPU without changing pixels.
    /// The original `into_image` remains unchanged for existing applications.
    fn into_image_with_options(
        self: Box<Self>,
        options: crate::DecodeOptions,
    ) -> Result<crate::Image, crate::ImageError> {
        let format = self.image_format();
        let mut image = self.into_image()?;
        options.checked_rgba_bytes(image.width, image.height)?;
        options.check_frame_count(image.frame_count())?;
        if let Some((width, height)) = options.output_dimensions {
            options.checked_rgba_bytes(width, height)?;
            let backend = if format.is_some_and(crate::gpu::gpu_rescale_candidate) {
                options.backend
            } else {
                crate::DecodeBackend::Cpu
            };
            image.rescale_with_backend(width, height, backend);
        }
        Ok(image)
    }
}
