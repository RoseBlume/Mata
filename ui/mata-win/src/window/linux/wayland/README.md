# Wayland backend

This folder is a Wayland equivalent of the supplied Windows backend. It uses the
current user's Wayland session socket for window management and input, and
`wl_shm` shared-memory buffers for software-framebuffer presentation.

No `sudo` or root access is required. The backend does not read `/dev/input` and
does not open DRM/KMS device nodes. Keyboard and mouse events arrive from
`wl_seat`, and pixels are sent to the compositor with reusable `wl_buffer`s.

The source framebuffer keeps the size passed to `PlatformWindow::new`. When the
compositor configures the top-level surface to another size, `renderer.rs` updates
a three-buffer mapped shared-memory ring using nearest-neighbor sampling.
Changed regions are tracked per buffer, so a recycled buffer catches up with
all missed changes. If all buffers are still busy, damage is retained for the
next available presentation rather than blocking the application thread.

## Wayland differences from Win32

* Top-level clients cannot choose absolute desktop coordinates, so `reposition`
  is intentionally a no-op.
* The window requests server-side controls through `xdg-decoration` when the
  compositor supports that extension. Otherwise the software renderer adds a
  34-pixel client-side titlebar with close, minimize, maximize/restore, and drag.
  The titlebar occupies *additional* surface space: applications continue to
  receive content-relative pointer positions and content-only resize events.
* `set_decorations(false)` disables client-side controls and asks a supporting
  compositor to omit server-side decorations. The maximize/minimize capability
  settings apply to client-side controls (a compositor owns server controls).
* Per-window icons are normally associated with the application/desktop entry,
  not loaded from an arbitrary path, so `set_icon` is a no-op here.
* `set_resizable(false)` is implemented by making the XDG min/max size equal.
* `hide` unmaps the window by committing a null buffer; `show` remaps on the next
  `present`.

The keyboard mapping in `mod.rs` maps Wayland's Linux evdev key codes directly
to the existing `Key` enum. This avoids a dependency on libxkbcommon and thus
avoids a native build dependency, but character keys follow the standard physical
US/QWERTY positions rather than interpreting arbitrary user keyboard layouts.


## Dependencies

The Wayland implementation retains `wayland-client`, `wayland-protocols`, and the Rust standard library, and uses in-tree `mmap` / `munmap` system FFI to write directly to shared-memory buffers. Backing files are created in `XDG_RUNTIME_DIR`, owned by the current user, and immediately unlinked. No `memmap2`, `tempfile`, `rustix`, `libc`, or `nix` **Rust crate** needs to be added. Call `frame_ready()` to pace optional continuous animations using compositor callbacks.
