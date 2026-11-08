//! `mata-video` provides a small streaming video-decoding API.
//!
//! The crate has no cross-platform framework dependency of its own. Its only
//! direct platform dependencies are:
//!
//! - Windows: the `windows` crate, using Media Foundation.
//! - Linux: the `gstreamer` crate, using GStreamer.
//!
//! Frames are normalized to packed numeric `0xAARRGGBB` pixels so callers can
//! use one representation on both platforms.

#![forbid(unsafe_op_in_unsafe_fn)]

#[cfg(not(any(windows, target_os = "linux")))]
compile_error!("mata-video currently supports only Windows and Linux");

mod backend;
mod error;
mod types;
mod scaling;
mod player;
mod frame_cache;

use std::{path::Path, time::Duration};

pub use error::VideoError;
pub use player::{VideoPlayer, PlaybackOptions, BufferStats, PlaybackState, SeekCompletion};
pub use types::{
    SeekMode,
    PixelFormat,
    VideoFrame,
    VideoInfo,
};

use backend::PlatformDecoder;

/// Streaming video decoder backed by the native platform media stack.
pub struct VideoDecoder {
    backend: PlatformDecoder,
    requested_resolution: Option<(u32, u32)>,
}

impl VideoDecoder {
    /// Opens a video file and prepares its first video stream for decoding.
    ///
    /// The decoder does not load the whole file into memory. Frames are decoded
    /// as [`next_frame`](Self::next_frame) is called.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VideoError> {
        Ok(Self {
            backend: PlatformDecoder::open(path.as_ref())?,
            requested_resolution: None,
        })
    }

    /// Returns metadata describing the decoded output stream.
    #[inline]
    pub fn info(&self) -> VideoInfo {
        let mut info = self.backend.info();
        if let Some((width, height)) = self.requested_resolution {
            info.width = width; info.height = height;
        }
        info
    }

    /// Duration from the container, if the platform reports it.
    #[inline]
    pub fn duration(&self) -> Option<Duration> { self.backend.info().duration }

    /// Native seek to a timestamp. Does not decode preceding video frames.
    /// Depending on the format and backend, the next frame may be from a
    /// preceding keyframe. Use [`VideoPlayer::seek`] for a precise display seek.
    pub fn seek(&mut self, position: Duration) -> Result<(), VideoError> {
        self.seek_with_mode(position, SeekMode::Accurate)
    }

    /// Choose between precise and quick, keyframe-aligned native seeking.
    /// Both modes jump through the container's native index when available;
    /// neither starts decoding at the beginning of a seekable video.
    pub fn seek_with_mode(&mut self, position: Duration, mode: SeekMode) -> Result<(), VideoError> {
        self.backend.seek(position, mode)
    }

    /// Quick native seek for timeline scrubbing and thumbnail previews.
    /// May return a frame earlier than `position`.
    pub fn seek_fast(&mut self, position: Duration) -> Result<(), VideoError> {
        self.seek_with_mode(position, SeekMode::Keyframe)
    }

    // Skip decoder preroll before converting pixels or resizing frames.
    // The platform first seeks natively, so only the local keyframe interval
    // should have to be decoded on correctly indexed, seekable media.
    pub(crate) fn next_frame_at_or_after(&mut self, position: Duration)
        -> Result<(Option<VideoFrame>, u64), VideoError>
    {
        let (frame, skipped) = self.backend.next_frame_at_or_after(position)?;
        let frame = frame.map(|frame| {
            if let Some((width, height)) = self.requested_resolution {
                scaling::scale_frame(frame, width, height)
            } else {
                Ok(frame)
            }
        }).transpose()?;
        Ok((frame, skipped))
    }

    /// Native timestamp seek followed by the first frame at or after the
    /// requested time. Frames from the preceding keyframe are skipped before
    /// pixel conversion, avoiding any decode from the start of a seekable file.
    /// Returns `None` when no following frame exists.
    pub fn seek_frame(&mut self, position: Duration) -> Result<Option<VideoFrame>, VideoError> {
        self.seek(position)?;
        self.next_frame_at_or_after(position).map(|(frame, _skipped)| frame)
    }

    /// Fast preview seek: return the first frame at a nearby indexed seek
    /// point. May be before the target; avoids accurate-timestamp preroll.
    pub fn seek_preview_frame(&mut self, position: Duration) -> Result<Option<VideoFrame>, VideoError> {
        self.seek_fast(position)?;
        self.next_frame()
    }

    /// Change the output dimensions. The Linux backend scales in its GStreamer
    /// pipeline; Windows tries its native video processor, with a software
    /// fallback if the driver rejects the requested format.
    pub fn set_output_resolution(&mut self, resolution: Option<(u32, u32)>) -> Result<(), VideoError> {
        if let Some((w, h)) = resolution {
            if w == 0 || h == 0 || (w as usize).checked_mul(h as usize).and_then(|n| n.checked_mul(4)).is_none() {
                return Err(VideoError::new("invalid output resolution"));
            }
        }
        self.backend.set_output_resolution(resolution)?;
        self.requested_resolution = resolution;
        Ok(())
    }

    /// Decode exactly one subsequent frame; playback timing is the player's job.
    #[inline]
    pub fn step_frame(&mut self) -> Result<Option<VideoFrame>, VideoError> { self.next_frame() }

    /// Decodes the next frame.
    ///
    /// Returns `Ok(None)` after end-of-stream has been reached.
    #[inline]
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, VideoError> {
        self.backend.next_frame()?.map(|frame| {
            if let Some((width, height)) = self.requested_resolution {
                scaling::scale_frame(frame, width, height)
            } else { Ok(frame) }
        }).transpose()
    }
}

impl Iterator for VideoDecoder {
    type Item = Result<VideoFrame, VideoError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_frame() {
            Ok(Some(frame)) => Some(Ok(frame)),
            Ok(None) => None,
            Err(error) => Some(Err(error)),
        }
    }
}
