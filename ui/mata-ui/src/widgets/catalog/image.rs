//! Image and picture widgets backed by `mata_image`.

use std::sync::{ Arc, Mutex };
#[cfg(feature = "image")]
use std::path::Path;

#[cfg(feature = "image")]
use mata_image::Image as DecodedImage;

use crate::Framebuffer;
use crate::widgets::{ Rect, WidgetError };
use super::{ base::{ Widget, WidgetExt }, types::ContentFit };

/// Mata image/picture backing store decoded by `mata_image`.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Stores width.
    ///
    /// Visibility: **public**.
    pub width: u32,
    /// Stores height.
    ///
    /// Visibility: **public**.
    pub height: u32,
    /// MataWin-native `0xAARRGGBB` pixels.
    pub pixels: Arc<Vec<u32>>,
}

impl ImageData {
    /// Construct a display image from already-decoded ARGB32 pixels without
    /// depending on `mata_image` (also useful for video frames).
    pub fn from_argb32(width: u32, height: u32, pixels: Vec<u32>) -> Result<Self, WidgetError> {
        let count = (width as usize).checked_mul(height as usize)
            .ok_or_else(|| WidgetError("image dimensions overflow".into()))?;
        if width == 0 || height == 0 || pixels.len() != count {
            return Err(WidgetError("invalid image dimensions or pixel count".into()));
        }
        Ok(Self { width, height, pixels: Arc::new(pixels) })
    }

    /// Performs the load operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the load operation. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn load(path: impl AsRef<Path>) -> Result<Self, WidgetError> {
        let path = path.as_ref();
        let mut file = std::fs::File::open(path).map_err(|e|
            WidgetError(format!("failed to open image {}: {e}", path.display()))
        )?;
        let decoded = DecodedImage::from_reader(&mut file).map_err(|e|
            WidgetError(format!("mata_image failed to decode {}: {e}", path.display()))
        )?;
        Self::from_decoded(decoded)
    }
    /// Performs the from bytes operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the from bytes operation. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, WidgetError> {
        let decoded = DecodedImage::from_bytes(bytes).map_err(|e|
            WidgetError(format!("mata_image failed to decode image bytes: {e}"))
        )?;
        Self::from_decoded(decoded)
    }
    /// Performs the from decoded operation.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the from decoded operation. Visibility: private/internal."]
    #[cfg(feature = "image")]
    fn from_decoded(decoded: DecodedImage) -> Result<Self, WidgetError> {
        // `mata_image::Image` keeps the primary display frame directly in
        // `Image::pixels`; `Image::frames` contains only *additional* animation
        // frames. Consume that primary buffer and convert it in place so MataWin
        // does not allocate a second full-size pixel vector.
        let width = decoded.width;
        let height = decoded.height;
        let mut pixels = decoded.into_pixels();
        for pixel in &mut pixels {
            *pixel = rgba_to_matawin(*pixel);
        }
        Ok(Self { width, height, pixels: Arc::new(pixels) })
    }
}

#[inline(always)]
/// Implements rgba to mata_win.
///
/// Visibility: **private**.
#[doc = "Implements the rgba to mata_win operation. Visibility: private/internal."]
#[cfg(feature = "image")]
fn rgba_to_matawin(pixel: u32) -> u32 {
    (pixel >> 8) | (pixel << 24)
}

