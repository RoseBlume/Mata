# mata-win — native windowing and framebuffers

Small native window creation, input events, cursor visibility and software framebuffer presentation. **No widgets, font/image/audio/video decoders, UI layouts, or UI rendering are included.** Use the sibling `mata-ui` crate for that functionality.

```rust
use mata_win::{Window, Framebuffer, events::WindowEvent};
let mut window = Window::new("MataWin", (800, 600));
let fb = Framebuffer::from_color(800, 600, 0xff101820);
let mut events = Vec::new();
loop {
    window.poll_events_into(&mut events);
    if events.iter().any(|e| matches!(e, WindowEvent::Close)) { break; }
    window.present(&fb);
}
```

Features: `wayland` and `x11` control Linux backends. Windows and Android use their native backends.

```sh
cargo check -p mata-win
cargo run -p mata-win --example simple
```

See `../mata-ui/README.md` for all widgets, media UI, text rendering, styling and examples.

## Optional Windows FPS telemetry

To monitor presented frames without administrator privileges, enable the
`telemetry` Cargo feature when building the application on Windows. For a
Cargo application that depends directly on `mata-win`:

```powershell
cargo build --release --features mata-win/telemetry
```

The Mata Bundler `mata monitor` command enables this feature automatically.
`Window::new` initializes the counter only when `MATA_MONITOR_FPS_FILE` is set
by the monitor. Both `present` and `present_dirty` record successful Direct2D
frame presentations; separate windows contribute to one total presentation rate.
When the feature is disabled (the default), the counter and I/O code are absent
from the binary. No admin rights, ETW, PresentMon or new dependencies are needed.
