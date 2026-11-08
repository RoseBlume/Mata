//! Opt-in video widget built on `mata-video`'s native Windows/Linux decoder.
//!
//! Frames are decoded only as needed and retained as ARGB32 without colour
//! conversion or a second pixel copy. The widget host advances playback when
//! `WidgetWindow::present` is called. Direct users may call [`Video::update`]
//! and [`Video::blit`] with their own render loop.

use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex, Weak, atomic::{AtomicBool, Ordering}},
    time::{Duration, Instant},
};
use mata_video::{VideoDecoder, VideoFrame, VideoInfo};
use crate::{Framebuffer, widgets::WidgetError};
#[cfg(feature = "audio")]
use crate::widgets::media::AudioSource;
#[cfg(feature = "audio")]
use super::media_widgets::MediaControls;
use super::{
    base::{Widget, WidgetExt},
    image::{ImageData, blit_scaled},
};

fn video_error(error: impl std::fmt::Display) -> WidgetError {
    WidgetError(format!("mata-video: {error}"))
}

/// Playback state shared by clones of a video widget.
struct VideoState {
    path: Option<PathBuf>,
    decoder: Option<VideoDecoder>,
    info: Option<VideoInfo>,
    image: Option<ImageData>,
    last_frame_end: Duration,
    pending: Option<VideoFrame>,
    position: Duration,
    clock: Option<Instant>,
    playing: bool,
    autoplay: bool,
    looping: bool,
    ended: bool,
}

impl Default for VideoState {
    fn default() -> Self {
        Self {
            path: None, decoder: None, info: None, image: None,
            last_frame_end: Duration::ZERO,
            pending: None, position: Duration::ZERO, clock: None,
            playing: false, autoplay: false, looping: false, ended: false,
        }
    }
}

impl VideoState {
    fn current_position(&self) -> Duration {
        match self.clock {
            Some(start) => self.position.saturating_add(start.elapsed()),
            None => self.position,
        }
    }

    fn restart_clock(&mut self) {
        self.clock = self.playing.then(Instant::now);
    }

    /// Open once, prepare the first frame, and retain its decoded pixel vector.
    fn open_file(&mut self, path: &Path) -> Result<(), WidgetError> {
        let mut decoder = VideoDecoder::open(path).map_err(video_error)?;
        let info = decoder.info();
        let first = decoder.next_frame().map_err(video_error)?
            .ok_or_else(|| WidgetError(format!("no video frames in {}", path.display())))?;
        let last_frame_end = frame_end(&first, info);
        let image = frame_image(first)?;
        self.path = Some(path.to_path_buf());
        self.info = Some(info);
        self.decoder = Some(decoder);
        self.image = Some(image);
        self.last_frame_end = last_frame_end;
        self.pending = None;
        self.position = Duration::ZERO;
        self.playing = self.autoplay;
        self.ended = false;
        self.restart_clock();
        Ok(())
    }

    fn advance(&mut self) -> Result<bool, WidgetError> {
        if !self.playing || self.decoder.is_none() || self.ended {
            return Ok(false);
        }
        // Avoid decoding an unbounded number of frames in a UI present call.
        const MAX_FRAMES_PER_TICK: usize = 4;
        let mut updated = false;
        for _ in 0..MAX_FRAMES_PER_TICK {
            if self.pending.is_none() {
                self.pending = self.decoder.as_mut().unwrap().next_frame().map_err(video_error)?;
                if self.pending.is_none() {
                    // Keep the last decoded frame visible for its full duration.
                    let end = self.info.and_then(|info| info.duration)
                        .unwrap_or(self.last_frame_end)
                        .max(self.last_frame_end);
                    if self.current_position() < end { return Ok(updated); }
                    if self.looping {
                        let path = self.path.clone().ok_or_else(|| WidgetError("video path missing".into()))?;
                        self.open_file(&path)?;
                        // Explicit `play` must continue even if autoplay was false.
                        self.playing = true;
                        self.restart_clock();
                        return Ok(true);
                    }
                    self.position = self.info.and_then(|info| info.duration)
                        .unwrap_or_else(|| self.current_position());
                    self.playing = false;
                    self.clock = None;
                    self.ended = true;
                    return Ok(updated);
                }
            }
            if self.pending.as_ref().is_some_and(|frame| frame.timestamp > self.current_position()) {
                break;
            }
            // The backend already returns packed numeric ARGB32 pixels.
            let next = self.pending.take().unwrap();
            self.last_frame_end = frame_end(&next, self.info.unwrap());
            self.image = Some(frame_image(next)?);
            updated = true;
        }
        Ok(updated)
    }

