//! Example: nonblocking, latest-wins scrubbing with a separately bounded cache.
//! cargo run --release --example async_scrub -- path/to/movie.mp4
use mata_video::{PlaybackOptions, VideoPlayer};
use std::{env, thread, time::Duration};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let path = env::args().nth(1).ok_or("usage: async_scrub <video-file>")?;
    let player = VideoPlayer::open_with_options(path, PlaybackOptions {
        output_resolution: Some((480, 270)),
        max_cached_bytes: 48 * 1024 * 1024,
        ..PlaybackOptions::default()
    })?;

    // In a GUI call request_seek_fast when the slider changes. This never
    // blocks on demuxing/decoding. Only the newest outstanding request matters.
    let _obsolete = player.request_seek_fast(Duration::from_secs(15))?;
    let id = player.request_seek_fast(Duration::from_secs(20))?;
    println!("requested seek {id}");
    loop {
        if let Some(done) = player.poll_seek_result() {
            if done.id != id { continue; }
            println!("seek complete: {:?} (cached={})", done.result, done.from_cache);
            done.result?;
            if let Some(frame) = player.poll_seek_preview_frame() {
                println!("preview {}x{} at {:?}", frame.width, frame.height, frame.timestamp);
            }
            break;
        }
        if let Some(error) = player.last_error() { return Err(error.into()); }
        thread::sleep(Duration::from_millis(5));
    }
    let stats = player.stats();
    println!("cached={} frames, {} bytes, {} hits", stats.cached_frames,
             stats.cached_bytes, stats.cache_hits);
    Ok(())
}