#[derive(Debug, Clone)]
/// Stores the state and data for picture.
///
/// Visibility: **public**.
pub struct Picture {
    /// Stores widget.
    ///
    /// Visibility: **private**.
    widget: Widget,
    /// Stores image.
    ///
    /// Visibility: **private**.
    image: Arc<Mutex<Option<ImageData>>>,
    /// Reuse the last scaled image across dirty-driven frames and scrolling.
    scaled: Arc<Mutex<Option<ScaledImage>>>,
    /// Stores content fit.
    ///
    /// Visibility: **private**.
    content_fit: Arc<Mutex<ContentFit>>,
    /// Stores keep aspect ratio.
    ///
    /// Visibility: **private**.
    keep_aspect_ratio: Arc<Mutex<bool>>,
    /// Stores alternative text.
    ///
    /// Visibility: **private**.
    alternative_text: Arc<Mutex<Option<String>>>,
}
impl Default for Picture {
    #[doc = "Creates the default value. Visibility: private/internal."]
    fn default() -> Self {
        Self::new()
    }
}
impl Picture {
    /// Creates a new instance.
    ///
    /// Visibility: **public**.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new() -> Self {
        Self {
            widget: Widget::new(),
            image: Arc::new(Mutex::new(None)),
            scaled: Arc::new(Mutex::new(None)),
            content_fit: Arc::new(Mutex::new(ContentFit::ScaleDown)),
            keep_aspect_ratio: Arc::new(Mutex::new(true)),
            alternative_text: Arc::new(Mutex::new(None)),
        }
    }
    /// Performs the from file operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the from file operation. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WidgetError> {
        let this = Self::new();
        this.set_file(path)?;
        Ok(this)
    }
    /// Sets file.
    ///
    /// Visibility: **public**.
    #[doc = "Sets file. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn set_file(&self, path: impl AsRef<Path>) -> Result<(), WidgetError> {
        *self.image.lock().unwrap() = Some(ImageData::load(path)?);
        self.scaled.lock().unwrap().take();
        self.widget.invalidate();
        Ok(())
    }
    /// Sets bytes.
    ///
    /// Visibility: **public**.
    #[doc = "Sets bytes. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn set_bytes(&self, bytes: &[u8]) -> Result<(), WidgetError> {
        *self.image.lock().unwrap() = Some(ImageData::from_bytes(bytes)?);
        self.scaled.lock().unwrap().take();
        self.widget.invalidate();
        Ok(())
    }
    /// Supply pixels without requiring the optional image decoder.
    pub fn set_image(&self, image: ImageData) {
        *self.image.lock().unwrap() = Some(image);
        self.scaled.lock().unwrap().take();
        self.widget.invalidate();
    }

    /// Returns the current image data.
    pub fn image(&self) -> Option<ImageData> {
        self.image.lock().unwrap().clone()
    }
    /// Sets content fit.
    ///
    /// Visibility: **public**.
    #[doc = "Sets content fit. Visibility: public."]
    pub fn set_content_fit(&self, fit: ContentFit) {
        *self.content_fit.lock().unwrap() = fit;
        self.widget.invalidate();
    }
    /// Sets keep aspect ratio.
    ///
    /// Visibility: **public**.
    #[doc = "Sets keep aspect ratio. Visibility: public."]
    pub fn set_keep_aspect_ratio(&self, value: bool) {
        *self.keep_aspect_ratio.lock().unwrap() = value;
        self.widget.invalidate();
    }
    /// Sets alternative text.
    ///
    /// Visibility: **public**.
    #[doc = "Sets alternative text. Visibility: public."]
    pub fn set_alternative_text<T: Into<String>>(&self, text: Option<T>) {
        *self.alternative_text.lock().unwrap() = text.map(Into::into);
    }

    /// Renders the primary decoded frame into a MataWin framebuffer using nearest
    /// neighbour scaling and alpha blending.
    #[doc = "Implements the blit operation. Visibility: public."]
    pub(crate) fn blit_within(&self, target: &mut Framebuffer, bounds: Rect, parent_opacity: f32) {
        if !self.widget.visible() { return; }
        let Some(image) = self.image() else { return; };
        if bounds.width == 0 || bounds.height == 0 { return; }
        let computed = self.widget.style_handle().computed(
            bounds, (target.width, target.height), Some(bounds)
        );
        blit_scaled_cached(
            &image, target, computed.content_bounds,
            *self.keep_aspect_ratio.lock().unwrap(),
            computed.opacity * parent_opacity, computed.border_radius, &self.scaled
        );
    }

    pub fn blit(&self, target: &mut Framebuffer) {
        let Some(image) = self.image() else {
            return;
        };
        let computed = self.widget.computed_style((target.width, target.height), None);
        let bounds = computed.content_bounds;
        if bounds.width == 0 || bounds.height == 0 || image.width == 0 || image.height == 0 {
            return;
        }
        blit_scaled_cached(
            &image,
            target,
            bounds,
            *self.keep_aspect_ratio.lock().unwrap(),
            computed.opacity,
            computed.border_radius, &self.scaled
        );
    }
}
impl WidgetExt for Picture {
    #[doc = "Implements the widget operation. Visibility: private/internal."]
    fn widget(&self) -> &Widget { &self.widget }
    fn renderable_content(&self) -> Option<crate::widgets::WidgetContent> {
        Some(crate::widgets::WidgetContent::Picture(self.clone()))
    }
}

