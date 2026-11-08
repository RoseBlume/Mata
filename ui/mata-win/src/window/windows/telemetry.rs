//! Optional Windows-only, unprivileged frame-presentation telemetry.
//!
//! Only compiled when both `target_os = "windows"` and Cargo's `telemetry`
//! feature are active. No Win32 profiling API, ETW, or administrator access.
//!
//! A process-wide counter aggregates successful presentations across MataWin
//! windows and publishes a sampled total FPS. The environment variable is read
//! at first window construction; release builds without the feature do not
//! compile any of this code.

use std::{
    fs,
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::{Duration, Instant},
};

const INTERVAL: Duration = Duration::from_millis(250);

struct Counter {
    path: PathBuf,
    interval_start: Instant,
    frames: u64,
}

static COUNTER: OnceLock<Option<Mutex<Counter>>> = OnceLock::new();

/// A lightweight handle to telemetry initialized during Windows window creation.
/// Multiple windows share a process-wide counter and the same output file.
pub struct FpsTelemetry;

impl FpsTelemetry {
    pub(super) fn from_env() -> Option<Self> {
        COUNTER.get_or_init(|| {
            let path = std::env::var_os("MATA_MONITOR_FPS_FILE")?;
            if path.is_empty() { return None; }
            Some(Mutex::new(Counter {
                path: PathBuf::from(path),
                interval_start: Instant::now(),
                frames: 0,
            }))
        }).as_ref().map(|_| Self)
    }

    /// Records only successfully presented frames, not event-loop iterations,
    /// failed Direct2D draws, or clean dirty-region no-ops.
    pub(super) fn frame_presented(&self) {
        let Some(Some(lock)) = COUNTER.get() else { return };
        let Ok(mut counter) = lock.lock() else { return };
        counter.frames = counter.frames.saturating_add(1);
        let elapsed = counter.interval_start.elapsed();
        if elapsed < INTERVAL { return; }
        let fps = counter.frames as f64 / elapsed.as_secs_f64();
        counter.frames = 0;
        counter.interval_start = Instant::now();
        // Best-effort I/O. Monitoring may never disrupt drawing or panic.
        // Readers retry naturally if they see a transient partial write.
        let _ = fs::write(&counter.path, format!("{fps:.4}\n"));
    }
}
