# mata-video

Native Windows Media Foundation / Linux GStreamer streaming video decoder with
an optional high-level, std-only video playback controller.

## Decoder (no background threads)

```rust
use mata_video::{VideoDecoder, SeekMode};
use std::time::Duration;

let mut video = VideoDecoder::open("movie.mp4")?;
let duration: Option<Duration> = video.duration();
video.set_output_resolution(Some((640, 360)))?;
let keyframe_preview = video.seek_preview_frame(Duration::from_secs(10))?;
let first_frame = video.seek_frame(Duration::from_secs(10))?; // at or after 10s
# Ok::<(), mata_video::VideoError>(())
```

`VideoFrame::pixels` is numeric ARGB32 (`0xAARRGGBB`).
`VideoInfo::duration` and `VideoDecoder::duration()` are `None` when container
duration is unavailable. Seeking is timestamp-oriented, rather than by nominal
frame number. The high-level `VideoPlayer::seek` jumps through the native media
index, then skips only local keyframe preroll until the requested timestamp.
Pre-target samples are discarded **before** costly pixel conversion/resizing.
`VideoPlayer::seek_fast` instead returns the first frame at a nearby seek point,
which is ideal for scrubbing and thumbnail previews. Low-level `VideoDecoder::seek_fast` and `seek_with_mode` offer the same
seek modes; `seek_frame` and `seek_preview_frame` additionally return the
corresponding decoded image without scanning the beginning of the file.

On Windows, the Source Reader uses advanced video processing for RGB output,
then attempts native output resizing. If the negotiated format cannot resize,
`mata-video` falls back to fast nearest-neighbour resizing in Rust. On Linux,
GStreamer `videoscale` resizes frames before they reach the app sink. Neither
approach guarantees that the *compressed stream itself* is decoded at reduced
resolution: some codecs always decode internally at their original dimensions.

## Buffered player (video only)

```rust
use mata_video::{VideoPlayer, PlaybackOptions, PlaybackState, SeekMode};
use std::time::Duration;

let options = PlaybackOptions {
    prebuffer_frames: 12,
    min_buffer_frames: 3,
    max_buffer_frames: 32,
    max_buffer_bytes: 256 * 1024 * 1024,
    max_cached_bytes: 48 * 1024 * 1024,
    adaptive_buffering: true,
    drop_late_frames: true,
    output_resolution: Some((1280, 720)),
    ..Default::default()
};
let player = VideoPlayer::open_with_options("movie.mp4", options)?;
println!("duration: {:?}", player.duration());
player.play()?;
// Call from your redraw/event loop, at your desired UI refresh rate:
if let Some(frame) = player.poll_frame() { /* show frame.pixels */ }
let s = player.stats();
println!("queued: {} frames, {} bytes; dropped: {}; {} underruns",
    s.buffered_frames, s.buffered_bytes, s.dropped_frames, s.buffer_underruns);

player.pause()?;
let next = player.step_forward()?;   // exactly one next frame while paused
player.seek_fast(Duration::from_secs(20))?; // quick keyframe preview
let preview = player.step_forward()?;      // while paused
player.seek(Duration::from_secs(20))?;     // accurate frame for playback
// Equivalent: player.seek_with_mode(Duration::from_secs(20), SeekMode::Accurate)?;
player.set_playback_rate(1.5)?;
player.set_output_resolution(Some((640, 360)))?;
player.set_prebuffer_frames(6)?;
player.set_max_buffer_bytes(128 * 1024 * 1024)?;
player.set_max_cached_bytes(32 * 1024 * 1024)?;
player.set_adaptive_buffering(false)?;
player.set_frame_dropping(false)?;
# Ok::<(), mata_video::VideoError>(())
```

### Important behavior

- The native decoder lives on a dedicated thread (including all Windows COM
  calls). `poll_frame()` is nonblocking and driven by the video presentation
  clock; call it regularly from your UI thread. The player initially
  prebuffers before starting the clock. Pausing freezes that clock.
- Seeking flushes old frames and uses GStreamer or Media Foundation's native
  demuxer/index seek. Normal precise seeking decodes only the GOP around the
  target (when the file is seekable/indexed), skipping pixel copies for frames
  before the requested PTS. `seek_fast()` skips the exact-PTS stage.
- **Limits:** an unindexed file, very long GOP, or a source that does not
  implement random access may still require additional decoding. Media
  Foundation exposes no separate keyframe flag on IMFSourceReader, so Windows
  fast seeks use its native seek-point behavior; the exact landing time varies.
  A fast seek does not guarantee an exact frame. Do not call it for frame-perfect
  editing. Windows and Linux may differ on what their first frame represents.
- Frame stepping is forward-only and works while paused; it returns a decoded
  frame, not an estimated frame number.
- Late-frame dropping is optional. When enabled, `poll_frame()` drops obsolete
  queued frames and returns the most recent due frame.
- Adaptive buffering expands its target on expensive decode calls or underruns
  and shrinks after sustained fast decoding. Both per-frame count and actual
  queued pixel-allocation bytes are capped. Decoder-internal/GPU surfaces are
  *not* counted.