    /// `mata-video` currently has no native seek operation; decode forward
    /// from a reopened stream with bounded memory until reaching the target.
    fn seek_to(&mut self, target: Duration) -> Result<(), WidgetError> {
        let path = self.path.as_deref().ok_or_else(|| WidgetError("no video file selected".into()))?;
        let mut decoder = VideoDecoder::open(path).map_err(video_error)?;
        let info = decoder.info();
        let target = info.duration.map_or(target, |duration| target.min(duration));
        let mut previous: Option<VideoFrame> = None;
        let mut pending = None;
        while let Some(frame) = decoder.next_frame().map_err(video_error)? {
            if frame.timestamp > target {
                pending = Some(frame);
                break;
            }
            previous = Some(frame);
        }
        let image = previous.or_else(|| pending.take())
            .ok_or_else(|| WidgetError("video has no decodable frames".into()))?;
        self.last_frame_end = frame_end(&image, info);
        self.image = Some(frame_image(image)?);
        self.pending = pending;
        self.info = Some(info);
        self.decoder = Some(decoder);
        self.position = target;
        self.ended = false;
        self.restart_clock();
        Ok(())
    }
}

fn frame_end(frame: &VideoFrame, info: VideoInfo) -> Duration {
    let frame_length = frame.duration
        .or_else(|| info.frames_per_second().filter(|fps| fps.is_finite() && *fps > 0.0)
            .map(|fps| Duration::from_secs_f64(1.0 / fps)))
        .unwrap_or(Duration::from_millis(33));
    frame.timestamp.saturating_add(frame_length)
}

fn frame_image(frame: VideoFrame) -> Result<ImageData, WidgetError> {
    let count = (frame.width as usize).checked_mul(frame.height as usize)
        .ok_or_else(|| WidgetError("video dimensions overflow".into()))?;
    if frame.width == 0 || frame.height == 0 || frame.pixels.len() != count {
        return Err(WidgetError("invalid decoded video frame dimensions".into()));
    }
    Ok(ImageData { width: frame.width, height: frame.height, pixels: Arc::new(frame.pixels) })
}

/// Video widget using the native `mata-video` decoder (feature `video`).
///
/// Clones share playback, current picture, and styling. Frames are timestamped
/// by the decoder; video contains no automatic audio stream playback. Optionally
/// attach a separately decoded audio file using [`set_audio_file`](Self::set_audio_file).
#[derive(Clone)]
pub struct Video {
    widget: Widget,
    state: Arc<Mutex<VideoState>>,
    #[cfg(feature = "audio")]
    audio: Arc<Mutex<Option<AudioSource>>>,
    #[cfg(feature = "audio")]
    controls: Arc<Mutex<Option<MediaControls>>>,
    keep_aspect_ratio: Arc<Mutex<bool>>,
    redraws: Arc<Mutex<Vec<Weak<AtomicBool>>>>,
}

impl std::fmt::Debug for Video {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Video").field("widget", &self.widget).finish_non_exhaustive()
    }
}

impl Default for Video {
    fn default() -> Self { Self::new() }
}

