# MIVL v0.7.4 — idle CPU / GPU behavior

## Problem

`NativeApp::pump()` previously called `sync()` on every 8 ms iteration, even when the user was not interacting and no frontend messages had arrived. Every sync re-evaluated text expressions, inspected font metrics, calculated the entire layout, built clipping rectangles and repaint signatures, and checked every widget. This needlessly kept the CPU awake. Some native style/invalidation paths can also cause extra GPU work, even though Mata-UI already has a dirty-frame guard.

## Fix

1. `Engine::revision()` is separate from `Engine::generation()` (which only changed when delegate trees were reconstructed). A successful changed property override advances revision. Rust-to-frontend events and async completions also advance it; pure reads and geometry queries never do.
2. `NativeApp::pump()` continues polling input, pending Rust messages and async results, but calls `sync()` only if revision, generation or instantiated widget count changes.
3. The loop polls at 16 ms while dragging, flicking or shortly after input, and falls back to 50 ms after 250 ms of inactivity. It no longer polls at 125 Hz while idle. That is a *maximum 50 ms poll latency* for background events; it is not a blocking OS event wait.
4. `WidgetWindow::present()` already checks its dirty/animation flags and returns without presenting when static. The MIVL loop continues to call this method to keep native animations available, but never requests an unconditional redraw on idle iterations.

## Manual Windows verification

From the Mata workspace root:

```powershell
cargo fmt --all
cargo test -p mivl --all-features
cargo run -p mivl --release  --bin mivl -- mivl/examples/responsive.mivl
```

1. Leave the cursor still and do nothing for 5 seconds. Compare CPU/GPU utilization to v0.7.3. Use a **release** build for meaningful performance measurements.
2. Move the cursor, drag the scrollbar, resize, and let a flick finish. Check input is smooth and the window returns to idle.
3. Test `FrontendHandle::emit`, a completed async command and direct `app.engine.set(...)` updates while the window is idle. Verify these still appear within approximately 50 ms plus processing time.
4. If GPU activity remains high while the MIVL revision is unchanged, profile `WidgetWindow::present()` and any active Mata-ui keyframe animations, video widgets, or the platform compositor; those are separate from the MIVL layout loop.

## Limitations

This environment has no `cargo`/`rustc` and cannot profile Windows GPUs. The changes were source-inspected, not compiled or hardware-benchmarked here. MataWin would need a cross-platform wait/wake event API to avoid periodic idle polling completely. The 50 ms wakeup does not guarantee exact response latency when the system is under load.