#[derive(Debug, Clone, Default)]
/// Stores the state and data for image.
///
/// Visibility: **public**.
pub struct Image {
    /// Layout size when used as a child of another widget.
    preferred_size: Arc<Mutex<Option<(u32, u32)>>>,
    /// Stores picture.
    ///
    /// Visibility: **private**.
    picture: Picture,
}
impl Image {
    /// Creates a new instance.
    ///
    /// Visibility: **public**.
    #[doc = "Creates a new value. Visibility: public."]
    pub fn new() -> Self {
        Self { picture: Picture::new(), preferred_size: Arc::new(Mutex::new(None)) }
    }
    /// Performs the from file operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the from file operation. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WidgetError> {
        Ok(Self { picture: Picture::from_file(path)?, preferred_size: Arc::new(Mutex::new(None)) })
    }
    /// Sets file.
    ///
    /// Visibility: **public**.
    #[doc = "Sets file. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn set_file(&self, path: impl AsRef<Path>) -> Result<(), WidgetError> {
        self.picture.set_file(path)
    }
    /// Sets bytes.
    ///
    /// Visibility: **public**.
    #[doc = "Sets bytes. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn set_bytes(&self, bytes: &[u8]) -> Result<(), WidgetError> {
        self.picture.set_bytes(bytes)
    }
    /// Set the size of this image when it is nested in a button or layout.
    /// Pixels are drawn centered inside the available content bounds.
    pub fn set_size(&self, width: u32, height: u32) {
        *self.preferred_size.lock().unwrap() = Some((width.max(1),height.max(1)));
        self.picture.widget().invalidate();
    }

    /// The optional requested content size, distinct from source image dimensions.
    pub fn preferred_size(&self) -> Option<(u32,u32)> {
        *self.preferred_size.lock().unwrap()
    }

    /// Performs the blit operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the blit operation. Visibility: public."]
    pub fn set_image(&self, image: ImageData) {
        self.picture.set_image(image);
    }
    pub fn blit(&self, target: &mut Framebuffer) {
        self.picture.blit(target);
    }

    pub(crate) fn blit_within(&self, target: &mut Framebuffer, bounds: Rect, opacity: f32) {
        self.picture.blit_within(target, bounds, opacity);
    }
    /// Returns the intrinsic image size, if pixels have been loaded.
    pub fn dimensions(&self) -> Option<(u32,u32)> {
        self.picture.image().map(|data|(data.width,data.height))
    }
    pub(crate) fn image_data(&self) -> Option<ImageData> {
        self.picture.image()
    }
}
impl WidgetExt for Image {
    #[doc = "Implements the widget operation. Visibility: private/internal."]
    fn widget(&self) -> &Widget { self.picture.widget() }
    fn renderable_content(&self) -> Option<crate::widgets::WidgetContent> {
        Some(crate::widgets::WidgetContent::Image(self.clone()))
    }
}

