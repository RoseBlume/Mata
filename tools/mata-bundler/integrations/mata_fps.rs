//! Optional zero-dependency FPS telemetry for applications launched by `mata monitor`.
//!
//! Copy this file into your app, declare `mod mata_fps;`, then call
//! `mata_fps::frame_presented()` once AFTER successfully presenting a rendered
//! frame to a window (e.g. after a swap/present, not for every game update).
//!
//! When run normally, this helper is inert: it never opens any files unless
//! MATA_MONITOR_FPS_FILE is present in the process environment.
use std::{fs, path::PathBuf, sync::{Mutex, OnceLock}, time::{Duration, Instant}};

struct FrameCounter {
    destination: PathBuf,
    period_started: Instant,
    frames: u64,
}

static COUNTER: OnceLock<Option<Mutex<FrameCounter>>> = OnceLock::new();

/// Record one frame actually presented to the user. Safe to call every frame.
/// Reports measured FPS at most four times a second to avoid per-frame I/O.
pub fn frame_presented() {
    let Some(counter) = COUNTER.get_or_init(|| {
        let path = std::env::var_os("MATA_MONITOR_FPS_FILE")?;
        if path.is_empty() { return None; }
        Some(Mutex::new(FrameCounter {
            destination: PathBuf::from(path),
            period_started: Instant::now(),
            frames: 0,
        }))
    }) else { return };
    let Ok(mut counter) = counter.lock() else { return };
    counter.frames = counter.frames.saturating_add(1);
    let elapsed = counter.period_started.elapsed();
    if elapsed < Duration::from_millis(250) { return; }
    let fps = counter.frames as f64 / elapsed.as_secs_f64();
    counter.frames = 0;
    counter.period_started = Instant::now();
    // Missing telemetry must never stop drawing frames. The monitor detects a
    // brief read/write race by retrying on its next sample.
    let _ = fs::write(&counter.destination, format!("{fps:.4}\n"));
}
