//! Implements the `renderer` module.
//!
//! This module documents both public API items and internal implementation details.

use std::ffi::c_void;

use crate::buffer::Framebuffer;
use super::DirtyRect;

use windows::{
    core::Result,
    Win32::{
        Foundation::HWND,
        Graphics::{
            Direct2D::{
                Common::{ D2D1_ALPHA_MODE_IGNORE, D2D1_PIXEL_FORMAT, D2D_RECT_F, D2D_RECT_U, D2D_SIZE_U },
                D2D1CreateFactory,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                D2D1_BITMAP_PROPERTIES,
                D2D1_FACTORY_TYPE_SINGLE_THREADED,
                D2D1_HWND_RENDER_TARGET_PROPERTIES,
                D2D1_PRESENT_OPTIONS_NONE,
                D2D1_RENDER_TARGET_PROPERTIES,
                ID2D1Bitmap,
                ID2D1Factory,
                ID2D1HwndRenderTarget,
            },
            Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM,
        },
    },
};

/// Direct2D presentation backend for the CPU framebuffer.
///
/// The bitmap is reused while its size is unchanged and recreated only when
/// the application submits a differently-sized framebuffer. WidgetWindow keeps
/// that bitmap exactly equal to the native client size after every resize, so
/// this backend performs a 1:1 pixel presentation instead of DPI upscaling. A normal frame
/// therefore performs only:
///
/// 1. one CPU -> Direct2D bitmap upload (`CopyFromMemory`),
/// 2. one `DrawBitmap`,
/// 3. one `EndDraw`/present.
///
/// The pixel format intentionally matches the old 32-bit `BI_RGB` GDI path:
/// bytes in memory are interpreted as B, G, R, X/A. This keeps existing
/// framebuffer colour packing compatible with the previous backend.
pub(super) struct Direct2DRenderer {
    /// Stores factory.
    ///
    /// Visibility: **private**.
    factory: ID2D1Factory,
    /// Stores render target.
    ///
    /// Visibility: **private**.
    render_target: Option<ID2D1HwndRenderTarget>,
    /// Stores bitmap.
    ///
    /// Visibility: **private**.
    bitmap: Option<ID2D1Bitmap>,
    /// Stores framebuffer size.
    ///
    /// Visibility: **private**.
    framebuffer_size: D2D_SIZE_U,
    /// Stores target size.
    ///
    /// Visibility: **private**.
    target_size: D2D_SIZE_U,
    /// Stores destination.
    ///
    /// Visibility: **private**.
    destination: D2D_RECT_F,
    /// Newly-created or resized bitmaps require a full upload before partial updates.
    bitmap_initialized: bool,
    /// A changed target needs a presentation even if no source pixels changed.
    needs_present: bool,
}

impl Direct2DRenderer {
    /// Creates a new instance.
    ///
    /// Visibility: **parent-module-visible**.
    #[doc = "Creates a new value. Visibility: parent-module-visible."]
    pub(super) fn new(hwnd: HWND, framebuffer_size: (u32, u32), target_size: (u32, u32)) -> Self {
        let factory = unsafe {
            D2D1CreateFactory::<ID2D1Factory>(D2D1_FACTORY_TYPE_SINGLE_THREADED, None).expect(
                "D2D1CreateFactory failed"
            )
        };

        let mut renderer = Self {
            factory,
            render_target: None,
            bitmap: None,
            framebuffer_size: D2D_SIZE_U {
                width: framebuffer_size.0,
                height: framebuffer_size.1,
            },
            target_size: D2D_SIZE_U {
                width: target_size.0,
                height: target_size.1,
            },
            destination: D2D_RECT_F {
                left: 0.0,
                top: 0.0,
                right: target_size.0 as f32,
                bottom: target_size.1 as f32,
            },
            bitmap_initialized: false,
            needs_present: true,
        };

        // A minimized window has a 0x0 client area. In that case resources are
        // created lazily when the window becomes visible again.
        if target_size.0 != 0 && target_size.1 != 0 {
            renderer
                .create_device_resources(hwnd)
                .expect("Failed to create Direct2D device resources");
        }

        renderer
    }