#[derive(Debug, Clone, Default)]
/// Stores the state and data for svg widget.
///
/// Visibility: **public**.
pub struct SvgWidget {
    /// Stores image.
    ///
    /// Visibility: **private**.
    image: Image,
}
impl SvgWidget {
    /// Performs the from file operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the from file operation. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WidgetError> {
        Ok(Self { image: Image::from_file(path)? })
    }
    /// Sets file.
    ///
    /// Visibility: **public**.
    #[doc = "Sets file. Visibility: public."]
    #[cfg(feature = "image")]
    pub fn set_file(&self, path: impl AsRef<Path>) -> Result<(), WidgetError> {
        self.image.set_file(path)
    }
    /// Performs the blit operation.
    ///
    /// Visibility: **public**.
    #[doc = "Implements the blit operation. Visibility: public."]
    pub fn blit(&self, target: &mut Framebuffer) {
        self.image.blit(target);
    }
}
impl WidgetExt for SvgWidget {
    fn renderable_content(&self) -> Option<crate::widgets::WidgetContent> {
        Some(crate::widgets::WidgetContent::Image(self.image.clone()))
    }

    #[doc = "Implements the widget operation. Visibility: private/internal."]
    fn widget(&self) -> &Widget {
        self.image.widget()
    }
}

/// One cached, premultiplied-alpha-aware resampling result. Keep the source
/// alive so another image replacing this picture cannot reuse an old bitmap.
#[derive(Debug, Clone)]
struct ScaledImage {
    source: Arc<Vec<u32>>,
    width: u32,
    height: u32,
    pixels: Arc<Vec<u32>>,
}

/// Draw static HTML/catalog pictures with high-quality resampling.
/// The resampled pixels are cached by source and size: scrolling only blits.
/// For very large outputs, avoid retaining an extra large image buffer.
/// Keep video-frame blitting on the lightweight nearest-neighbour path.
/// Video frames change every frame and cannot benefit from a bitmap cache.
pub(super) fn blit_scaled(
    image: &ImageData,
    target: &mut Framebuffer,
    bounds: Rect,
    keep_aspect: bool,
    opacity: f32,
    radius: crate::widgets::style::ResolvedBorderRadius
) {
    let mut dw = bounds.width;
    let mut dh = bounds.height;
    let mut ox = bounds.x;
    let mut oy = bounds.y;
    if keep_aspect {
        let sx = (bounds.width as f64) / (image.width as f64);
        let sy = (bounds.height as f64) / (image.height as f64);
        let scale = sx.min(sy);
        dw = ((image.width as f64) * scale).round().max(1.0) as u32;
        dh = ((image.height as f64) * scale).round().max(1.0) as u32;
        ox += ((bounds.width - dw) / 2) as i32;
        oy += ((bounds.height - dh) / 2) as i32;
    }
    for dy in 0..dh {
        let ty = oy + (dy as i32);
        if ty < 0 || ty >= (target.height as i32) {
            continue;
        }
        let sy = (((dy as u64) * (image.height as u64)) / (dh as u64)) as u32;
        for dx in 0..dw {
            let tx = ox + (dx as i32);
            if tx < 0 || tx >= (target.width as i32) {
                continue;
            }
            let sx = (((dx as u64) * (image.width as u64)) / (dw as u64)) as u32;
            if !inside_rounded(bounds, radius, tx, ty) {
                continue;
            }
            let mut src = image.pixels[(sy * image.width + sx) as usize];
            let a = ((((src >> 24) & 0xff) as f32) * opacity.clamp(0.0, 1.0))
                .round()
                .clamp(0.0, 255.0) as u32;
            src = (src & 0x00ff_ffff) | (a << 24);
            let dst_i = ((ty as u32) * target.width + (tx as u32)) as usize;
            target.pixels[dst_i] = alpha_over(src, target.pixels[dst_i]);
        }
    }
}


