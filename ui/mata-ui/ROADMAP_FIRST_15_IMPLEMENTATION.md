# mata-ui — First 15 roadmap items: implementation and examples

**Baseline:** `Mata_text_selection_emoji_fixed.zip` (1 November 2026).  
**Scope:** `ui/mata-ui` plus keyboard mappings in `ui/mata-win`.  
**Compatibility:** existing `Button::new`, `TextField::new`, `add_content`, and `poll_events_into` stay available. No new third-party crate dependencies were introduced.

> **Build verification:** Rust/Cargo were not installed in the authoring environment. These changes were reviewed and their syntax delimiters checked, but the crate and examples **have not been compiled or run**. The status below distinguishes working implementation code from full platform integration. Never treat an uncompiled item as release-ready.

## How to run

From the `Mata/` directory:

```sh
cargo test -p mata-ui --lib
cargo run -p mata-ui --example roadmap_01_focus
cargo run -p mata-ui --example roadmap_05_context_menu
cargo run -p mata-ui --example roadmap_10_scroll
```

Replace the number/name with any example below. For Linux file pickers install `zenity` or `kdialog` through your distro's package manager.

## Feature-by-feature completion status

| # | Roadmap ID | What the code implements | Example | Remaining work before the original roadmap item is *fully* complete |
|---|---|---|---|---|
| 1 | INT-01 | Tab/Shift+Tab traverses enabled, visible Buttons, TextFields, and live catalog roots; focus outlines. | `roadmap_01_focus` | Configurable tab order, focus scopes, nested window/modal ordering and platform a11y focus sync. |
| 2 | INT-02 | Enter/Space activates the focused Button and catalog control; TextFields keep Enter activation. | `roadmap_02_keyboard` | Per-widget arrow navigation (menu arrow keys included), nested list/radio-group policies and default/cancel buttons. |
| 3 | LAY-01 | `WidgetContent::Interactive` and `ContentGroup::append(&button/field)` resolve child control bounds in retained rows, columns and overlays; child hits use real control registry. | `roadmap_03_nested` | Dedicated layout tree handles, constraints/size negotiation, event bubbling, independent reparenting and scoped focus. |
| 4 | WID-01 | Real, size-aware software painting and model-based activation for CheckButton, ToggleButton, Switch, Scale, Range and progress/level bars. | `roadmap_04_catalog` | Most other catalog widget types still use the fixed-aspect demo painter; convert them individually. |
| 5 | INT-04 | `MenuItem`, `set_context_menu`, clamped `show_context_menu`, Escape/outside-click dismissal, Up/Down/Enter navigation, disabled choices, `MenuItemActivated` event, auto Copy for selected text. | `roadmap_05_context_menu` | Menus do not yet support submenus, separators, accessible menu roles, type-to-search or smooth animation. |
| 6 | TXT-01 | `update_ime_preedit` and `commit_text` accept Unicode composition from a caller; temporary preedit is shown without changing saved text. | `roadmap_06_ime` | **Native IME connections are not implemented**: requires Wayland text-input-v3 / Windows TSF or IMM / X11 IME backend integration, candidate positioning and preedit underlining. |
| 7 | TXT-03 | TextField drag selection, Shift+Arrows, Ctrl+Arrows word movement, Shift+Home/End, Delete, existing clipboard shortcuts; new Home/End/Delete native key mappings. | `roadmap_07_editing` | Undo/redo, select-word-on-double-click, full per-locale word boundaries, password field policy. |
| 8 | VIS-01 | `Theme::dark/light` semantic palettes and `WidgetWindow::set_theme` set standard control tokens. | `roadmap_08_theme` | **Automatic OS color-scheme tracking is not implemented**. Add XDG portal/Windows UISettings/macOS notifications. Theme override currently replaces explicit control colors. |
| 9 | VIS-02 | Keyboard focus outlines on controls/catalog, hover and pressed shading under theme colors, disabled input skipped for focus and activation. | `roadmap_09_states` | Validation/error, loading, reduced-motion and per-control richer animation states. |
| 10 | LAY-02 | `add_scroll_content`, `set_scroll_offset`, wheel scrolling, viewport buffer and clipped embedded-controls, retained nested content. | `roadmap_10_scroll` | Scrollbar, thumb interaction, horizontal axis, touch momentum, viewport virtualization, clip-aware painter to avoid pixel backups. |
| 11 | ACC-01 | `AccessibilityNode` roles/names/descriptions/states, per-widget overrides and in-process `accessibility_snapshot`, automatic basic Button and TextField nodes. | `roadmap_11_accessibility` | **No native screen-reader bridge**: Linux AT-SPI, Windows UIA, and macOS accessibility APIs and live value/action updates still required. |
| 12 | WID-02 | Existing Win32 Open/Save preserved; Linux Open/Save/Folder path dialogs via `zenity` / `kdialog`, extension filters, initial path, cancel detection, and multi-file selection. | `roadmap_12_dialogs` | XDG desktop portal implementation, native Windows folder picker, asynchronous ownership and support for other OSes. |
| 13 | TXT-06 | UTF-8-safe cursor/delete grapheme grouping for common combining marks, ZWJ emoji, regional-indicator flag pairs and skin tones. | `roadmap_13_unicode` | **Full UAX #29 grapheme segmentation, Unicode Bidirectional Algorithm and complex-script shaping are NOT implemented here**; use a full Unicode text engine for those. |
| 14 | API-01 | `Button::builder` and `TextField::builder` with bounds/text/visibility/enabled/style, retaining original constructors. | `roadmap_14_builder` | Nested declarative layout builder, callbacks, validation hints and bindable state. |
| 15 | QA-01 | Headless input-logic unit tests for selection boundaries/focus/menu, framebuffer diff and PPM snapshot helper, event logger demonstration. | `roadmap_15_regression` | Automated native input injection, screenshot golden files, cross-platform CI, normalized DPI/fonts and complete test matrix. |

