# Nested widgets

MataWin buttons accept retained widget **content** rather than requiring a
hard-coded text label. `Button::new(&window, bounds, content)` accepts:

- `Text` or `Label` (including `&Text`/`&Label`), preserving your own font family,
  font size, foreground color, and later text updates.
- `Image` or `Picture`, including raw ARGB32 content if the `image` feature is off.
- `ContentGroup::row(gap)`, `::column(gap)`, `::overlay()` and the existing
  `catalog::Box` container (`Box::append(&text)` now retains renderable children);
  these can be arbitrarily
  nested and edited using `append`, `clear`, and `set_gap`.
- `&str`/`String` for backwards compatibility (these produce a normal `Text`).

```rust
use mata_win::widgets::{Button, ContentGroup, Image, Text, Rect};

let text = Text::new("Open image");
text.set_font_family("Arial");
text.set_font_size(19.0);

let image = Image::new();
// image.set_file("icon.png")?;   // requires the optional `image` feature
let row = ContentGroup::row(8);
row.append(&image);
row.append(&text);
let button = Button::new(&window, Rect::new(20, 20, 220, 56), row)?;
text.set_text("Updated image"); // retained handle: window becomes dirty
button.set_child(&image);       // replace the entire content at runtime
```

Click events, pressed/hover state, and focus remain on the **button**, even when
the pointer is on its child image or text. Nested groups layout their children
within the parent's computed content rectangle and clip their leaf rendering.
`WidgetWindow::add_content(bounds, content)` renders standalone visual trees;
its roots are visual rather than focusable controls.

Run the included interactive demo:

```sh
cargo run -p mata-win --features fonts --example nested_widgets
```

The initial implementation supports visual child nodes (`Text`, `Label`, `Image`,
`Picture`) and nested layout groups. Controls that need independent keyboard
focus and independent pointer actions inside a parent group are **not** yet
represented by the child-content tree; they remain window-registered controls.
The `fonts` feature must be enabled for glyph rendering; image ARGB32 rendering
is available without the `image` feature.
