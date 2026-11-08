# Cross-platform cursor API

The `mata_win::Window` now provides these methods **only on Windows and Linux** (X11 and Wayland). Android is excluded using `#[cfg(not(target_os = "android"))]`.

```rust
window.set_cursor_visible(false)?; // hide system pointer over window
window.set_cursor_visible(true)?;  // show it again

window.hide_cursor()?;             // convenience alias
window.show_cursor()?;             // convenience alias

let requested = window.cursor_visible(); // requested per-window state
let inside = window.cursor_focused();    // pointer over window, not keyboard focus
```

`set_cursor_visible` returns `Result<(), String>` for native errors. The visibility getter reports the *requested* state, not whether the pointer is visible elsewhere on screen. The methods do not warp, lock, or confine the pointer.

- **Windows:** Uses `WM_SETCURSOR` with per-HWND policy; does not change the process/global ShowCursor counter. The setting only applies to the client area.
- **X11:** Uses a transparent cursor pixmap via core X11, restoring the inherited cursor when shown. Pointer enter/leave events provide focus status.
- **Wayland:** Uses `wl_pointer.set_cursor(None)` to hide the pointer, and a locally owned ARGB cursor surface to show it. The preference is remembered and applied when the pointer re-enters. Wayland does not allow a client to arbitrarily warp the pointer.
- **Android:** No cursor methods.

This API is intended for software-rendered games and apps which show a cursor for menus and hide it during gameplay. It is **not a replacement** for relative-pointer or pointer-constraints protocols. When hidden without locked relative pointer support, the physical cursor may still leave the window and movements may stop being delivered.

## Typical pause/resume handling

```rust
// Call this only when transitioning between gameplay and pause, not every frame:
window.set_cursor_visible(paused)?;
```

Run the standalone demonstration:

```sh
cargo run -p mata-win --example cursor_visibility
```

## Compatibility with older `mata-raycast`

The older game code calls `capture_cursor(true/false)` and `warp_cursor_to_center()`. Those are **not added here** because their semantics cannot be implemented uniformly using these backends' existing APIs. The game must be updated to call `set_cursor_visible(false/true)` and to omit cursor warping. Pointer hiding alone cannot give endless mouse deltas across every compositor.
