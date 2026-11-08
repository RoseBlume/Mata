# Mata Monitor — non-administrator FPS tracking

`mata monitor` runs an application, tracks process CPU, GPU, memory and network, and writes independent SVG graphs and change-only CSV time series. **No administrator privileges, Performance Log Users membership, PresentMon download or ETW trace session are required.**

Accurate **per-application** FPS comes from the renderer itself. The monitor passes the target program a `MATA_MONITOR_FPS_FILE` path; a small optional Rust helper records completed frame presentations and publishes measured FPS to that file approximately every 250 ms.

## Commands

```powershell
mata monitor                     # Release profile (default)
mata monitor --debug             # Debug profile
mata monitor --poll-ms 250       # Monitor samples every 250 ms
mata monitor --out reports       # Custom report base directory
mata monitor -p mineral-empire   # Cargo workspace package
mata monitor --fps-file fps.txt  # Optional override for the FPS telemetry file
```

`mata monitor` ends when the application ends, when you type `exit` and press Enter, or when Ctrl+C is received. The application is launched with its own stdin disconnected; this allows the monitor to listen for `exit`. The monitor does **not** restart the process when source files change. It supports Windows and Linux hosts, not Android/cross-target monitoring.

## Windows: automatic MataWin instrumentation (no application changes)

`mata monitor` automatically adds `--features mata-win/telemetry` **only when building
Windows applications**. The `mata-win` crate must be a direct dependency of the
selected Cargo application package, and expose the `telemetry` feature provided
in the included updated `mata-win` crate. If your binary only uses `mata-win`
indirectly via another crate, add it as a direct dependency or forward the
feature explicitly from your app's `Cargo.toml`.

```toml
[dependencies]
mata-win = { path = "../mata-win" }
```

The constructor `Window::new()` automatically starts the counter if
`MATA_MONITOR_FPS_FILE` is present, and both `present()` and `present_dirty()`
record **only successfully completed Direct2D draws**. Multiple Windows MataWin
windows contribute to one aggregated *presentation rate*, not a per-window
framerate. Unchanged/empty dirty-region calls are excluded. It samples at most
four times per second and never opens the telemetry file in a normal execution
without the environment variable.

Normal `mata build` release binaries have **no telemetry code**, unless your
application explicitly enables the Cargo feature. Running `mata monitor` with
its default release profile DOES enable telemetry intentionally; `--debug`
changes optimization level, not this behavior. Since Cargo features are
additive, avoid `--features mata-win/telemetry` in distributed production builds.

### Linux or custom rendering backends

For renderers not using MataWin's Windows native presentation path, copy
[`integrations/mata_fps.rs`](integrations/mata_fps.rs) into the application,
declare `mod mata_fps;`, and call `mata_fps::frame_presented()` after each
successful frame display. An arbitrary uninstrumented renderer cannot be
measured precisely without another process/driver-specific capture source.

There is **no elevation, ETW session or PresentMon dependency**.

## Report files

```text
target/mata/monitor/<package>-<timestamp>/
  index.html
  changes.csv
  fps.svg            fps.csv
  gpu.svg            gpu.csv
  cpu.svg            cpu.csv
  memory.svg         memory.csv
  network_rx.svg     network_rx.csv
  network_tx.svg     network_tx.csv
```

Each metric logs only changed values after rounding to its display precision. An absent measurement is N/A rather than a fabricated zero. FPS becomes zero only when the application previously reported a valid FPS and the file is now stale for more than two seconds (for example, when a static view is no longer being redrawn).

## Measurement scope and accuracy

| Metric | Source | Scope |
|---|---|---|
| FPS | `MATA_MONITOR_FPS_FILE` | Successful MataWin Direct2D presentations on Windows (aggregated across windows); app-provided telemetry elsewhere |
| CPU | Linux `/proc/<pid>/stat`, Windows `Get-Process` | Monitored process, normalized over the logical cores |
| Memory | Linux VmRSS or Windows WorkingSet64 | Resident memory of the monitored process |
| GPU | Linux DRM fdinfo or Windows GPU Engine counters | Monitored process where drivers expose data; otherwise N/A |
| Network | Application telemetry, otherwise host network counters | Marked as per-app or system-wide in the report |

No additional Cargo dependencies are introduced. Windows resource counters currently run through PowerShell, which can limit polling cadence on busy systems. GPU metrics can be absent even with a functioning GPU. None of these counters can be used to determine FPS accurately by themselves.

## Optional per-application network telemetry

The monitored app also receives `MATA_MONITOR_NETWORK_FILE`. The app can write two cumulative byte counters (received, then sent) as whitespace-delimited integers:

```rust
if let Ok(file) = std::env::var("MATA_MONITOR_NETWORK_FILE") {
    let _ = std::fs::write(file, format!("{rx_total_bytes} {tx_total_bytes}"));
}
```

Write periodically, not on every network packet. Without those counters, network graphs cover the **entire host**, not solely the monitored program.
