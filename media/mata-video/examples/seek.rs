//! cargo run --example seek -- video.mp4 12500
//! Compare a fast (seek-point) preview with an accurate timestamp seek.
use std::{env, time::{Duration, Instant}};
use mata_video::{SeekMode, VideoDecoder};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = env::args().skip(1);
    let path = args.next().ok_or("usage: seek <video.mp4> <target-milliseconds>")?;
    let millis: u64 = args.next().ok_or("missing target milliseconds")?.parse()?;
    let target = Duration::from_millis(millis);
    let mut decoder = VideoDecoder::open(path)?;
    for mode in [SeekMode::Keyframe, SeekMode::Accurate] {
        let began = Instant::now();
        let frame = match mode {
            SeekMode::Keyframe => decoder.seek_preview_frame(target)?,
            SeekMode::Accurate => decoder.seek_frame(target)?,
        };
        println!("{mode:?}: first frame {:?}, elapsed {:?}",
            frame.as_ref().map(|frame| frame.timestamp), began.elapsed());
    }
    Ok(())
}