- `VideoPlayer::stats()` exposes queue frames, bytes and presentation duration,
  target queue length, cumulative decode/display/drop/seek-discard counts,
  underruns, peak bytes, rolling average decode time, estimated decode fps,
  position, status and end-of-stream.
- This version does **not** implement audio decoding, audio/video synchronization
  or volume control. `set_playback_rate()` controls the video presentation clock
  only. Integrate `mata-audio` / `mata-playback` separately for audio.
- No new Linux Rust dependencies were added. Windows uses the existing
  `windows` crate with additional Win32 features enabled.

## Nonblocking scrubbing (new)

`request_seek_fast` and `request_seek` return a monotonic request ID after only
updating shared state and signaling the decoder worker. Multiple pending requests
are **coalesced**. An already executing native decoder operation cannot be
interrupted, but its obsolete frame is never published. The UI remains free.

```rust
use mata_video::{VideoPlayer, PlaybackOptions};
use std::time::Duration;

let player = VideoPlayer::open_with_options("movie.mp4", PlaybackOptions {
    max_cached_bytes: 48 * 1024 * 1024,
    output_resolution: Some((480, 270)), // scrubbing uses smaller pixels
    ..Default::default()
})?;

// Pause once at the start of dragging (pause() itself is synchronous).
player.pause()?;
// In slider drag handler (nonblocking):
let seek_id = player.request_seek_fast(Duration::from_millis(12_345))?;
// In UI event/redraw handler (nonblocking):
if let Some(done) = player.poll_seek_result() {
    if done.id == seek_id {
        done.result?;  // propagate any decode error
        if let Some(preview) = player.poll_seek_preview_frame() {
            // Display preview.pixels in mata-win.
            let _ = preview;
        }
    }
}
// When drag completes, request a precise timestamp (also nonblocking):
player.request_seek(Duration::from_millis(12_345))?;
# Ok::<(), mata_video::VideoError>(())
```

The player reuses nearby, already decoded pixels from an LRU cache (default
32 MiB, separately capped from the playback buffer). Frames are copied to the
cache because `VideoFrame::pixels` remains a public `Vec<u32>`. This trades a
small amount of memory/copy CPU for repeated-scrub latency. A cached **paused**
preview does not change the underlying native decoder position; playback or
single-step synchronizes it before decoding more video. Cache entries are
invalidated when output resolution changes. `BufferStats` now includes
`cached_frames`, `cached_bytes`, `cache_hits`, `seek_requests`,
`seek_completions`, and `seek_pending`.

### Hardware decoding

- **Windows:** The Source Reader now initializes a D3D11 video-support device,
  registers an `IMFDXGIDeviceManager`, and enables Media Foundation hardware
  transforms. If device initialization is unavailable, it uses the established
  CPU decoder path. Whether a specific codec actually decodes on GPU depends on
  its installed Media Foundation transforms. CPU ARGB output remains necessary.
- **Linux:** Before `uridecodebin` autoplugs a decoder, installed/enabled
  hardware video decoders with nonzero factory ranks are promoted above the
  primary rank. Software decoders remain available as fallback. GStreamer
  factory ranks are **process-global**; this affects other GStreamer pipelines
  in the same process. Plugin availability, supported codecs, output memory
  negotiation, and GPU drivers determine whether decoding is accelerated.
- Hardware decoding may not be faster for short/small clips when GPU-to-CPU
  readback dominates. Set `MATA_VIDEO_DISABLE_GPU=1` before launch to skip
  Windows GPU setup or Linux factory-rank promotion for comparisons. On Linux
  this does not disable plugins that GStreamer would otherwise automatically
  choose; use GStreamer plugin configuration if strict software-only is needed.
  The fastest timeline previews generally also request
  a smaller `output_resolution` or use separately encoded thumbnail media.

### Seek modes and caveats

- Linux fast seek uses `FLUSH | KEY_UNIT | SNAP_NEAREST`. A preview can be
  **after** the requested timestamp. Precise seeking uses `FLUSH | ACCURATE`.
- Windows uses the Source Reader's `SetCurrentPosition` seek index; there is no
  equivalent dedicated keyframe flag. Accurate seek discards nearby preroll
  *before* converting pixels to ARGB.
- Unsupported/unindexed formats and very long GOPs can still have slow seeks.
  An ongoing blocking native decoder operation cannot be interrupted; the
  newer request supersedes its output, not the low-level operation itself.
- The cache stores **decoded frames**, not an on-disk compressed keyframe index.
  Native demuxers remain responsible for locating seek points.
- `poll_seek_preview_frame()` consumes a queued frame only while paused.
  During normal playback use `poll_frame()`.

## Commands

```console
cargo check -p mata-video
cargo test -p mata-video
cargo run --release -p mata-video --example playback -- path/to/movie.mp4
cargo run --release -p mata-video --example async_scrub -- path/to/movie.mp4
```

Linux runtime requirements: GStreamer 1.x development libraries and the
`uridecodebin`, `videoconvert`, `videoscale` and `appsink` plugins.
