# Opt-in native text selection and right-click events

Text, Label, TextView, and the other catalog text widgets expose:

```rust
let t = mata_ui::widgets::Text::new("Drag to select this 🌷 text");
t.set_selectable(true); // false by default
window.add_content(mata_ui::widgets::Rect::new(20, 20, 500, 60), &t);

// Shared state (even across widget clones):
let current: Option<String> = t.selected_text();
let span: Option<(usize, usize)> = t.selection_range(); // UTF-8 byte boundaries
t.select_all();
t.clear_selection();
```

The host applies left-mouse drag selection to opted-in text in ordinary content,
retained nested groups, or live catalog text. Selection can span multiple lines,
uses font measurements where `fonts` is enabled, and renders a blue highlight.
`set_text()` clears the range. `Ctrl+A` selects all in the currently selected
read-only widget when no TextField has focus; `Ctrl+C` copies to the clipboard.
`WidgetWindow::selected_text()` and `WidgetWindow::copy_selection()` are also
available to context-menu handlers. A TextField retains its existing independent
editing and selection behavior.

Right-clicking is supported on all widgets regardless of selectability. The host
emits `PointerRightPressed(x, y)`, `PointerRightReleased(x, y)` and
`ContextMenuRequested { widget, x, y }`; `widget` identifies the hit widget
where available. Existing `PointerPressed` and `PointerReleased` stay left-button
only, preserving old button behavior. No context menu is created automatically:
applications decide whether to show a menu, copy selection or perform another
widget-specific action.

Try `cargo run -p mata-ui --example selectable_text` (fonts feature enabled).
