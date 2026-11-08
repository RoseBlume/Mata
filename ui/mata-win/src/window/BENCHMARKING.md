# Benchmarking the native window backends

The uploaded archive contains only the window module, not the containing Cargo
crate. Execute these measurements after integrating the module into your full
MataWin workspace, in `--release` builds on the target device.

## Comparisons

Run each workload with `present()` and with `present_dirty()` using identical
pixel data, client dimensions, and animation schedules:

1. **Idle:** 10 seconds with no damage. Expected: no frame uploads after the
   initial frame when using `present_dirty(&fb, &[])`.
2. **Small update:** change a 100×40 rectangle in a 1920×1080 framebuffer.
   Count CPU rasterization time and presentation time separately.
3. **Animation:** change a moving 200×200 region, including its old position.
   On Wayland, optionally start each animation frame only when `frame_ready()`
   becomes true; continue processing input/events continuously.
4. **Full frame:** upload an entire framebuffer at 1280×720 and 1920×1080.
5. **Resize:** alternate target dimensions and check the first post-resize frame
   is fully correct, with no scale/smear artifacts.
6. **Surface lifecycle:** hide/show, compositor reconnection if available,
   Android rotation and Activity surface teardown/restoration.

Use `std::time::Instant` around your application rasterizer and around native
`present()`/`present_dirty()` calls. For reliable comparisons, report median
and 95th-percentile frame time, CPU utilization, and memory use. `EndDraw`,
`wl_surface.commit`, and X11 flush do not necessarily measure final display
latency; use compositor/display tooling when relevant.

No benchmark results are claimed by this source-only patch.
