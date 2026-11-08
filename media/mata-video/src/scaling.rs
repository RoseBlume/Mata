//! Fallback when a backend cannot produce resized output natively.
use crate::{VideoError, VideoFrame};

/// Fast nearest-neighbour scale, avoiding floating-point operations per pixel.
pub(crate) fn scale_frame(mut frame: VideoFrame, width: u32, height: u32) -> Result<VideoFrame, VideoError> {
    if width == 0 || height == 0 { return Err(VideoError::new("output resolution cannot be zero")); }
    if frame.width == width && frame.height == height { return Ok(frame); }
    let out_len = (width as usize).checked_mul(height as usize)
        .ok_or_else(|| VideoError::new("output resolution overflows pixel count"))?;
    let input_len = (frame.width as usize).checked_mul(frame.height as usize)
        .ok_or_else(|| VideoError::new("source frame size overflow"))?;
    if frame.width == 0 || frame.height == 0 || frame.pixels.len() != input_len {
        return Err(VideoError::new("invalid source frame dimensions"));
    }
    let mut pixels = Vec::new();
    pixels.try_reserve_exact(out_len).map_err(|_| VideoError::new("cannot allocate resized frame"))?;
    for y in 0..height {
        let source_y = (u64::from(y) * u64::from(frame.height) / u64::from(height)) as usize;
        let row = source_y * frame.width as usize;
        for x in 0..width {
            let source_x = (u64::from(x) * u64::from(frame.width) / u64::from(width)) as usize;
            pixels.push(frame.pixels[row + source_x]);
        }
    }
    frame.pixels = pixels;
    frame.width = width;
    frame.height = height;
    Ok(frame)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;
    #[test]
    fn scaling_preserves_timestamps_and_pixel_layout() {
        let f = VideoFrame { width: 2, height: 2, pixels: vec![1, 2, 3, 4],
            timestamp: Duration::from_secs(2), duration: Some(Duration::from_millis(40)) };
        let f = scale_frame(f, 4, 4).unwrap();
        assert_eq!(f.pixels, vec![1,1,2,2, 1,1,2,2, 3,3,4,4, 3,3,4,4]);
        assert_eq!(f.timestamp, Duration::from_secs(2));
        assert_eq!(f.duration, Some(Duration::from_millis(40)));
    }
    #[test]
    fn rejects_bad_sizes() {
        let f = VideoFrame { width: 1, height: 1, pixels: vec![1],
            timestamp: Duration::ZERO, duration: None };
        assert!(scale_frame(f, 0, 1).is_err());
    }
}
