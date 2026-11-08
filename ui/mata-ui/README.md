# mata-ui — widgets and display

Houses the original MataWin `widgets` implementation, now separated from the
low-level `mata-win` crate. A UI window is composed over a `mata_win::Window`.
The raster UI targets `mata_win::Framebuffer`.

```toml
[dependencies]
mata-win = { path = "../mata-win" }
mata-ui = { path = "../mata-ui", default-features = false, features = ["fonts"] }
```

```rust
use mata_win::Window;
use mata_ui::widgets::{WidgetWindow, Rect, Text};
let host = WidgetWindow::from_window(Window::new("Mata UI", (800, 600)))
    .expect("create UI host");
let title = Text::new("Hello from mata-ui");
// Use host.add_content(...), Button::new(...), or custom framebuffers.
```

`mata_ui::widgets::*` retains the original widget module API, while
`mata_ui::{Window, Framebuffer, events}` reexports *the same native types* for
convenience. No reverse dependency exists from mata-win to mata-ui.

Features: `fonts`, `image`, `audio`, `video`, `webview` enable optional
widgets and integrations; `wayland` and `x11` forward to mata-win backends.
Android can use software widgets, while `webview` remains desktop-only.

```sh
cargo check -p mata-ui --no-default-features --features fonts,wayland,x11
cargo run -p mata-ui --example nested_widgets --features fonts
```

Existing examples are under `examples/`. Obsolete pre-widget demos that
refer to removed legacy engine APIs are retained under `examples/legacy/`,
but intentionally excluded from Cargo example discovery.

## Internationalization (no external i18n dependencies)

Enable the optional `i18n` feature to use the standalone `mata-i18n` crate:

```toml
mata-ui = { path = "../mata-ui", features = ["i18n"] }
```

```rust
use mata_ui::i18n::{I18n, args};
let translations = I18n::new("en-US").unwrap();
translations.add_resource("en", "hello = Hello, {name}!").unwrap();
let caption = translations.tr_with("hello", &args! { "name" => "Mata" });
```

Run `cargo run --example i18n_switch --features i18n` from `mata-ui/` to see UI buttons switch between English and French. The `mata-i18n` crate depends only on the Rust standard library; it does not pull in a serialization or ICU implementation.