impl Video {
    /// Creates an empty video widget.
    pub fn new() -> Self {
        Self {
            widget: Widget::new(), state: Arc::new(Mutex::new(VideoState::default())),
            #[cfg(feature = "audio")]
            audio: Arc::new(Mutex::new(None)),
            #[cfg(feature = "audio")]
            controls: Arc::new(Mutex::new(None)),
            keep_aspect_ratio: Arc::new(Mutex::new(true)),
            redraws: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Registers a host's invalidation flag without retaining the host itself.
    pub(crate) fn register_redraw(&self, dirty: &Arc<AtomicBool>) {
        let mut redraws = self.redraws.lock().unwrap();
        redraws.retain(|weak| weak.strong_count() > 0);
        if !redraws.iter().any(|weak| weak.ptr_eq(&Arc::downgrade(dirty))) {
            redraws.push(Arc::downgrade(dirty));
        }
    }

    fn mark_dirty(&self) {
        let mut redraws = self.redraws.lock().unwrap();
        redraws.retain(|weak| {
            if let Some(flag) = weak.upgrade() {
                flag.store(true, Ordering::Release);
                true
            } else {
                false
            }
        });
    }

    /// Opens a video with explicit autoplay behavior.
    pub fn from_file(path: impl AsRef<Path>, autoplay: bool) -> Result<Self, WidgetError> {
        let video = Self::new();
        video.set_autoplay(autoplay);
        video.set_file(path)?;
        Ok(video)
    }

    /// Sets the video file, validates it, and decodes the first frame.
    pub fn set_file(&self, path: impl AsRef<Path>) -> Result<(), WidgetError> {
        self.state.lock().unwrap().open_file(path.as_ref())?;
        self.mark_dirty();
        Ok(())
    }

    /// Returns the active file path, if any.
    pub fn file(&self) -> Option<PathBuf> {
        self.state.lock().unwrap().path.clone()
    }

    /// Returns the decoded stream information.
    pub fn info(&self) -> Option<VideoInfo> {
        self.state.lock().unwrap().info
    }

    /// Returns the most recently presented video frame without copying pixels.
    pub fn frame(&self) -> Option<ImageData> {
        self.state.lock().unwrap().image.clone()
    }

    /// Configures autoplay when a new file is opened.
    pub fn set_autoplay(&self, autoplay: bool) {
        self.state.lock().unwrap().autoplay = autoplay;
    }

    /// Starts or resumes playback.
    pub fn play(&self) {
        let mut state = self.state.lock().unwrap();
        if state.decoder.is_some() {
            if state.ended {
                let _ = state.seek_to(Duration::ZERO);
            }
            if !state.playing {
                state.playing = true;
                state.restart_clock();
            }
        }
        drop(state);
        #[cfg(feature = "audio")]
        if let Some(audio) = self.audio.lock().unwrap().as_ref() { audio.play(); }
        self.mark_dirty();
    }

    /// Pauses playback without discarding the last video frame.
    pub fn pause(&self) {
        let mut state = self.state.lock().unwrap();
        if state.playing {
            state.position = state.current_position();
            state.playing = false;
            state.clock = None;
        }
        drop(state);
        #[cfg(feature = "audio")]
        if let Some(audio) = self.audio.lock().unwrap().as_ref() { audio.pause(); }
        self.mark_dirty();
    }

    /// Stops playback and resets the video position to zero.
    pub fn stop(&self) -> Result<(), WidgetError> {
        let mut state = self.state.lock().unwrap();
        if state.decoder.is_some() {
            state.playing = false;
            state.seek_to(Duration::ZERO)?;
            state.clock = None;
        }
        drop(state);
        #[cfg(feature = "audio")]
        if let Some(audio) = self.audio.lock().unwrap().as_ref() { audio.stop(); }
        self.mark_dirty();
        Ok(())
    }

    /// Repositions playback (currently forward-decoded from the start).
    pub fn seek(&self, seconds: f64) -> Result<(), WidgetError> {
        if !seconds.is_finite() || seconds < 0.0 {
            return Err(WidgetError("video seek position must be finite and nonnegative".into()));
        }
        if seconds >= u64::MAX as f64 {
            return Err(WidgetError("video seek position is too large".into()));
        }
        let target = Duration::from_secs_f64(seconds);
        self.state.lock().unwrap().seek_to(target)?;
        #[cfg(feature = "audio")]
        if let Some(audio) = self.audio.lock().unwrap().as_ref() { audio.seek(seconds); }
        self.mark_dirty();
        Ok(())
    }

    /// Current playback position in seconds.
    pub fn position(&self) -> f64 {
        self.state.lock().unwrap().current_position().as_secs_f64()
    }

    /// Stream duration in seconds if available.
    pub fn duration(&self) -> Option<f64> {
        self.info().and_then(|info| info.duration.map(|duration| duration.as_secs_f64()))
    }

    /// True when playback is actively advancing.
    pub fn is_playing(&self) -> bool {
        self.state.lock().unwrap().playing
    }

    /// Enables looping at end of stream.
    pub fn set_looping(&self, enabled: bool) {
        self.state.lock().unwrap().looping = enabled;
        #[cfg(feature = "audio")]
        if let Some(audio) = self.audio.lock().unwrap().as_ref() { audio.set_looping(enabled); }
    }

    /// Controls whether the image is letterboxed to preserve its aspect ratio.
    pub fn set_keep_aspect_ratio(&self, enabled: bool) {
        *self.keep_aspect_ratio.lock().unwrap() = enabled;
        self.mark_dirty();
    }

    /// Advances decoding to the media-clock position. Returns true on a new frame.
    /// Call this on each event-loop iteration when not attached to a WidgetWindow.
    pub fn update(&self) -> Result<bool, WidgetError> {
        self.state.lock().unwrap().advance()
    }

    /// Renders the most recent frame using MataWin's native scaled ARGB blitter.
    pub fn blit(&self, target: &mut Framebuffer) {
        if !self.widget.visible() { return; }
        let Some(image) = self.frame() else { return; };
        let computed = self.widget.computed_style((target.width, target.height), None);
        if computed.content_bounds.width == 0 || computed.content_bounds.height == 0 { return; }
        blit_scaled(&image, target, computed.content_bounds,
            *self.keep_aspect_ratio.lock().unwrap(), computed.opacity, computed.border_radius);
    }

    /// Attaches a separate decoded audio stream to the video transport.
    /// `mata-video` currently decodes video frames only (not embedded audio).
    #[cfg(feature = "audio")]
    pub fn set_audio_file(&self, path: impl AsRef<Path>, show_controls: bool) -> Result<(), WidgetError> {
        let state = self.state.lock().unwrap();
        let source = AudioSource::open(path, state.playing).map_err(WidgetError)?;
        source.set_looping(state.looping);
        source.seek(state.current_position().as_secs_f64());
        drop(state);
        *self.audio.lock().unwrap() = Some(source.clone());
        *self.controls.lock().unwrap() = show_controls.then(|| MediaControls::new(source));
        Ok(())
    }

    /// Returns audio-only controls when a separate audio file is attached.
    #[cfg(feature = "audio")]
    pub fn controls(&self) -> Option<MediaControls> {
        self.controls.lock().unwrap().clone()
    }
}

impl WidgetExt for Video {
    fn widget(&self) -> &Widget { &self.widget }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_pixels_are_shared_without_conversion() {
        let frame = VideoFrame {
            width: 2, height: 1, pixels: vec![0xff112233, 0x80224466],
            timestamp: Duration::ZERO, duration: None,
        };
        let image = frame_image(frame).unwrap();
        assert_eq!(&image.pixels[..], &[0xff112233, 0x80224466]);
        assert_eq!(Arc::strong_count(&image.pixels), 1);
    }
    #[test]
    fn video_clock_pauses_without_drift() {
        let mut state = VideoState::default();
        state.position = Duration::from_millis(250);
        assert_eq!(state.current_position(), Duration::from_millis(250));
        assert!(!state.playing);
    }
    #[test]
    fn frame_end_uses_framerate_when_duration_missing() {
        let frame = VideoFrame { width: 1, height: 1, pixels: vec![0xffffffff],
            timestamp: Duration::from_secs(2), duration: None };
        let info = VideoInfo { width: 1, height: 1, frame_rate_numerator: 25,
            frame_rate_denominator: 1, duration: None,
            pixel_format: mata_video::PixelFormat::Argb32 };
        assert_eq!(frame_end(&frame, info), Duration::from_millis(2040));
    }
    #[test]
    fn empty_video_does_not_advance() {
        let player = Video::new();
        assert!(!player.update().unwrap());
        assert!(!player.is_playing());
    }
}
