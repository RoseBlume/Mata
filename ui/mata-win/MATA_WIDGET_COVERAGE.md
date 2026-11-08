# Mata widget catalog coverage

`mata_win::widgets::catalog` keeps Mata-native counterparts for the requested GTK4 widget-class surface. These names are compatibility/API coverage names only; they are not GTK bindings and do not link GTK.

The catalog includes windows/dialog states, application/action controls, boxes, buttons/toggles/checks/switches, calendars, color/file/font controls, column/grid/list/tree views, drawing areas, dropdowns, editable/text widgets, popovers, fixed/flow/grid/overlay containers, frames, header bars, image/picture/SVG widgets, labels/inscriptions, level/progress/range/scale/spin controls, media controls, notebooks, paned/revealer/scrolling widgets, shortcut widgets, stacks, status widgets, native video widgets (optional `video` feature), viewports, and window-control states.

`Button` and `TextField` remain the primary input-handled MataWin software controls.

The catalog now also has native software painting through `WidgetExt::renderable_content()`:
most state-backed catalog types produce `WidgetContent::Painted`, text/images/Box retain their
specialized renderers, `SvgWidget` shares the image renderer, and `DrawingArea` and `GLArea`
show the real surface framebuffer via `WidgetContent::Surface`. The painted catalog surfaces
are **paint-only**. Unlike `Button` and `TextField`, wrapper widgets do not yet dispatch
keyboard/mouse interactions or implement full GTK-style semantics. Numeric and selected
states are snapshots, refreshed when the host re-adds their content. Dialog/popup/chooser
widgets display a sample surface, not a functioning native dialog.

`Audio`/`AudioSource` are Mata extensions for streamed playback.

`WebView` is the only widget allowed to use platform web-engine/toolkit code.
