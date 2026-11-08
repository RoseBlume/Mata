# Window presentation performance (no new dependencies)

The existing `Window::present(&Framebuffer)` and `Window::present_exact(...)`
behaviors remain unchanged: they submit a full frame. Use the **opt-in**
`Window::present_dirty(&Framebuffer, &[DirtyRect])` after updating only a small
region of the existing CPU framebuffer to reduce Windows CPU-to-GPU uploads.

## Example

```rust
// Import DirtyRect from the same `window` module as Window.
// `framebuffer` is the same full-sized CPU buffer across frames.
window.present(&framebuffer); // First draw; initializes the GPU image.

// A text field has changed. Update its pixels in the CPU framebuffer first.
// Do not clear the rest of the framebuffer to an outdated background.
window.present_dirty(
    &framebuffer,
    &[DirtyRect::new(120, 90, 240, 48)],
);

// Nothing changed: no upload or presentation on Windows.
window.present_dirty(&framebuffer, &[]);

// A resize should regenerate a full framebuffer, at the current native
// client size, and submit it with `present` or `present_exact`.
window.present(&framebuffer);
```

### Correctness requirements

- The caller **must** mark every changed source pixel; not only newly occupied
  areas but old positions of moved controls, hover effects, and decorations.
- Coordinate units are source framebuffer pixels, not DPI-independent units.
  Rectangles are clipped to the actual source dimensions. Zero-area regions are
  skipped. No per-region scratch image allocations are made.
- The first Direct2D bitmap upload, and any upload after a source resize or
  device loss, is always full. A window target resize redraws cached pixels.
- Linux and Android now use native dirty region presentation. Wayland uses
  mmap-backed wl_shm buffers with per-buffer damage tracking; X11 supports
  rectangular PutImage and optional MIT-SHM; Android uses native dirty locks.
  See `PLATFORM_OPTIMIZATIONS.md` for implementation details.
- `Window::present()` intentionally remains a complete presentation of the
  full framebuffer on every call to preserve prior semantics. The optimizer
  therefore requires the UI/widget renderer to pass accurate dirty rectangles.
- Drawing into the CPU framebuffer is still the application's responsibility;
  `present_dirty` optimizes presentation, not layout/text rasterization.

### Windows details

- Uploads use `ID2D1Bitmap::CopyFromMemory(dst_rect, src_data, source_pitch)`.
  `source_pitch` is the **full source width × 4**. The source pointer is offset
  to the dirty rectangle's top-left pixel.
- Up to 8 small regions are uploaded separately. Many small regions are
  combined into a bounding rectangle, or the entire bitmap is uploaded when a
  full copy is likely cheaper. The threshold is a heuristic, not a guaranteed
  improvement for every GPU.
- `DrawBitmap` continues drawing the whole GPU bitmap to avoid stale pixels in
  HWND swap-chain buffers; only the upload is partial.
- `EndDraw` errors discard device resources and retry once with a full upload.
- No Windows crate features or third-party dependencies have been added.

## Checking the changes

The source archive contains only the window module, not `Cargo.toml` or the
`Framebuffer` implementation, so compile it inside your existing MataWin crate:

```sh
cargo check -p mata-win --target x86_64-pc-windows-msvc
cargo test -p mata-win --target x86_64-pc-windows-msvc
```

On a host without a compatible Windows linker/runtime, `cargo check` can work
with the target toolchain installed, while actually running Windows tests
requires Windows or a compatible test setup.
