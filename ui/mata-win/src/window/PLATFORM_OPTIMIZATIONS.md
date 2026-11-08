# Native presentation optimizations (October 2026)

All code retains the existing `Framebuffer` and `Window::present()` entrypoints. No
additional Rust crate is necessary to compile the default platform paths.

## API

```rust
window.present(&framebuffer); // explicit full frame
window.present_dirty(&framebuffer, &[DirtyRect::new(80, 50, 200, 40)]);
// Before driving an optional continuous animation on Wayland:
if window.frame_ready() { /* render the animation's next frame */ }
```

The dirty coordinates are in **source framebuffer pixels**. The window must have
received a complete frame first. Resize, a compositor buffer with no previous
content, device/surface restoration, and X11 exposure invalidate older content.
Dirty rectangles are clipped, transformed when source != target dimensions,
coalesced, and replaced by full-frame damage when partial work becomes too large.
Calling with empty damage does nothing except when native state requires a redraw.

## Wayland

* Three `wl_shm` buffers are individually mapped with Linux `mmap` (through a
  small in-tree OS FFI shim) instead of using `pwrite` on a staging vector.
* Damage history records up to 48 presentations. When an old `wl_buffer` is
  released and recycled, changes missed since its last presentation are
  reconstructed; a history gap requires full refresh.
* Only changed destination pixels are written, using cached nearest-neighbor
  index maps when necessary.
* `wl_surface.damage_buffer` marks affected buffer regions. A `wl_surface.frame`
  callback updates the `Window::frame_ready()` advisory hint. Interactive input
  still presents even when the callback is outstanding, avoiding lost updates.
* A `wl_buffer.release` callback prevents mutating compositor-owned memory.
  Retired buffers remain allocated until released.

## X11

* `PutImage` fallback packs only modified rows and columns into a reusable
  staging buffer, including nonstandard TrueColor layouts. Requests are split
  by the X server's maximum request length; oversized rows are split into strips.
* X11 `Expose` and size changes mark the next frame as fully damaged.
* Optional **MIT-SHM** supports mapped triple-buffered local X11 transfers with
  completion events and per-buffer change history. Remote X servers and servers
  without fd-passing support use core `PutImage` instead. Shared buffers are
  never written while outstanding server operations reference them.

### Opting into MIT-SHM without a new dependency

The uploaded archive did not include `Cargo.toml`, so the feature declarations
cannot be modified here. In your existing manifest, retain your existing
`x11rb` dependency and enable its built-in `shm` feature; then add your own
crate feature to enable the optional code:

```toml
[features]
x11-shm = ["x11rb/shm"]
```

For example, an existing `x11rb = "0.14"` can remain unchanged because the
feature linkage above automatically enables its `shm` feature. Activate with
`cargo check --features x11,x11-shm` or add `x11-shm` to your default feature
list when local X11 is the common case. No extra crate is introduced.

## Android

* The ANativeWindow format is configured only when its identity or size
  changes, or after lifecycle restoration.
* `lock(Some(&mut bounds))` requests only damaged pixels. The returned bounds
  may grow, so everything inside the returned region is always repainted.
* ARGB to RGBA conversion uses one 32-bit store per pixel. Nearest-neighbor
  scaling runs only if source/native dimensions differ.

## Safety/performance notes

* The Linux FFI uses `mmap` and `munmap`, available without root permission.
  It operates on ordinary private, unlinked 0600 files, and keeps the descriptor
  alive while its mapping is in use.
* For best results the widget renderer should avoid rasterizing unchanged
  widgets and should call `present_dirty` only after it has updated the same
  dirty rectangles in the CPU framebuffer.
* Benchmark idle frames, small input updates, continuous animations, full
  redraws, resizes, and surface restoration on real target systems.
* No manifest was included in this source-only archive; compilation and
  device-level performance testing are still required in the complete crate.