**Practical conclusion:** The repository contains useful source changes and 15 runnable *example definitions*, but all 15 roadmap items **cannot honestly be marked fully completed**. Native accessibility, native IME, and full shaping require substantial platform-specific implementation. In addition, compilation is currently unverified.

## Public API sketches

### Retained interactive child

```rust
let row = ContentGroup::row(12);
row.append(&button);             // references the registered live Button
row.append(&text_field);         // references the registered live TextField
window.add_content(Rect::new(20, 40, 540, 60), row);
```

Keep the live Button/TextField values alive for as long as they're needed. Their original constructor still registers them with the host, and the content tree supplies layout positions.

### Context menu

```rust
window.set_context_menu(button.id(), vec![
    MenuItem::new(10, "Duplicate"),
    MenuItem::new(20, "Delete").disabled(),
]);
// Look for WidgetEvent::MenuItemActivated { owner, item }.
```

### Scrollable content

```rust
let id = window.add_scroll_content(Rect::new(12, 72, 500, 260), column, 900);
window.set_scroll_offset(id, 150);
```

### Unicode input-method integration surface

```rust
let mut events = Vec::new();
window.update_ime_preedit("かな", &mut events); // temporary display
window.commit_text("漢字", &mut events);      // atomic edit
```

This example simulates input from a native backend. It does **not** claim that Japanese/Korean/Chinese OS input methods work automatically yet.

### Screenshot regression primitives

```rust
let image = window.framebuffer();
qa::save_ppm(&image, "ui-regression.ppm")?;
let changes = qa::compare(&previous, &image);
```

## Manual QA checklist

1. Tab, Shift+Tab, focus ring, activation via Space/Enter and skipping disabled controls.
2. Mouse select inside/outside a TextField; Ctrl+A/C/X/V, Shift+Arrows, Ctrl+Arrows, Home/End/Delete.
3. Right-click over selected text (automatic Copy) and configured menu (disabled items, Escape, keyboard selection, outside click).
4. Nested interactive controls inside row/column content and clipped scroll viewport.
5. Window resize while a menu and scroll view are present; light/dark theme switching.
6. Linux file dialog cancellation, multi-select, file extension filtering; Win32 picker behavior.
7. Combining characters, flags and emoji caret movement; explicitly test complex scripts as known unsupported coverage.
8. `cargo test -p mata-ui --lib` and all 15 examples, in COSMIC Wayland, X11, and Windows where applicable.

## Files added

`src/widgets/interaction.rs`, `src/widgets/qa.rs`, `src/widgets/file_dialog_linux.rs`, `examples/roadmap_01_focus.rs` through `roadmap_15_regression.rs`, `examples/roadmap_support/mod.rs`.

**Core files modified:** `src/widgets/{window.rs,content.rs,controls.rs,events.rs,render.rs,mod.rs,catalog/buttons.rs}` and `../mata-win` key enum/keyboard mappings.