    /// Performs the create device resources operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the create device resources operation. Visibility: private/internal."]
    fn create_device_resources(&mut self, hwnd: HWND) -> Result<()> {
        if
            self.target_size.width == 0 ||
            self.target_size.height == 0 ||
            self.framebuffer_size.width == 0 ||
            self.framebuffer_size.height == 0
        {
            return Ok(());
        }

        if self.render_target.is_none() {
            // WidgetWindow rasterizes directly in native client pixels and the
            // Win32 pointer messages are also expressed in client coordinates.
            // Force the Direct2D target to 96 DPI so one D2D unit is exactly one
            // client/framebuffer pixel. Leaving dpiX/dpiY at 0 asks Direct2D to
            // inherit the monitor DPI; under per-monitor DPI awareness that
            // scales the already-native-size framebuffer a second time and makes
            // the visible controls disagree with hit-testing coordinates.
            let render_target_properties = D2D1_RENDER_TARGET_PROPERTIES {
                dpiX: 96.0,
                dpiY: 96.0,
                ..Default::default()
            };
            let hwnd_properties = D2D1_HWND_RENDER_TARGET_PROPERTIES {
                hwnd,
                pixelSize: self.target_size,
                // Let Direct2D/DWM pace presentation instead of forcing immediate full-surface uploads.
                presentOptions: D2D1_PRESENT_OPTIONS_NONE,
            };

            let render_target = unsafe {
                self.factory.CreateHwndRenderTarget(&render_target_properties, &hwnd_properties)?
            };
            self.render_target = Some(render_target);
        }

        if self.bitmap.is_none() {
            let bitmap_properties = D2D1_BITMAP_PROPERTIES {
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_IGNORE,
                },
                dpiX: 96.0,
                dpiY: 96.0,
            };

            let render_target = self.render_target
                .as_ref()
                .expect("Direct2D render target missing after creation");
            let bitmap = unsafe {
                render_target.CreateBitmap(
                    self.framebuffer_size,
                    None,
                    self.framebuffer_size.width.saturating_mul(4),
                    &bitmap_properties
                )?
            };
            self.bitmap = Some(bitmap);
            self.bitmap_initialized = false;
            self.needs_present = true;
        }