fn blit_scaled_cached(
    image: &ImageData, target: &mut Framebuffer, bounds: Rect,
    keep_aspect: bool, opacity: f32,
    radius: crate::widgets::style::ResolvedBorderRadius,
    cache: &Mutex<Option<ScaledImage>>,
) {
    if image.width == 0 || image.height == 0 || bounds.width == 0 || bounds.height == 0 { return; }
    let (mut dw, mut dh) = (bounds.width, bounds.height);
    let (mut ox, mut oy) = (bounds.x, bounds.y);
    if keep_aspect {
        let scale = (bounds.width as f64 / image.width as f64)
            .min(bounds.height as f64 / image.height as f64);
        dw = (image.width as f64 * scale).round().max(1.0) as u32;
        dh = (image.height as f64 * scale).round().max(1.0) as u32;
        ox += (bounds.width.saturating_sub(dw) / 2) as i32;
        oy += (bounds.height.saturating_sub(dh) / 2) as i32;
    }
    if dw == 0 || dh == 0 { return; }
    const MAX_CACHED_PIXELS: u64 = 8_000_000;
    let scaled = if dw == image.width && dh == image.height {
        image.pixels.clone()
    } else if (dw as u64) * (dh as u64) <= MAX_CACHED_PIXELS {
        let mut stored = cache.lock().unwrap();
        if stored.as_ref().is_none_or(|cached|
            cached.width != dw || cached.height != dh
                || !Arc::ptr_eq(&cached.source, &image.pixels)) {
            *stored = Some(ScaledImage { source: image.pixels.clone(), width: dw,
                height: dh, pixels: Arc::new(resample_argb(image,dw,dh)) });
        }
        stored.as_ref().unwrap().pixels.clone()
    } else {
        Arc::new(resample_argb(image,dw,dh))
    };
    let (begin_x, begin_y) = (ox.max(0),oy.max(0));
    let end_x = ox.saturating_add(dw as i32).min(target.width as i32);
    let end_y = oy.saturating_add(dh as i32).min(target.height as i32);
    for ty in begin_y..end_y {
        for tx in begin_x..end_x {
            if !inside_rounded(bounds, radius, tx, ty) {continue;}
            let idx = ((ty - oy) as u32 * dw + (tx - ox) as u32) as usize;
            let mut src = scaled[idx];
            let alpha = (((src >> 24) as f32) * opacity.clamp(0.0,1.0)).round() as u32;
            src = (src & 0x00ff_ffff) | (alpha.min(255) << 24);
            let dst = ty as usize * target.width as usize + tx as usize;
            target.pixels[dst] = alpha_over(src,target.pixels[dst]);
        }
    }
}

