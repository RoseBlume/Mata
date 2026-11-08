## 0.7.4 — Idle CPU/GPU optimization

- Removed continuous layout and text-measurement passes when the reactive tree has not changed. The native event pump now checks `Engine::revision()` and only synchronizes changed widgets.
- Added a monotonic property revision separate from delegate generation. Direct Rust setters, event handler assignments, async completions, and frontend messages invalidate the native projection. Repeated identical overrides do not generate work.
- Replaced 8 ms unconditional polling with 16 ms after input / during gestures, switching to 50 ms idle polling. Async commands and Rust frontend messages continue to be checked without native input.
- Preserved Mata-UI's dirty-frame presentation: no GPU framebuffer submit is requested for an unchanged scene. Animated widgets and kinetic scrolling can still request updates.
- Added tests for idle revision stability, changed vs unchanged values, frontend events and idle cadence.

**Limitations:** The runtime still uses a 50 ms cross-platform idle poll because MataWin does not currently expose a portable wait/wake API. This is not a fully event-driven loop. The native window and GPU must still be profiled on target hardware to confirm utilization improvements.

## v0.7.3 — Multiline text rendering and scroll-safe clipping

- Shared UTF-8-safe Rust line-breaking engine with word, word-only, and anywhere modes.
- Native glyph advances used for both text wrapping and intrinsic auto-height.
- MIVL Text now paints into a retained, cropped Mata DrawingArea surface, preserving
  original glyph positioning when partially visible in scrollable viewports.
- Supports `wrapMode`, `maxLines`, `textOverflow: "ellipsis"`, vertical multiline
  alignment, newline preservation, responsive word wrapping, and reactive reflow.
- Added text rendering demo, headless integration tests and native geometry tests.
- No new external dependencies; previous Rust command/event APIs preserved.

## v0.7.1 — Length unit lexer hotfix

- Fixed `em` and `ex` units being mistaken for scientific notation (for example, `0.95em`).
- Preserved numeric exponents such as `1e2px` and `2E-1rem`.
- Fixed child `fontSize: 0.95em` and `rem` to respect the configured root font size.
- Added regression tests for valid units, malformed exponents, and `examples/responsive.mivl`.

# Changelog

## 0.5.0 — MIVL rename and compile-time modules

- Renamed `mata-qml` package and API to `mivl`; `.mqml` examples now use `.mivl`.
- Renamed `mata-qml-macros` to `mivl-macros` and `#[mata_qml::command]` to `#[mivl::command]`.
- Added `mivl-build` (std-only, depends on local `mivl`) to validate MIVL source and generate component wrappers.
- Added `mivl::include_modules!();` and `CompiledWindow` wrapper.
- Added a build-script example demonstrating `MainWindow::new()`, Rust command callbacks, and `run()`.
- Preserved interpreter syntax and runtime behavior; legacy extension still works through dynamic parsing.

# v0.4.0 — Rust command bridge and widget roadmap

- Added `todo.md`, a prioritized backlog of remaining widgets and required infrastructure.
- Added std-only `mivl-macros` path crate; `#[mivl::command]` emits a `register_<fn>` helper.
- Implemented explicit command registration and removal, Rust closure callbacks, handler-only `invoke(...)`, synchronous typed results, unknown/duplicate command errors and strict input checks.
- Implemented assignment from command results, returning values into reactive properties.
- Preserved command registry across virtualized model recomposition.
- Added host/markup example pair, integration tests and `NAMING.md`.
- Limitations: no async invocation, IPC, sandboxed untrusted templates, panic-catching policy, generic methods, or automatic registration. Rust-side effects are not transactional.


## 0.2.0 — Views

- New QML-style `ListView`, `GridView`, `ScrollView`, `Flickable`, `StackView`, and `SwipeView` containers.
- Array literals (`["a", "b"]`) and `Value::List` in the interpreter; `len()` works on arrays.
- `delegate: Type { ... }` for model-backed ListView/GridView; integer-count or primitive-array models, local `index`/`modelData`, dynamic regeneration.
- List/Grid viewport placement, content offsets, computed item counts and dimensions, grid columns/cells, and clipping.
- Stack/Swipe current-page visibility and native previous/next navigation controls.
- Scrollable native views with on-screen arrow controls and `Engine::scroll()` helper.
- Native controls are reused and pooled across model updates rather than recreated on every change.
- New `examples/views.mivl` and headless unit tests for data bindings, layout, clipping, paging, and model changes.