        Ok(())
    }

    /// Performs the discard device resources operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the discard device resources operation. Visibility: private/internal."]
    fn discard_device_resources(&mut self) {
        // Bitmap is tied to the render target, so release it first.
        self.bitmap = None;
        self.render_target = None;
        self.bitmap_initialized = false;
        self.needs_present = true;
    }

    /// Performs the shutdown operation.
    ///
    /// Visibility: **parent-module-visible**.
    #[doc = "Implements the shutdown operation. Visibility: parent-module-visible."]
    pub(super) fn shutdown(&mut self) {
        self.discard_device_resources();
    }

    /// Changes the CPU source bitmap size without scaling old pixels into the
    /// new window size. The next present creates a matching Direct2D bitmap.
    #[doc = "Resizes framebuffer. Visibility: parent-module-visible."]
    pub(super) fn resize_framebuffer(&mut self, size: (u32, u32)) {
        let new_size = D2D_SIZE_U {
            width: size.0,
            height: size.1,
        };
        if self.framebuffer_size == new_size {
            return;
        }

        self.framebuffer_size = new_size;
        // The Direct2D bitmap has immutable dimensions, but the HWND render
        // target does not need to be recreated just because the source changes.
        self.bitmap = None;
        self.bitmap_initialized = false;
        self.needs_present = true;
    }

    /// Performs the resize target operation.
    ///
    /// Visibility: **parent-module-visible**.
    #[doc = "Resizes target. Visibility: parent-module-visible."]
    pub(super) fn resize_target(&mut self, hwnd: HWND, size: (u32, u32)) {
        let new_size = D2D_SIZE_U {
            width: size.0,
            height: size.1,
        };

        if self.target_size == new_size {
            return;
        }

        self.target_size = new_size;
        self.destination.right = size.0 as f32;
        self.destination.bottom = size.1 as f32;
        self.needs_present = true;

        if size.0 == 0 || size.1 == 0 {
            return;
        }

        if let Some(target) = &self.render_target {
            if (unsafe { target.Resize(&new_size) }).is_err() {
                self.discard_device_resources();
            }
        }

        if self.render_target.is_none() {
            let _ = self.create_device_resources(hwnd);
        }
    }

    /// Copies only the caller's damaged rectangles, using the original
    /// framebuffer stride. Avoids extra row buffers and per-frame allocations.
    /// A larger damaged fraction or many scattered rectangles falls back to
    /// one full upload, reducing the number of GPU/driver calls.
    fn upload_damage(&self, pixels: *const c_void, dirty_regions: &[DirtyRect]) -> Result<()> {
        let Some(bitmap) = self.bitmap.as_ref() else {
            return Ok(());
        };
        let width = self.framebuffer_size.width;
        let height = self.framebuffer_size.height;
        let pitch = width.checked_mul(4).expect("Framebuffer stride exceeds u32");
        let total_area = u64::from(width) * u64::from(height);

        // First draw (or legacy `present`) is always full. The initial D2D
        // bitmap has undefined contents until it receives the first upload.
        if !self.bitmap_initialized || total_area == 0 {
            return unsafe { bitmap.CopyFromMemory(None, pixels, pitch) };
        }

        let mut total_dirty_area = 0u64;
        let mut union: Option<DirtyRect> = None;
        let mut count = 0usize;
        for rect in dirty_regions.iter().filter_map(|r| r.clipped(width, height)) {
            count += 1;
            total_dirty_area = total_dirty_area.saturating_add(
                u64::from(rect.width) * u64::from(rect.height)
            );
            union = Some(match union {
                None => rect,
                Some(bounds) => {
                    let left = bounds.x.min(rect.x);
                    let top = bounds.y.min(rect.y);
                    let right = (bounds.x + bounds.width).max(rect.x + rect.width);
                    let bottom = (bounds.y + bounds.height).max(rect.y + rect.height);
                    DirtyRect::new(left, top, right - left, bottom - top)
                }
            });
        }

        if count == 0 {
            return Ok(());
        }
        // When most pixels changed, one big upload is typically cheaper.
        if total_dirty_area >= total_area / 2 {
            return unsafe { bitmap.CopyFromMemory(None, pixels, pitch) };
        }
        if count > 8 {
            let bounds = union.expect("Nonempty dirty region count");
            let union_area = u64::from(bounds.width) * u64::from(bounds.height);
            if union_area >= total_area / 2 {
                return unsafe { bitmap.CopyFromMemory(None, pixels, pitch) };
            }
            return Self::upload_rect(bitmap, pixels, width, pitch, bounds);
        }

        for rect in dirty_regions.iter().filter_map(|r| r.clipped(width, height)) {
            Self::upload_rect(bitmap, pixels, width, pitch, rect)?;
        }
        Ok(())
    }

    /// Copies one tightly-packed BGRA rectangle out of the full framebuffer.
    /// `pitch` remains the *full framebuffer width*, not the rectangle width.
    #[inline]
    fn upload_rect(
        bitmap: &ID2D1Bitmap,
        pixels: *const c_void,
        full_width: u32,
        pitch: u32,
        rect: DirtyRect,
    ) -> Result<()> {
        let offset_pixels = (rect.y as usize)
            .checked_mul(full_width as usize)
            .and_then(|v| v.checked_add(rect.x as usize))
            .expect("Framebuffer source offset overflow");
        let offset_bytes = offset_pixels.checked_mul(4)
            .expect("Framebuffer byte offset overflow");
        let destination = D2D_RECT_U {
            left: rect.x,
            top: rect.y,
            right: rect.x + rect.width,
            bottom: rect.y + rect.height,
        };
        unsafe {
            bitmap.CopyFromMemory(
                Some(&destination as *const D2D_RECT_U),
                pixels.cast::<u8>().add(offset_bytes).cast::<c_void>(),
                pitch,
            )
        }
    }

    /// Draws the cached GPU bitmap. The full bitmap is drawn because HWND
    /// swapchain contents are not guaranteed to survive presentation; partial
    /// *CPU-to-GPU uploads* are still effective without retaining back buffers.
    fn present_once(
        &mut self,
        hwnd: HWND,
        pixels: *const c_void,
        dirty_regions: Option<&[DirtyRect]>,
    ) -> Result<bool> {
        self.create_device_resources(hwnd)?;
        if self.bitmap.is_none() || self.render_target.is_none() {
            return Ok(false);
        }
        let full_upload = !self.bitmap_initialized || dirty_regions.is_none();
        if !full_upload && !self.needs_present {
            let contains_visible_damage = dirty_regions.unwrap().iter().any(|region| {
                region.clipped(self.framebuffer_size.width, self.framebuffer_size.height).is_some()
            });
            if !contains_visible_damage {
                return Ok(false);
            }
        }

        // Empty/fully-clipped regions do not update the GPU when the bitmap
        // is already populated. A target resize still redraws cached content.
        if full_upload {
            let pitch = self.framebuffer_size.width.checked_mul(4)
                .expect("Framebuffer stride exceeds u32");
            unsafe { self.bitmap.as_ref().unwrap().CopyFromMemory(None, pixels, pitch)?; }
        } else {
            self.upload_damage(pixels, dirty_regions.unwrap())?;
        }
        self.bitmap_initialized = true;

        let target = self.render_target.as_ref().unwrap();
        let bitmap = self.bitmap.as_ref().unwrap();
        unsafe {
            target.BeginDraw();
            target.DrawBitmap(
                bitmap,
                Some(&self.destination as *const D2D_RECT_F),
                1.0,
                D2D1_BITMAP_INTERPOLATION_MODE_NEAREST_NEIGHBOR,
                None,
            );
            target.EndDraw(None, None)?;
        }
        self.needs_present = false;
        Ok(true)
    }

    /// Legacy path: always upload and present the full framebuffer.
    pub(super) fn present(&mut self, hwnd: HWND, framebuffer: &Framebuffer) -> bool {
        self.present_impl(hwnd, framebuffer, None)
    }

    /// Opt-in damage path: only upload marked source pixels. An empty dirty
    /// region slice is a no-op unless resources/target must be refreshed.
    pub(super) fn present_dirty(
        &mut self,
        hwnd: HWND,
        framebuffer: &Framebuffer,
        dirty_regions: &[DirtyRect],
    ) -> bool {
        self.present_impl(hwnd, framebuffer, Some(dirty_regions))
    }

    fn present_impl(
        &mut self,
        hwnd: HWND,
        framebuffer: &Framebuffer,
        dirty_regions: Option<&[DirtyRect]>,
    ) -> bool {
        if self.target_size.width == 0 || self.target_size.height == 0 ||
            framebuffer.width == 0 || framebuffer.height == 0
        {
            return false;
        }
        self.resize_framebuffer((framebuffer.width, framebuffer.height));
        let pixels = framebuffer.as_ptr().cast::<c_void>();

        // Device loss invalidates the previously uploaded GPU image. Retry
        // after discarding resources; the retry automatically uploads all pixels.
        match self.present_once(hwnd, pixels, dirty_regions) {
            Ok(presented) => presented,
            Err(_) => {
                self.discard_device_resources();
                self.present_once(hwnd, pixels, None)
                    .expect("Direct2D presentation failed after recreating the render target")
            }
        }
    }
}