/// Filter 0xAARRGGBB pixels using pixel-centred bilinear interpolation for
/// enlargement and exact box coverage for reduction. Both accumulate channels
/// premultiplied by opacity to avoid dark fringes around transparent images.
fn resample_argb(image:&ImageData, width:u32, height:u32)->Vec<u32>{
    if width==0 || height==0 {return Vec::new();}
    let mut output=vec![0; (width as usize)*(height as usize)];
    let sx=image.width as f64/width as f64;
    let sy=image.height as f64/height as f64;
    for y in 0..height {
        for x in 0..width {
            let mut acc=[0.0f64;4];
            let mut total=0.0f64;
            if sx>1.0 || sy>1.0 {
                // Integral area filter. Axis-aligned pixel overlap is exact,
                // so shrinking a checkerboard does not generate moire.
                let left=x as f64*sx;
                let top=y as f64*sy;
                let right=(x+1) as f64*sx;
                let bottom=(y+1) as f64*sy;
                for iy in (top.floor() as u32)..(bottom.ceil() as u32).min(image.height) {
                    let wy=((iy+1) as f64).min(bottom)-(iy as f64).max(top);
                    if wy<=0.0 {continue;}
                    for ix in (left.floor() as u32)..(right.ceil() as u32).min(image.width) {
                        let wx=((ix+1) as f64).min(right)-(ix as f64).max(left);
                        if wx<=0.0 {continue;}
                        let weight=wx*wy;
                        add_sample(&mut acc,image.pixels[(iy*image.width+ix) as usize],weight);
                        total+=weight;
                    }
                }
            } else {
                let fx=((x as f64+0.5)*sx-0.5).clamp(0.0,(image.width-1) as f64);
                let fy=((y as f64+0.5)*sy-0.5).clamp(0.0,(image.height-1) as f64);
                let (x0,y0)=(fx.floor() as u32,fy.floor() as u32);
                let (x1,y1)=((x0+1).min(image.width-1),(y0+1).min(image.height-1));
                let (tx,ty)=(fx-x0 as f64,fy-y0 as f64);
                for (ix,iy,weight) in [
                    (x0,y0,(1.0-tx)*(1.0-ty)),(x1,y0,tx*(1.0-ty)),
                    (x0,y1,(1.0-tx)*ty),(x1,y1,tx*ty),
                ] {
                    add_sample(&mut acc,image.pixels[(iy*image.width+ix) as usize],weight);
                    total+=weight;
                }
            }
            output[(y*width+x) as usize]=finish_sample(acc,total);
        }
    }
    output
}
#[inline]
fn add_sample(acc:&mut [f64;4],pixel:u32,weight:f64){
    let a=((pixel>>24)&255) as f64/255.0;
    acc[0]+=a*weight;
    acc[1]+=((pixel>>16)&255) as f64*a*weight;
    acc[2]+=((pixel>>8)&255) as f64*a*weight;
    acc[3]+=(pixel&255) as f64*a*weight;
}
#[inline]
fn finish_sample(v:[f64;4],total:f64)->u32{
    if total<=0.0 {return 0;}
    let alpha=(v[0]/total).clamp(0.0,1.0);
    if alpha<=0.0 {return 0;}
    let chan=|component:f64| (component/v[0]).round().clamp(0.0,255.0) as u32;
    ((alpha*255.0).round() as u32)<<24 | chan(v[1])<<16 | chan(v[2])<<8 | chan(v[3])
}

/// Implements inside rounded.
///
/// Visibility: **private**.
#[doc = "Implements the inside rounded operation. Visibility: private/internal."]
fn inside_rounded(
    r: Rect,
    rad: crate::widgets::style::ResolvedBorderRadius,
    x: i32,
    y: i32
) -> bool {
    let fx = (x as f32) + 0.5;
    let fy = (y as f32) + 0.5;
    let left = r.x as f32;
    let top = r.y as f32;
    let right = left + (r.width as f32);
    let bottom = top + (r.height as f32);
    /// Implements inside.
    ///
    /// Visibility: **private**.
    #[doc = "Implements the inside operation. Visibility: private/internal."]
    fn inside(cx: f32, cy: f32, rx: f32, ry: f32, x: f32, y: f32) -> bool {
        if rx <= 0.0 || ry <= 0.0 {
            return true;
        }
        let dx = (x - cx) / rx;
        let dy = (y - cy) / ry;
        dx * dx + dy * dy <= 1.0
    }
    if fx < left + rad.top_left.x && fy < top + rad.top_left.y {
        return inside(
            left + rad.top_left.x,
            top + rad.top_left.y,
            rad.top_left.x,
            rad.top_left.y,
            fx,
            fy
        );
    }
    if fx > right - rad.top_right.x && fy < top + rad.top_right.y {
        return inside(
            right - rad.top_right.x,
            top + rad.top_right.y,
            rad.top_right.x,
            rad.top_right.y,
            fx,
            fy
        );
    }
    if fx > right - rad.bottom_right.x && fy > bottom - rad.bottom_right.y {
        return inside(
            right - rad.bottom_right.x,
            bottom - rad.bottom_right.y,
            rad.bottom_right.x,
            rad.bottom_right.y,
            fx,
            fy
        );
    }
    if fx < left + rad.bottom_left.x && fy > bottom - rad.bottom_left.y {
        return inside(
            left + rad.bottom_left.x,
            bottom - rad.bottom_left.y,
            rad.bottom_left.x,
            rad.bottom_left.y,
            fx,
            fy
        );
    }
    true
}