Compatibility: Rust's standard library remains the only dependency of the interpreter; the native adapter continues to depend only on the user's existing `mata-ui` and `mata-win` crates. No new crates have been introduced.

Known limitations: no mouse-wheel event forwarding from the provided `mata-ui` snapshot, no touch flick inertia, no Qt `ListModel` roles, no delegate virtualization, and retained-widget clipping can be imperfect for partly visible text and controls. Cargo build/tests have not been run in this environment because Rust is not installed.

## 0.3.0 - Wheel, touch gestures, virtualized views

- `WidgetWindow` event bridge patch for pointer location, mouse wheel and
  native touches. Optional MataWin Windows `WM_POINTER` patch for touchscreens.
- Native wheel routing to the deepest clipped scrollable viewport.
- Single-finger and desktop pointer panning, swipe navigation for SwipeView,
  inertia with exponential friction and bounded scrolling.
- New virtualized `ListView` and `GridView` materializing visible rows plus
  `cacheBuffer`, up to 512 delegate instances at once. Numeric models are
  compact and can contain up to 1,000,000 entries.
- Added `itemHeight`, `cacheBuffer`, `dragEnabled`, `materialized_range`, and
  absolute `index`/`modelData` indexing across virtualized pages.
- Clamps scroll positions after model shrink and retains reusable Mata widgets.
- Regression tests for 100k-row lists, 100k-cell grids, absolute indices,
  scroll bounds, model updates, and nonuniform-height validation.
- Caveat: source-level validation only; build/run requires a local Rust toolchain.

## v0.6.0 — Rust/MIVL bridge

- Implemented `mivl::register_commands![module::command, command]` for default command registration on new engines and the `in &mut engine;` form for existing engines.
- Added macro-generated command registrar modules while preserving `register_<name>` helpers.
- Added `#[command] async fn` with std-only background future execution and `invoke_async`.
- Delivered async completions and failures to MIVL via root Window handlers and observable properties.
- Added Rust `FrontendHandle`, `EventListener`, listener delivery modes and toggle/reset APIs.
- Preserved frontend listeners and command registries during virtualized model recomposition.
- Fixed malformed parser diagnostic and cast-comparison syntax from the earlier release.

## 0.7.0 — responsive layout, styling and scrollbars

- Lexer/parser understand CSS-inspired dimensions: px, %, vw, vh, vmin,
  vmax, em, rem, ch, ex and pt, including mixed arithmetic.
- `parent.width`/`parent.height` continue to participate in reactive layouts.
- Added `auto`, intrinsic sizes, min/max constraints, padding, margins,
  border padding, anchored and absolute positioning, Row/Column alignment.
- Added text alignment, wrapping-aware size estimates, line height, border
  widths, colors and four-corner radii on Rectangle surfaces.
- Added general `overflow` clipping and scrolling, interactive vertical and
  horizontal scrollbar DrawingAreas, appearance/policy controls and thumb drag.
- Added `examples/responsive.mivl`, `tests/styles.rs`, and `STYLING.md`.
- All v0.6 command bridge, async callbacks, events and inclusion macros retained.

## 0.7.2 — Native text metrics and clipping fix

- Use native Mata FontRenderer glyph advance/line metrics instead of fixed `characters * fontSize * 0.56` estimates for intrinsic text sizing.
- Ensure auto width/height changes propagate to parent Column/Row/container intrinsic sizes after text or font changes.
- Cache font renderer instances and glyph measurements; cap the measured text cache at 1,024 entries.
- Compute glyph-sized native label bounds to honor `textAlign` and `verticalAlign`, rather than centering text in the entire widget rectangle.
- Preserve the std-only parser and fallback approximate layout path; no new Rust dependencies.
- Add layout and alignment regression tests.

Known limitation: Mata's current Text content host accepts a single bounds/clip rectangle, so clipped text at a scrolling viewport boundary can still require a later renderer update to retain its exact off-screen origin. Word wrapping remains approximate and is not yet mapped to Mata's glyph layout.
