# Servo-backed WebView widget

`mata-ui` provides **one** optional WebView implementation, powered by Servo:

| Feature | Backend | Renderer |
| --- | --- | --- |
| `webview-servo` or `webview` (alias) | Servo 0.7.0 | Offscreen software framebuffer |

Only Servo renders web content. The former WebView2/WebKitGTK code and the Boa
HTML renderer are no longer part of the active workspace.

## Build / run

Use the **complete Mata workspace**, including the media and core path dependencies referenced in `ui/mata-ui/Cargo.toml`:

```bash
cargo run -p mata-ui --example browser_servo --features webview-servo
```


Servo is a large dependency and needs a working graphics driver/software OpenGL implementation and its platform build dependencies. It is not part of the default build. Servo 0.7 has its own platform/runtime limitations; using software readback is slower than GPU texture sharing, especially for video, animation, or large browser areas.

## Widget usage

```rust
use mata_ui::widgets::{Anchors, Rect, WebView, WidgetWindow};

fn example(window: &WidgetWindow) -> Result<WebView, Box<dyn std::error::Error>> {
    let page = WebView::new(window, Rect::new(0, 50, 1024, 718))?;
    page.set_anchors(Anchors::ALL);
    page.navigate("https://www.rust-lang.org/")?;
    Ok(page)
}
```

Run `window.poll_events_into(&mut events)` continuously in your UI event loop and call `window.present()` to display invalidated frames. WebView supports `navigate`, `reload`, `go_back`, `go_forward`, `set_bounds`, `set_visible`, style handles, and resize policies. The host translates mouse movement/clicks, wheel motion, keyboard input, and focus to Servo. The backend retains its last image between repaints and clips to the window bounds.

## Implementation details

- A single thread-local Servo engine services all `WebView` widgets created on that UI thread.
- Each webview gets a separate `SoftwareRenderingContext` (not a native GTK/Win32 child).
- `WebViewDelegate::notify_new_frame_ready` invalidates the Mata host. During the next `present()`, the browser paints and its RGBA pixels are blended onto the native Mata framebuffer.
- The existing native webview backend is unaffected when only `webview` is active.
- This implementation currently lacks advanced host integrations such as IME composition, drag-and-drop, popup-window management, accessibility bridging, custom permission prompts, accelerated texture sharing, and complete browser keyboard shortcut/clipboard integration. These require extra embedder delegates and event APIs.

## Verification limits

The supplied `ui.zip` contains `ui/` but not its referenced `../../media` and `../../core` crates. Furthermore, the modification environment does not have a Rust toolchain. The changes were structurally inspected and are **not represented as a successfully compiled or runtime-tested Servo integration**. Build and test the patched `mata-ui` in your full workspace with the command above.