#[inline]
/// Implements alpha over.
///
/// Visibility: **private**.
#[doc = "Implements the alpha over operation. Visibility: private/internal."]
fn alpha_over(src: u32, dst: u32) -> u32 {
    let a = (src >> 24) & 0xff;
    if a == 255 {
        return src;
    }
    if a == 0 {
        return dst;
    }
    let inv = 255 - a;
    let sr = (src >> 16) & 0xff;
    let sg = (src >> 8) & 0xff;
    let sb = src & 0xff;
    let dr = (dst >> 16) & 0xff;
    let dg = (dst >> 8) & 0xff;
    let db = dst & 0xff;
    let r = (sr * a + dr * inv + 127) / 255;
    let g = (sg * a + dg * inv + 127) / 255;
    let b = (sb * a + db * inv + 127) / 255;
    0xff00_0000 | (r << 16) | (g << 8) | b
}


#[cfg(all(test, feature = "image"))]
mod tests {
    use super::{ImageData, rgba_to_matawin};
    use mata_image::{Image as DecodedImage, ImageFrame};

    #[test]
    fn converts_new_mataimage_primary_frame_in_place() {
        let decoded = DecodedImage {
            width: 2,
            height: 1,
            pixels: vec![0x1122_3344, 0xaabb_ccdd],
            // New mata_image semantics: these are additional animation frames,
            // not the primary frame MataWin should display by default.
            frames: vec![ImageFrame { pixels: vec![0x0102_0304, 0x0506_0708] }],
        };

        let image = ImageData::from_decoded(decoded).expect("conversion should succeed");
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.pixels.as_slice(), &[0x4411_2233, 0xddaa_bbcc]);
    }

    #[test]
    fn rgba_to_matawin_rotates_rgba_to_argb() {
        assert_eq!(rgba_to_matawin(0x1122_3344), 0x4411_2233);
        assert_eq!(rgba_to_matawin(0x0000_00ff), 0xff00_0000);
    }
}


#[cfg(test)]
mod native_image_tests {
    use super::*;

    #[test]
    fn native_argb_pixels_are_available_without_decoder() {
        let image = ImageData::from_argb32(2, 1, vec![0xff11_2233, 0xff44_5566]).unwrap();
        let picture = Picture::new();
        picture.set_image(image);
        assert_eq!(&picture.image().unwrap().pixels[..], &[0xff11_2233, 0xff44_5566]);
    }

    #[test]
    fn native_argb_rejects_bad_dimensions() {
        assert!(ImageData::from_argb32(0, 1, vec![]).is_err());
        assert!(ImageData::from_argb32(u32::MAX, u32::MAX, vec![]).is_err());
        assert!(ImageData::from_argb32(2, 2, vec![0xffaa_bbcc]).is_err());
    }
}

#[cfg(test)]
mod high_quality_resampling_tests {
    use super::*;
    #[test]
    fn downscale_averages_checkerboard() {
        let image=ImageData::from_argb32(2,2,vec![0xff000000,0xffffffff,0xffffffff,0xff000000]).unwrap();
        let pixel=resample_argb(&image,1,1)[0];
        assert_eq!(pixel>>24,255);
        assert!((pixel as u8 as i32-128).abs()<=1);
    }
    #[test]
    fn transparent_edges_do_not_bleed_black() {
        let image=ImageData::from_argb32(2,1,vec![0x00_000000,0xff_ff0000]).unwrap();
        let middle=resample_argb(&image,3,1)[1];
        assert_eq!((middle>>16)&255,255);
        assert!((middle>>24)&255>0);
    }
    #[test]
    fn identity_preserves_source() {
        let image=ImageData::from_argb32(2,1,vec![0x00ffffff,0xffaabbcc]).unwrap();
        assert_eq!(resample_argb(&image,2,1),vec![0x00_000000,0xff_aabbcc]);
    }
}
