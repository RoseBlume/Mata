//! Threaded video-only playback, bounded prefetch and a presentation clock.
//!
//! The decoder is opened and used on one dedicated worker thread (important
//! for COM on Windows). UI threads can call `poll_frame()` without blocking
//! on decoding or waiting for frame presentation.
use std::{
    collections::VecDeque,
    path::Path,
    sync::{mpsc::{self, Receiver, Sender, RecvTimeoutError}, Arc, Mutex},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use crate::{SeekMode, VideoDecoder, VideoError, VideoFrame, VideoInfo};
use crate::frame_cache::FrameCache;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackState { Paused, Playing, Ended, Failed }

/// Limits apply to the decoded ARGB32 frame queue, not native decoder surfaces.
#[derive(Debug, Clone, Copy)]
pub struct PlaybackOptions {
    /// Initial desired number of frames to decode ahead.
    pub prebuffer_frames: usize,
    /// Lower and upper limits of the adaptive queue target.
    pub min_buffer_frames: usize,
    pub max_buffer_frames: usize,
    /// Hard cap for queued frame allocations, in bytes.
    pub max_buffer_bytes: usize,
    /// Additional bytes reserved for reusable ARGB32 frames, separate from
    /// max_buffer_bytes. Zero disables decoded-frame caching.
    pub max_cached_bytes: usize,
    /// Adjust buffer target based on decode cost and underruns.
    pub adaptive_buffering: bool,
    /// Discard old frames when the display clock is ahead of buffered frames.
    pub drop_late_frames: bool,
    /// Playback rate. This is a *video-only* clock; audio time stretching is not provided.
    pub playback_rate: f64,
    /// Output size. None keeps original decoded dimensions.
    pub output_resolution: Option<(u32, u32)>,
}
impl Default for PlaybackOptions {
    fn default() -> Self {
        Self {
            prebuffer_frames: 8, min_buffer_frames: 2, max_buffer_frames: 24,
            max_buffer_bytes: 128 * 1024 * 1024,
            max_cached_bytes: 32 * 1024 * 1024,
            adaptive_buffering: true, drop_late_frames: true,
            playback_rate: 1.0, output_resolution: None,
        }
    }
}
impl PlaybackOptions {
    fn validate(&self) -> Result<(), VideoError> {
        if self.min_buffer_frames == 0 || self.max_buffer_frames < self.min_buffer_frames
            || self.prebuffer_frames < self.min_buffer_frames
            || self.prebuffer_frames > self.max_buffer_frames
            || self.max_buffer_bytes == 0
        { return Err(VideoError::new("invalid frame buffering limits")); }
        validate_rate(self.playback_rate)?;
        if let Some((width, height)) = self.output_resolution {
            if width == 0 || height == 0 || (width as usize).checked_mul(height as usize).and_then(|n| n.checked_mul(4)).is_none() {
                return Err(VideoError::new("invalid output dimensions"));
            }
        }
        Ok(())
    }
}
fn validate_memory_for_size(info: VideoInfo, max_bytes: usize) -> Result<(), VideoError> {
    let min_bytes = (info.width as usize).checked_mul(info.height as usize)
        .and_then(|pixels| pixels.checked_mul(4))
        .ok_or_else(|| VideoError::new("video dimensions overflow memory calculation"))?;
    if min_bytes > max_bytes {
        Err(VideoError::new(format!(
            "even one {}x{} ARGB32 frame requires {min_bytes} bytes; increase max_buffer_bytes above {max_bytes}",
            info.width, info.height,
        )))
    } else { Ok(()) }
}
fn validate_rate(rate: f64) -> Result<(), VideoError> {
    if !rate.is_finite() || !(0.1..=8.0).contains(&rate) {
        Err(VideoError::new("playback rate must be finite and between 0.1 and 8.0"))
    } else { Ok(()) }
}

/// Instantaneous queue state and cumulative counters. The counters are reset
/// only when the player is opened, not on seek or pause.
#[derive(Debug, Clone, Copy)]
pub struct BufferStats {
    pub buffered_frames: usize,
    pub buffered_bytes: usize,
    pub cached_frames: usize,
    pub cached_bytes: usize,
    pub cache_hits: u64,
    pub seek_requests: u64,
    pub seek_completions: u64,
    pub seek_pending: bool,
    pub buffered_duration: Duration,
    pub target_buffer_frames: usize,
    pub decoded_frames: u64,
    pub presented_frames: u64,
    pub dropped_frames: u64,
    pub seek_discarded_frames: u64,
    pub buffer_underruns: u64,
    pub peak_buffered_bytes: usize,
    pub average_decode_time: Duration,
    pub estimated_decode_fps: f64,
    pub playback_position: Duration,
    pub end_of_stream: bool,
    pub state: PlaybackState,
}

/// Completion of the most recently requested asynchronous seek.
/// Superseded seeks never produce completion events or stale frames.
#[derive(Debug, Clone)]
pub struct SeekCompletion {
    pub id: u64,
    pub requested: Duration,
    pub mode: SeekMode,
    /// Presentation timestamp of the selected frame, or None at EOF.
    pub result: Result<Option<Duration>, VideoError>,
    pub from_cache: bool,
}

#[derive(Debug, Clone, Copy)]
struct SeekRequest { id: u64, at: Duration, mode: SeekMode }

#[derive(Clone, Copy)]
struct Clock {
    media: Duration,
    wall: Instant,
    rate: f64,
    playing: bool,
}
impl Clock {
    fn new(rate: f64) -> Self { Self { media: Duration::ZERO, wall: Instant::now(), rate, playing: false } }
    fn position(&self) -> Duration {
        if !self.playing { return self.media; }
        self.media.saturating_add(Duration::from_secs_f64(
            (self.wall.elapsed().as_secs_f64() * self.rate).min(u64::MAX as f64)
        ))
    }
    fn reset(&mut self, at: Duration) { self.media = at; self.wall = Instant::now(); }
    fn set_playing(&mut self, playing: bool) {
        self.media = self.position(); self.wall = Instant::now(); self.playing = playing;
    }
    fn set_rate(&mut self, rate: f64) {
        self.media = self.position(); self.wall = Instant::now(); self.rate = rate;
    }
}

struct Shared {
    queue: VecDeque<VideoFrame>,
    cache: FrameCache,
    cache_hits: u64,
    next_seek_id: u64,
    completed_seeks: u64,
    pending_seek: Option<SeekRequest>,
    seek_result: Option<SeekCompletion>,
    // When a cached preview is used, native decoder position is unchanged.
    // The next play / step must resynchronize before it decodes more frames.
    parked_at: Option<Duration>,
    bytes: usize,
    info: VideoInfo,
    options: PlaybackOptions,
    target: usize,
    clock: Clock,
    state: PlaybackState,
    eof: bool,
    error: Option<VideoError>,
    decoded: u64,
    presented: u64,
    dropped: u64,
    seek_discarded: u64,
    underruns: u64,
    peak_bytes: usize,
    avg_decode_seconds: f64,
    stable_frames: usize,
    underrun_active: bool,
}
impl Shared {
    fn new(info: VideoInfo, opts: PlaybackOptions) -> Self {
        Self {
            queue: VecDeque::new(), bytes: 0, info, target: opts.prebuffer_frames,
            cache: FrameCache::default(), cache_hits: 0,
            next_seek_id: 0, completed_seeks: 0, pending_seek: None,
            seek_result: None, parked_at: None,
            clock: Clock::new(opts.playback_rate), options: opts,
            state: PlaybackState::Paused, eof: false, error: None,
            decoded: 0, presented: 0, dropped: 0, seek_discarded: 0,
            underruns: 0, peak_bytes: 0, avg_decode_seconds: 0.0,
            stable_frames: 0, underrun_active: false,
        }
    }
    fn buffer_bytes(frame: &VideoFrame) -> usize {
        frame.pixels.capacity() * std::mem::size_of::<u32>()
    }
    fn pop_front(&mut self) -> Option<VideoFrame> {
        let frame = self.queue.pop_front()?;
        self.bytes = self.bytes.saturating_sub(Self::buffer_bytes(&frame));
        Some(frame)
    }
    fn pop_back(&mut self) -> Option<VideoFrame> {
        let frame = self.queue.pop_back()?;
        self.bytes = self.bytes.saturating_sub(Self::buffer_bytes(&frame));
        Some(frame)
    }
    fn clear(&mut self) { self.queue.clear(); self.bytes = 0; self.underrun_active = false; }
    fn can_decode(&self) -> bool {
        // Reserve space for another expected frame *before* reading it.
        // Otherwise a decoder can repeatedly decode-and-discard at the cap.
        let expected = (self.info.width as usize)
            .checked_mul(self.info.height as usize)
            .and_then(|n| n.checked_mul(4))
            .unwrap_or(usize::MAX);
        self.pending_seek.is_none() && self.parked_at.is_none()
            && !self.eof && self.error.is_none()
            && self.queue.len() < self.target
            && expected <= self.options.max_buffer_bytes.saturating_sub(self.bytes)
    }
    fn observe_decode(&mut self, elapsed: Duration) {
        let seconds = elapsed.as_secs_f64();
        if self.decoded == 0 { self.avg_decode_seconds = seconds; }
        else { self.avg_decode_seconds = self.avg_decode_seconds * 0.9 + seconds * 0.1; }
        self.decoded += 1;
        if !self.options.adaptive_buffering { return; }
        let fps = self.info.frames_per_second().unwrap_or(30.0).clamp(1.0, 240.0);
        let budget = 1.0 / (fps * self.clock.rate);
        if self.avg_decode_seconds > budget * 0.65 {
            self.target = (self.target + 1).min(self.options.max_buffer_frames);
            self.stable_frames = 0;
        } else if self.avg_decode_seconds < budget * 0.25 {
            self.stable_frames += 1;
            if self.stable_frames >= 120 {
                self.target = self.target.saturating_sub(1).max(self.options.min_buffer_frames);
                self.stable_frames = 0;
            }
        } else { self.stable_frames = 0; }
    }
    fn stats(&self) -> BufferStats {
        let position = self.clock.position();
        let buffered_duration = self.queue.back().map(|last|
            last.timestamp.saturating_sub(position)
                .saturating_add(last.duration.unwrap_or_default())
        ).unwrap_or_default();
        BufferStats {
            buffered_frames: self.queue.len(), buffered_bytes: self.bytes,
            cached_frames: self.cache.len(), cached_bytes: self.cache.bytes(),
            cache_hits: self.cache_hits, seek_requests: self.next_seek_id,
            seek_completions: self.completed_seeks,
            seek_pending: self.pending_seek.is_some() || self.next_seek_id > self.completed_seeks,
            buffered_duration, target_buffer_frames: self.target,
            decoded_frames: self.decoded, presented_frames: self.presented,
            dropped_frames: self.dropped, seek_discarded_frames: self.seek_discarded,
            buffer_underruns: self.underruns, peak_buffered_bytes: self.peak_bytes,
            average_decode_time: Duration::from_secs_f64(self.avg_decode_seconds),
            estimated_decode_fps: if self.avg_decode_seconds > 0.0 { 1.0 / self.avg_decode_seconds } else { 0.0 },
            playback_position: self.info.duration.map(|duration| position.min(duration)).unwrap_or(position),
            end_of_stream: self.eof, state: self.state,
        }
    }
    fn finish_if_drained(&mut self) {
        if self.eof && self.queue.is_empty() {
            self.clock.set_playing(false);
            if let Some(duration) = self.info.duration { self.clock.reset(duration); }
            self.state = PlaybackState::Ended;
        }
    }
    fn start_if_ready(&mut self) {
        if self.state != PlaybackState::Playing || self.clock.playing { return; }
        let frame_bytes = (self.info.width as usize)
            .saturating_mul(self.info.height as usize).saturating_mul(4).max(1);
        let max_fit = (self.options.max_buffer_bytes / frame_bytes).max(1);
        let needed = self.target.min(max_fit);
        if self.queue.len() >= needed || (self.eof && !self.queue.is_empty()) {
            // Preserve an explicit seek position if possible. Otherwise begin
            // at the first actual PTS rather than considering preroll late.
            if let Some(first) = self.queue.front() {
                if self.clock.media < first.timestamp { self.clock.reset(first.timestamp); }
            }
            self.clock.set_playing(true);
        }
    }
}

type Response<T> = Sender<Result<T, VideoError>>;
enum Command {
    Play(Response<()>), Pause(Response<()>),
    Seek(Duration, SeekMode, Response<()>), Step(Response<Option<VideoFrame>>),
    Resize(Option<(u32,u32)>, Response<()>), Rate(f64, Response<()>),
    Options(PlaybackOptions, Response<()>), WakeSeek, Stop,
}

/// Video-only playback controller. The queue and stats can be inspected from a
/// UI thread; all native decoder operations run on the worker thread.
pub struct VideoPlayer {
    shared: Arc<Mutex<Shared>>,
    sender: Sender<Command>,
    worker: Option<JoinHandle<()>>,
}
impl VideoPlayer {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, VideoError> {
        Self::open_with_options(path, PlaybackOptions::default())
    }
    pub fn open_with_options(path: impl AsRef<Path>, options: PlaybackOptions) -> Result<Self, VideoError> {
        options.validate()?;
        let path = path.as_ref().to_owned();
        let (sender, receiver) = mpsc::channel();
        let (init_sender, init_receiver) = mpsc::sync_channel(1);
        // Create native COM objects inside this thread, not the caller's thread.
        let worker = thread::Builder::new().name("mata-video-decode".into()).spawn(move || {
            let mut decoder = match VideoDecoder::open(&path) {
                Ok(decoder) => decoder,
                Err(error) => { let _ = init_sender.send(Err(error)); return; }
            };
            if let Some(size) = options.output_resolution {
                if let Err(error) = decoder.set_output_resolution(Some(size)) {
                    let _ = init_sender.send(Err(error)); return;
                }
            }
            let info = decoder.info();
            if let Err(error) = validate_memory_for_size(info, options.max_buffer_bytes) {
                let _ = init_sender.send(Err(error)); return;
            }
            let shared = Arc::new(Mutex::new(Shared::new(info, options)));
            if init_sender.send(Ok(Arc::clone(&shared))).is_err() { return; }
            worker_loop(decoder, shared, receiver);
        }).map_err(|error| VideoError::new(format!("failed to create video worker: {error}")))?;
        let shared = match init_receiver.recv() {
            Ok(Ok(shared)) => shared,
            Ok(Err(error)) => { let _ = worker.join(); return Err(error); },
            Err(_) => { let _ = worker.join(); return Err(VideoError::new("video decoder stopped during initialization")); },
        };
        Ok(Self { shared, sender, worker: Some(worker) })
    }
    fn send<T>(&self, build: impl FnOnce(Response<T>) -> Command) -> Result<T, VideoError> {
        let (sender, receiver) = mpsc::channel();
        self.sender.send(build(sender))
            .map_err(|_| VideoError::new("video worker is not running"))?;
        receiver.recv().map_err(|_| VideoError::new("video worker did not respond"))?
    }
    pub fn info(&self) -> VideoInfo { self.shared.lock().unwrap().info }
    pub fn duration(&self) -> Option<Duration> { self.info().duration }
    pub fn position(&self) -> Duration { self.stats().playback_position }
    pub fn state(&self) -> PlaybackState { self.shared.lock().unwrap().state }
    pub fn last_error(&self) -> Option<VideoError> { self.shared.lock().unwrap().error.clone() }
    pub fn stats(&self) -> BufferStats { self.shared.lock().unwrap().stats() }
    pub fn buffered_frames(&self) -> usize { self.stats().buffered_frames }
    pub fn buffered_bytes(&self) -> usize { self.stats().buffered_bytes }
    pub fn playback_rate(&self) -> f64 { self.options().playback_rate }
    pub fn seek_relative(&self, offset: i64) -> Result<(), VideoError> {
        let current = self.position();
        let target = if offset >= 0 { current.saturating_add(Duration::from_millis(offset as u64)) }
            else { current.saturating_sub(Duration::from_millis(offset.unsigned_abs())) };
        self.seek(target)
    }
    pub fn play(&self) -> Result<(), VideoError> { self.send(Command::Play) }
    pub fn pause(&self) -> Result<(), VideoError> { self.send(Command::Pause) }
    /// Reset playback to zero and pause, retaining the same file.
    pub fn stop(&self) -> Result<(), VideoError> {
        self.pause()?; self.seek(Duration::ZERO)
    }
    /// Precise seek using the container's native seek table when available.
    /// Only frames from a nearby keyframe onward need decoding. This may
    /// block until the first frame at or after `position` has been decoded.
    pub fn seek(&self, position: Duration) -> Result<(), VideoError> {
        self.seek_with_mode(position, SeekMode::Accurate)
    }

    /// Seek directly to a nearby keyframe/seek point, without decoding
    /// forward to `position`. Useful for rapidly dragging the timeline.
    /// The returned preview can be earlier than the requested timestamp.
    pub fn seek_fast(&self, position: Duration) -> Result<(), VideoError> {
        self.seek_with_mode(position, SeekMode::Keyframe)
    }

    /// Select accurate playback seeking or fast keyframe seeking.
    /// Native seeking and the initial frame decode happen on the worker.
    pub fn seek_with_mode(&self, position: Duration, mode: SeekMode) -> Result<(), VideoError> {
        // Cancel pending async requests when a synchronous seek is requested.
        {
            let mut s = self.shared.lock().unwrap();
            s.pending_seek = None;
            s.next_seek_id = s.next_seek_id.wrapping_add(1);
            s.completed_seeks = s.next_seek_id;
            s.seek_result = None;
        }
        self.send(|sender| Command::Seek(position, mode, sender))
    }

    /// Schedule a seek without blocking the calling/UI thread. Repeated calls
    /// overwrite the pending seek rather than filling the command channel.
    /// Returned IDs can be matched against `poll_seek_result`.
    pub fn request_seek_with_mode(&self, position: Duration, mode: SeekMode) -> Result<u64, VideoError> {
        let mut s = self.shared.lock().unwrap();
        s.next_seek_id = s.next_seek_id.wrapping_add(1);
        let id = s.next_seek_id;
        let needs_wakeup = s.pending_seek.is_none();
        s.pending_seek = Some(SeekRequest { id, at: position, mode });
        s.seek_result = None;
        s.clear();
        s.eof = false;
        s.parked_at = None;
        s.clock.set_playing(false);
        s.clock.reset(position);
        if s.state == PlaybackState::Ended { s.state = PlaybackState::Paused; }
        // Never block on a decoder operation in a UI-facing request call.
        // An unbounded channel send is a short in-process notification.
        if needs_wakeup {
            self.sender.send(Command::WakeSeek)
                .map_err(|_| VideoError::new("video worker is not running"))?;
        }
        Ok(id)
    }
    pub fn request_seek(&self, position: Duration) -> Result<u64, VideoError> {
        self.request_seek_with_mode(position, SeekMode::Accurate)
    }
    pub fn request_seek_fast(&self, position: Duration) -> Result<u64, VideoError> {
        self.request_seek_with_mode(position, SeekMode::Keyframe)
    }
    /// Nonblocking result. Returns only the newest request's completion.
    pub fn poll_seek_result(&self) -> Option<SeekCompletion> {
        self.shared.lock().unwrap().seek_result.take()
    }
    /// Retrieve a completed preview while paused without starting playback.
    /// When playing, use poll_frame() instead.
    pub fn poll_seek_preview_frame(&self) -> Option<VideoFrame> {
        let mut s = self.shared.lock().unwrap();
        if s.state == PlaybackState::Playing || s.pending_seek.is_some() { return None; }
        let frame = s.pop_front()?;
        s.clock.reset(frame.timestamp);
        s.presented += 1;
        Some(frame)
    }
    /// Bound additional cache memory independently from the playback queue.
    pub fn set_max_cached_bytes(&self, bytes: usize) -> Result<(), VideoError> {
        let mut options = self.options();
        options.max_cached_bytes = bytes;
        self.set_buffer_options(options)
    }
    /// Step exactly one decoded frame while paused. Does not use frame dropping.
    /// May block the caller while the decoder produces a sample.
    pub fn step_forward(&self) -> Result<Option<VideoFrame>, VideoError> {
        self.send(Command::Step)
    }
    pub fn set_output_resolution(&self, resolution: Option<(u32,u32)>) -> Result<(), VideoError> {
        self.send(|sender| Command::Resize(resolution, sender))
    }
    pub fn set_playback_rate(&self, rate: f64) -> Result<(), VideoError> {
        self.send(|sender| Command::Rate(rate, sender))
    }
    pub fn options(&self) -> PlaybackOptions { self.shared.lock().unwrap().options }
    pub fn set_buffer_options(&self, options: PlaybackOptions) -> Result<(), VideoError> {
        self.send(|sender| Command::Options(options, sender))
    }
    pub fn set_prebuffer_frames(&self, frames: usize) -> Result<(), VideoError> {
        let mut options = self.options(); options.prebuffer_frames = frames;
        options.min_buffer_frames = options.min_buffer_frames.min(frames);
        options.max_buffer_frames = options.max_buffer_frames.max(frames);
        self.set_buffer_options(options)
    }
    pub fn set_max_buffer_bytes(&self, bytes: usize) -> Result<(), VideoError> {
        let mut options = self.options(); options.max_buffer_bytes = bytes;
        self.set_buffer_options(options)
    }
    pub fn set_adaptive_buffering(&self, enabled: bool) -> Result<(), VideoError> {
        let mut options = self.options(); options.adaptive_buffering = enabled;
        self.set_buffer_options(options)
    }
    pub fn set_frame_dropping(&self, enabled: bool) -> Result<(), VideoError> {
        let mut options = self.options(); options.drop_late_frames = enabled;
        self.set_buffer_options(options)
    }
    /// Returns at most one due frame, dropping superseded frames to catch up.
    /// Returns None when paused, a frame is not due yet, or the queue is empty.
    /// Does not block the UI thread. Check `last_error()` for worker failures.
    pub fn poll_frame(&self) -> Option<VideoFrame> {
        let mut s = self.shared.lock().unwrap();
        if s.state != PlaybackState::Playing { return None; }
        if !s.clock.playing { return None; } // Prebuffer before starting media time.
        let now = s.clock.position();
        if s.options.drop_late_frames {
            while s.queue.len() > 1 && s.queue.get(1).is_some_and(|next| next.timestamp <= now) {
                s.pop_front(); s.dropped += 1;
            }
        }
        if s.queue.front().is_some_and(|first| first.timestamp <= now) {
            let frame = s.pop_front();
            s.presented += 1;
            s.underrun_active = false;
            s.finish_if_drained();
            return frame;
        }
        if s.queue.is_empty() && !s.eof && !s.underrun_active {
            s.underruns += 1;
            s.underrun_active = true;
            if s.options.adaptive_buffering {
                s.target = (s.target + 3).min(s.options.max_buffer_frames);
                s.stable_frames = 0;
            }
        }
        s.finish_if_drained();
        None
    }
}
impl Drop for VideoPlayer {
    fn drop(&mut self) {
        let _ = self.sender.send(Command::Stop);
        if let Some(worker) = self.worker.take() { let _ = worker.join(); }
    }
}

fn worker_loop(mut decoder: VideoDecoder, shared: Arc<Mutex<Shared>>, receiver: Receiver<Command>) {
    loop {
        // Commands take priority over decoding; no decoder call is made with
        // the shared mutex locked. The consumer can always read queue/stats.
        match receiver.try_recv() {
            Ok(cmd) => { if !command(cmd, &mut decoder, &shared) { break; } continue; },
            Err(mpsc::TryRecvError::Disconnected) => break,
            Err(mpsc::TryRecvError::Empty) => {}
        }
        // A seek supersedes speculative frame decode, including when the
        // decoder is otherwise paused/full. Do not process stale seek commands.
        if let Some(request) = { shared.lock().unwrap().pending_seek.take() } {
            let result = seek_decoder(&mut decoder, &shared, request.at, request.mode, Some(request.id), true);
            let mut s = shared.lock().unwrap();
            if s.next_seek_id == request.id {
                s.completed_seeks = request.id;
                let from_cache = s.parked_at.is_some();
                let completion = SeekCompletion {
                    id: request.id, requested: request.at, mode: request.mode,
                    result: result.clone().map(|_| s.queue.front().map(|f| f.timestamp)),
                    from_cache,
                };
                s.seek_result = Some(completion);
                if let Err(error) = result {
                    s.error = Some(error);
                    s.state = PlaybackState::Failed;
                    s.clock.set_playing(false);
                }
            }
            continue;
        }
        if shared.lock().unwrap().can_decode() {
            if let Err(error) = decode_one(&mut decoder, &shared) {
                let mut s = shared.lock().unwrap();
                s.error = Some(error);
                s.clock.set_playing(false);
                s.state = PlaybackState::Failed;
                s.eof = true;
            }
            continue;
        }
        match receiver.recv_timeout(Duration::from_millis(25)) {
            Ok(cmd) => if !command(cmd, &mut decoder, &shared) { break; },
            Err(RecvTimeoutError::Disconnected) => break,
            Err(RecvTimeoutError::Timeout) => {},
        }
    }
}

fn decode_one(decoder: &mut VideoDecoder, shared: &Arc<Mutex<Shared>>) -> Result<(), VideoError> {
    let began = Instant::now();
    let next = decoder.next_frame()?;
    let mut s = shared.lock().unwrap();
    if let Some(frame) = next {
        let size = Shared::buffer_bytes(&frame);
        if size > s.options.max_buffer_bytes {
            return Err(VideoError::new(format!("one decoded frame requires {size} bytes, exceeding the queue memory cap of {}", s.options.max_buffer_bytes)));
        }
        s.observe_decode(began.elapsed());
        // Can happen after a buffer limit is reduced (the worker itself is
        // single-threaded but the UI can consume buffered frames concurrently).
        if size <= s.options.max_buffer_bytes.saturating_sub(s.bytes)
            && s.queue.len() < s.options.max_buffer_frames
        {
            let cache_cap = s.options.max_cached_bytes;
            s.cache.insert(&frame, cache_cap);
            s.bytes += size;
            s.peak_bytes = s.peak_bytes.max(s.bytes);
            s.queue.push_back(frame);
            s.underrun_active = false;
            s.start_if_ready();
        } else {
            // Backpressure should normally prevent this branch. Prefer
            // discarding a newly decoded frame to exceeding the memory cap.
            s.dropped += 1;
        }
    } else { s.eof = true; s.finish_if_drained(); s.start_if_ready(); }
    Ok(())
}

fn seek_decoder(
    decoder: &mut VideoDecoder,
    shared: &Arc<Mutex<Shared>>,
    at: Duration,
    mode: SeekMode,
    request_id: Option<u64>,
    allow_cache: bool,
) -> Result<(), VideoError> {
    // For paused preview seeks, an exact nearby decoded frame is immediately
    // reusable without touching the native decoder. Reposition on play/step.
    {
        let mut s = shared.lock().unwrap();
        let is_latest = request_id.is_none_or(|id| id == s.next_seek_id);
        if !is_latest { return Ok(()); }
        if allow_cache && s.state != PlaybackState::Playing && s.options.max_cached_bytes > 0 {
            let fps = s.info.frames_per_second().unwrap_or(30.0).clamp(1.0, 240.0);
            let interval = Duration::from_secs_f64(1.0 / fps);
            if let Some(frame) = s.cache.lookup(at, mode, interval) {
                s.clear();
                s.eof = false;
                s.error = None;
                s.clock.set_playing(false);
                s.clock.reset(frame.timestamp);
                if matches!(s.state, PlaybackState::Ended | PlaybackState::Failed) { s.state = PlaybackState::Paused; }
                s.parked_at = Some(frame.timestamp);
                s.cache_hits += 1;
                s.bytes = Shared::buffer_bytes(&frame);
                s.queue.push_back(frame);
                return Ok(());
            }
        }
    }
    // Jump via the demuxer's native seek/index first; never replay the whole
    // stream through next_frame() merely to reach a later timestamp.
    decoder.seek_with_mode(at, mode)?;
    if request_id.is_some_and(|id| shared.lock().unwrap().next_seek_id != id) {
        return Ok(());
    }
    {
        let mut s = shared.lock().unwrap();
        s.clear();
        s.parked_at = None;
        s.eof = false;
        s.error = None;
        s.clock.reset(at);
        if s.state == PlaybackState::Playing { s.clock.set_playing(false); }
        if matches!(s.state, PlaybackState::Ended | PlaybackState::Failed) { s.state = PlaybackState::Paused; }
    }

    let began = Instant::now();
    let (frame, skipped) = match mode {
        // Native backends drop local keyframe preroll *before* copying or
        // resizing pixels. This only decodes frames near the seek target.
        SeekMode::Accurate => decoder.next_frame_at_or_after(at)?,
        // No exact-timestamp forward decoding, just grab the first image
        // available at the seek point for an immediate scrub preview.
        SeekMode::Keyframe => (decoder.next_frame()?, 0),
    };
    let mut s = shared.lock().unwrap();
    if request_id.is_some_and(|id| s.next_seek_id != id) { return Ok(()); }
    s.seek_discarded = s.seek_discarded.saturating_add(skipped);
    s.decoded = s.decoded.saturating_add(skipped);
    match frame {
        Some(frame) => {
            let size = Shared::buffer_bytes(&frame);
            if size > s.options.max_buffer_bytes {
                return Err(VideoError::new("decoded frame exceeds queue memory limit"));
            }
            // Average decode time is per native frame in the local GOP, not
            // the wall time of an entire seek containing several samples.
            let count = skipped.saturating_add(1);
            s.observe_decode(Duration::from_secs_f64(began.elapsed().as_secs_f64() / count as f64));
            // Fast/keyframe seeking may land before the requested timestamp.
            // Start playback at the frame that was actually retrieved instead
            // of immediately dropping that frame as artificially "late".
            if mode == SeekMode::Keyframe { s.clock.reset(frame.timestamp); }
            let cache_cap = s.options.max_cached_bytes;
            s.cache.insert(&frame, cache_cap);
            s.bytes += size;
            s.peak_bytes = s.peak_bytes.max(s.bytes);
            s.queue.push_back(frame);
            s.start_if_ready();
        },
        None => {
            s.eof = true;
            s.finish_if_drained();
            s.start_if_ready();
        },
    }
    Ok(())
}

fn command(cmd: Command, decoder: &mut VideoDecoder, shared: &Arc<Mutex<Shared>>) -> bool {
    match cmd {
        Command::Stop => return false,
        Command::WakeSeek => {},
        Command::Play(reply) => {
            let parked = shared.lock().unwrap().parked_at;
            let result = if shared.lock().unwrap().state == PlaybackState::Failed {
                Err(VideoError::new("cannot resume a failed decoder"))
            } else if let Some(at) = parked {
                seek_decoder(decoder, shared, at, SeekMode::Accurate, None, false)
            } else {
                let s = shared.lock().unwrap();
                if s.state == PlaybackState::Ended {
                    drop(s);
                    seek_decoder(decoder, shared, Duration::ZERO, SeekMode::Accurate, None, false)
                } else { Ok(()) }
            };
            if result.is_ok() {
                let mut s = shared.lock().unwrap();
                if !s.eof || !s.queue.is_empty() {
                    s.state = PlaybackState::Playing;
                    s.start_if_ready();
                }
            }
            let _ = reply.send(result);
        },
        Command::Pause(reply) => {
            let mut s = shared.lock().unwrap();
            s.clock.set_playing(false);
            if s.state == PlaybackState::Playing { s.state = PlaybackState::Paused; }
            let _ = reply.send(Ok(()));
        },
        Command::Seek(at, mode, reply) => {
            let result = seek_decoder(decoder, shared, at, mode, None, true);
            let _ = reply.send(result);
        },
        Command::Step(reply) => {
            let mut s = shared.lock().unwrap();
            if s.state == PlaybackState::Playing {
                let _ = reply.send(Err(VideoError::new("pause playback before stepping")));
                return true;
            }
            if s.state == PlaybackState::Failed {
                let _ = reply.send(Err(s.error.clone().unwrap_or_else(|| VideoError::new("decoder failed"))));
                return true;
            }
            let parked = s.parked_at;
            let existing = if parked.is_none() { s.pop_front() } else { None };
            drop(s);
            if let Some(at) = parked {
                if let Err(error) = seek_decoder(decoder, shared, at, SeekMode::Accurate, None, false) {
                    let _ = reply.send(Err(error));
                    return true;
                }
            }
            let existing = existing.or_else(|| shared.lock().unwrap().pop_front());
            let result = if let Some(frame) = existing { Ok(Some(frame)) }
                else if shared.lock().unwrap().eof { Ok(None) }
                else { decoder.next_frame() };
            if let Ok(Some(frame)) = &result {
                let mut s = shared.lock().unwrap();
                s.clock.reset(frame.timestamp);
                s.presented += 1;
                s.state = PlaybackState::Paused;
            } else if matches!(result, Ok(None)) {
                let mut s = shared.lock().unwrap(); s.eof = true; s.finish_if_drained();
            }
            let _ = reply.send(result);
        },
        Command::Resize(resolution, reply) => {
            let (old, at, cap, original_info) = {
                let s = shared.lock().unwrap();
                (s.options.output_resolution, s.clock.position(), s.options.max_buffer_bytes, s.info)
            };
            let mut result = if let Some((width, height)) = resolution {
                validate_memory_for_size(VideoInfo { width, height, ..original_info }, cap)
            } else { Ok(()) };
            if result.is_ok() {
                let mut s = shared.lock().unwrap();
                s.cache.clear();
                s.parked_at = None;
                drop(s);
                result = decoder.set_output_resolution(resolution);
            }
            if result.is_ok() { result = validate_memory_for_size(decoder.info(), cap); }
            if result.is_ok() {
                // Old frames become invalid after a resolution change.
                result = seek_decoder(decoder, shared, at, SeekMode::Accurate, None, false);
            }
            if result.is_ok() {
                let mut s = shared.lock().unwrap();
                s.info = decoder.info();
                s.options.output_resolution = resolution;
                s.cache.clear();
                s.parked_at = None;
            } else {
                // Best-effort restoration of the prior backend output caps.
                let _ = decoder.set_output_resolution(old);
                let _ = seek_decoder(decoder, shared, at, SeekMode::Accurate, None, false);
            }
            let _ = reply.send(result);
        },
        Command::Rate(rate, reply) => {
            let result = validate_rate(rate);
            if result.is_ok() {
                let mut s = shared.lock().unwrap();
                s.clock.set_rate(rate); s.options.playback_rate = rate;
            }
            let _ = reply.send(result);
        },
        Command::Options(opts, reply) => {
            let (old, at, original_info) = {
                let s = shared.lock().unwrap();
                (s.options, s.clock.position(), s.info)
            };
            let mut result = opts.validate();
            if result.is_ok() {
                let (width, height) = opts.output_resolution
                    .unwrap_or((original_info.width, original_info.height));
                result = validate_memory_for_size(
                    VideoInfo { width, height, ..original_info }, opts.max_buffer_bytes
                );
            }
            if result.is_ok() && old.output_resolution != opts.output_resolution {
                {
                    let mut s = shared.lock().unwrap();
                    s.cache.clear();
                    s.parked_at = None;
                }
                result = decoder.set_output_resolution(opts.output_resolution);
                if result.is_ok() { result = seek_decoder(decoder, shared, at, SeekMode::Accurate, None, false); }
            }
            if result.is_ok() { result = validate_memory_for_size(decoder.info(), opts.max_buffer_bytes); }
            if result.is_ok() {
                let mut s = shared.lock().unwrap();
                s.clock.set_rate(opts.playback_rate);
                s.options = opts;
                s.target = opts.prebuffer_frames;
                s.info = decoder.info();
                if old.output_resolution != opts.output_resolution {
                    s.cache.clear();
                    s.parked_at = None;
                }
                s.cache.trim(opts.max_cached_bytes);
                while s.queue.len() > opts.max_buffer_frames || s.bytes > opts.max_buffer_bytes {
                    s.pop_back();
                }
                s.start_if_ready();
            } else if old.output_resolution != opts.output_resolution {
                let _ = decoder.set_output_resolution(old.output_resolution);
                let _ = seek_decoder(decoder, shared, at, SeekMode::Accurate, None, false);
            }
            let _ = reply.send(result);
        },
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    fn info() -> VideoInfo { VideoInfo {
        width: 2, height: 2, frame_rate_numerator: 30, frame_rate_denominator: 1,
        duration: Some(Duration::from_secs(10)), pixel_format: crate::PixelFormat::Argb32,
    }}
    fn frame(ms: u64) -> VideoFrame { VideoFrame {
        width: 2, height: 2, pixels: vec![0; 4],
        timestamp: Duration::from_millis(ms), duration: Some(Duration::from_millis(33)),
    }}
    #[test]
    fn accurate_is_default_and_keyframe_is_explicit() {
        assert_eq!(SeekMode::default(), SeekMode::Accurate);
        assert_ne!(SeekMode::Keyframe, SeekMode::Accurate);
    }
    #[test]
    fn buffer_accounting_and_drop_late_frames() {
        let mut s = Shared::new(info(), PlaybackOptions::default());
        for ms in [0, 33, 66] {
            let f = frame(ms); s.bytes += Shared::buffer_bytes(&f); s.queue.push_back(f);
        }
        let shared = Arc::new(Mutex::new(s));
        let (sender, _receiver) = mpsc::channel();
        let player = VideoPlayer { shared, sender, worker: None };
        {
            let mut guard = player.shared.lock().unwrap();
            guard.clock.reset(Duration::from_millis(70));
            guard.state = PlaybackState::Playing;
        }
        assert_eq!(player.poll_frame().unwrap().timestamp, Duration::from_millis(66));
        assert_eq!(player.stats().dropped_frames, 2);
        assert_eq!(player.stats().buffered_bytes, 0);
    }
    #[test]
    fn options_are_validated() {
        assert!(PlaybackOptions { max_buffer_bytes: 0, ..PlaybackOptions::default() }.validate().is_err());
        assert!(validate_rate(f64::NAN).is_err());
        assert!(validate_rate(-1.0).is_err());
        assert!(validate_rate(2.0).is_ok());
    }
    #[test]
    fn adaptive_buffer_target_respects_cap() {
        let mut s = Shared::new(info(), PlaybackOptions::default());
        for _ in 0..100 { s.observe_decode(Duration::from_millis(40)); }
        assert_eq!(s.target, s.options.max_buffer_frames);
    }
    #[test]
    fn prebuffer_does_not_advance_clock_until_ready() {
        let mut s = Shared::new(info(), PlaybackOptions::default());
        s.state = PlaybackState::Playing;
        let minimum = s.target;
        for ms in 0..minimum {
            let f = frame(ms as u64 * 33);
            s.bytes += Shared::buffer_bytes(&f);
            s.queue.push_back(f);
            s.start_if_ready();
            if ms + 1 < minimum { assert!(!s.clock.playing); }
        }
        assert!(s.clock.playing);
    }
    #[test]
    fn memory_check_rejects_frames_larger_than_limit() {
        assert!(validate_memory_for_size(info(), 15).is_err());
        assert!(validate_memory_for_size(info(), 16).is_ok());
    }
    #[test]
    fn pause_does_not_advance_clock() {
        let mut clock = Clock::new(2.0);
        clock.reset(Duration::from_secs(3));
        assert_eq!(clock.position(), Duration::from_secs(3));
        clock.set_playing(true);
        clock.set_playing(false);
        let p = clock.position();
        assert_eq!(clock.position(), p);
    }
}

#[cfg(test)]
mod seek_tests {
    use super::*;
    fn info() -> VideoInfo { VideoInfo { width: 2, height: 2,
        frame_rate_numerator: 30, frame_rate_denominator: 1,
        duration: Some(Duration::from_secs(10)), pixel_format: crate::PixelFormat::Argb32 } }
    #[test]
    fn latest_seek_wins_without_blocking() {
        let shared = Arc::new(Mutex::new(Shared::new(info(),PlaybackOptions::default())));
        let (sender, rx) = mpsc::channel();
        let player = VideoPlayer { shared: shared.clone(), sender, worker: None };
        let a = player.request_seek_fast(Duration::from_secs(1)).unwrap();
        let b = player.request_seek_fast(Duration::from_secs(2)).unwrap();
        assert!(b > a);
        let s = shared.lock().unwrap();
        assert_eq!(s.pending_seek.unwrap().at,Duration::from_secs(2));
        assert!(s.queue.is_empty());
        assert!(s.stats().seek_pending);
        assert_eq!(rx.try_iter().count(),1);
    }
    #[test]
    fn resize_invalidates_cached_frames() {
        let mut s = Shared::new(info(),PlaybackOptions::default());
        let f = VideoFrame {width:2,height:2,pixels:vec![0;4],
            timestamp:Duration::ZERO,duration:None};
        s.cache.insert(&f,s.options.max_cached_bytes);
        assert_eq!(s.cache.len(),1);
        s.cache.clear();
        assert_eq!(s.cache.len(),0);
    }
}
