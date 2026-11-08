//! cargo run --release --example playback -- path/to/video.mp4
//! Run in a terminal. Integrate player.poll_frame() into mata_win's redraw loop.
use std::{env, thread, time::{Duration, Instant}};
use mata_video::{PlaybackOptions, PlaybackState, VideoPlayer};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let filename = env::args().nth(1).ok_or("usage: playback <video-file>")?;
    let opts = PlaybackOptions {
        output_resolution: Some((640, 360)),
        prebuffer_frames: 8,
        max_buffer_bytes: 64 * 1024 * 1024,
        ..PlaybackOptions::default()
    };
    let player = VideoPlayer::open_with_options(filename, opts)?;
    println!("Video: {:?}; duration: {:?}", player.info(), player.duration());
    player.play()?;
    let mut last_print = Instant::now();
    loop {
        if let Some(frame) = player.poll_frame() {
            // Display frame.pixels in mata_win here. These are ARGB32 u32 values.
            let _ = frame;
        }
        if last_print.elapsed() >= Duration::from_secs(1) {
            let s = player.stats();
            println!("t={:?}, queue={}/{} frames ({} bytes), buffered={:?}, decoded={}, shown={}, dropped={}, underruns={}, decoder={:.1}fps",
                s.playback_position, s.buffered_frames, s.target_buffer_frames,
                s.buffered_bytes, s.buffered_duration, s.decoded_frames,
                s.presented_frames, s.dropped_frames, s.buffer_underruns,
                s.estimated_decode_fps);
            last_print = Instant::now();
        }
        if let Some(error) = player.last_error() { return Err(error.into()); }
        if player.state() == PlaybackState::Ended { break; }
        thread::sleep(Duration::from_millis(2));
    }
    Ok(())
}
