# MIVL — remaining widgets and framework backlog

> Target: a cross-platform, Rust-first declarative **markup** UI toolkit powered only by Rust's standard library and existing Mata crates.  This is an **implementation backlog**, not a statement that the corresponding native Mata widgets already exist.  Qt/QML-compatible names are design inspirations, not a claim of compatibility.  As of v0.6.0, most entries below remain unchecked.  "Widget complete" means the parser, reactive properties, layout, renderer, events, keyboard/touch, accessibility, examples and tests all work (not merely that its name parses).

## Currently supported baseline (not TODO)

- [x] `Window`, `Item`, `Rectangle`, `Text`, `Button`, `TextField` — basic rendering/interaction; styling is limited.
- [x] `Row`, `Column` — basic flow layouts.
- [x] `ScrollView`, `Flickable`, `ListView`, `GridView`, `StackView`, `SwipeView` — initial implementations, subject to the improvements below.
- [x] Mouse wheel, pointer/touch drags, inertia, and bounded virtualization for fixed-size rows/cells with the supplied native input patches. Platform testing is still pending.
- [x] Named Rust commands via `Engine::register_command` and std-only `#[mivl::command]`; synchronous `invoke` within handlers, typed values and error propagation.

## P0 — Foundation: finish before adding many widgets

- [ ] Central widget/component registry instead of a hardcoded `supported_type` match; public extension API for native Mata widgets.
- [ ] Component lifecycle: creation, mounting, unmounting, disposal, focus changes and destruction of retained native resources.
- [ ] Unique, stable keyed component identities so list recycling does not confuse input callbacks or editable state.
- [ ] True hierarchical clipping/scissor regions and correct z-index, opacity, hover and hit-testing for overlapping controls.
- [ ] Full two-way bindings with change notifications, computed-binding dependency graph, and batching of updates (avoid full document scans).
- [ ] Layout invalidation and partial redraw of dirty regions; avoid full model cloning on unrelated updates.
- [ ] Style system: reusable styles, theme tokens, hover/pressed/focused/disabled states, and per-widget overrides.
- [ ] Explicit `focus`, `activeFocus`, `tabIndex`, `focusPolicy`, focus rings and predictable keyboard traversal.
- [ ] Consistent device-independent pixels, per-monitor DPI, text scaling and font fallback through `mata-fonts`.
- [ ] Correct nested scrolling, overscroll, parent/child gesture arbitration, mouse capture and touch cancellation on all supported platforms.
- [ ] Wayland/X11 multitouch-contact forwarding and validation on Windows/Android/Linux.
- [ ] Wheel acceleration, smooth scrolling and optional inertia/overscroll snapping with reduced-motion support.
- [ ] Variable-height `ListView` rows (prefix sums/extent tree), variable-width cells, and dynamic layout while virtualized.
- [ ] `GridView` horizontal flow, RTL flow, variable cell sizes, selection, keyboard navigation and efficient viewport invalidation.
- [ ] Delegates with object roles, `required property`, nested views and independent per-row state.
- [ ] Stable view selection while sorting/filtering/mutating models; reuse keyed delegates based on data ID.
- [ ] Accessible names/roles/states, keyboard-only usage, screen-reader announcements and platform accessibility bridges.
- [ ] Consistent event bubbling/capture, pointer enter/leave and modifier keys.
- [ ] Window resize constraints, drag-to-resize correctness, safe input coordinates and minimum widget size constraints.
- [ ] Automated native screenshot and interactive integration tests on Windows, Wayland, X11 and Android.

## P1 — Standard interactive controls

- [ ] `Label` — semantically associated control label.
- [ ] `TextArea` — multiline editor, scrolling, selection and input method.
- [ ] `PasswordField` — masked entry, reveal toggle and safe clipboard policy.
- [ ] `RichText` — runs, links, inline styling and selectable rich content.
- [ ] `TextEdit` / `RichTextEdit` — rich document editing and undo/redo.
- [ ] `CheckBox` — checked/unchecked/indeterminate state.
- [ ] `RadioButton` and `RadioGroup` — exclusive grouping and keyboard selection.
- [ ] `Switch` / `ToggleButton` — boolean control with touch semantics.
- [ ] `ComboBox` / `Select` — single-select popup list, filtering and keyboard navigation.
- [ ] `MultiSelect` — multiple choices, removable tokens and accessible summary.
- [ ] `SpinBox` / `NumberField` — numeric min/max/step/increments.
- [ ] `DoubleSpinBox` / `DecimalField` — precision, locale formatting and clamping.
- [ ] `Slider` — pointer/keyboard input and snapping.
- [ ] `RangeSlider` — two handles and constrained bounds.
- [ ] `ProgressBar` — determinate/indeterminate values.
- [ ] `BusyIndicator` / `Spinner` — animated loading state.
- [ ] `Dial` / `Knob` — circular value adjustment.
- [ ] `ScrollBar` — thumb/track/page controls, linked with views.
- [ ] `ToolButton` / `IconButton` — compact button with tooltip and icon.
- [ ] `LinkButton` / `Hyperlink` — keyboard-activated links with explicit navigation callbacks.
- [ ] `SegmentedButton` / `SegmentedControl` — exclusive or multiselect segments.
- [ ] `SearchField` — clear, submit and suggestions.
- [ ] `Rating` — star-like value selection.
- [ ] `ColorPicker` / `ColorField` — RGBA entry and preview.
- [ ] `DatePicker` / `Calendar` — locale-aware date selection.
- [ ] `TimePicker` — time selection, 12/24 h display.
- [ ] `DateTimePicker` — combined date/time editor.
- [ ] `FilePicker` / `FolderPicker` — native file-system dialog abstraction.
- [ ] `KeySequenceInput` — shortcut capture.
- [ ] `EditableLabel` — edit-on-demand text.
- [ ] `Badge` / `Chip` / `Tag` — compact label and optional remove action.
- [ ] `Separator` / `Divider` — horizontal/vertical rules.
- [ ] `ToolTip` — delay, pointer, keyboard and screen-reader support.
- [ ] `Toast` / `Snackbar` — queued nonmodal notifications.
- [ ] `Banner` / `InlineAlert` — informational/error/success messaging.

## P1 — Layout, grouping, navigation and windows

- [ ] `RowLayout`, `ColumnLayout`, `GridLayout` — stretch, align, preferred/min/max size, fill.
- [ ] `Flow` / `FlowLayout` — wrapping children and responsive sizing.
- [ ] `Grid` — static positioned grid (distinct from model-backed GridView).
- [ ] `WrapPanel`, `DockPanel`, `SplitView` / `Splitter` — docked and resizable panes.
- [ ] `StackLayout` and `OverlayLayout` — stacked composition and aligned overlay.
- [ ] `Spacer` — stretchable space, equal spacing and alignment.
- [ ] `Frame`, `GroupBox`, `Panel`, `Card` — structural containers and borders.
- [ ] `Page` and `PageStack` — navigation lifecycle and back behavior.
- [ ] `TabBar`, `TabButton`, `TabView` — tabs, close, reorder and keyboard shortcuts.
- [ ] `ToolBar`, `StatusBar`, `MenuBar`, `NavigationBar` — application chrome.
- [ ] `Menu`, `MenuItem`, `MenuSeparator`, `MenuGroup`, `SubMenu` — nested menus, accelerators and submenus.
- [ ] `ContextMenu` — right-click/long-press menu and placement.
- [ ] `Drawer` / `SidePanel` / `NavigationDrawer` — modal and nonmodal side sheets.
- [ ] `Popup` / `Popover` — anchored overlays, focus trapping and dismissal.
- [ ] `Dialog`, `MessageDialog`, `ConfirmationDialog`, `InputDialog` — modal flows and platform accessibility.
- [ ] `AboutDialog`, `ProgressDialog`, `SettingsDialog` — standardized applications dialogs.
- [ ] `Wizard` / `Stepper` — multi-step flow and validation.
- [ ] `Breadcrumbs` — nested route navigation.
- [ ] `Pagination` — page selection for models and remote results.
- [ ] `TreeNavigation` — collapsible navigation sidebar.
- [ ] `MDIWindow` / `SubWindow` / `FloatingPanel` — multi-document internal windows.
- [ ] `ApplicationWindow`, `PopupWindow`, `DialogWindow` — multiwindow lifecycles and transient parent ownership.
- [ ] `SystemTrayIcon`, `Notification` — platform host integration and permissions.

## P1–P2 — Views, collections and data visualization

- [ ] `Repeater` — instantiate arbitrary repeated components without a viewport.
- [ ] `ListModel`, `ListElement`, `SortFilterModel` — role-based models with inserts/updates/removals.
- [ ] `TreeView`, `TreeModel` — virtualized hierarchy, expand/collapse and keyboard focus.
- [ ] `TableView`, `DataGrid` — two-axis virtualization, sortable headers and resizable columns.
- [ ] `TableHeader`, `TableColumn`, `TableRow` — explicit column configuration and accessibility.
- [ ] `TreeTableView` — hierarchical table with frozen columns.
- [ ] `ListBox` and `SelectionModel` — selectable list with multi-selection.
- [ ] `Carousel` — animated or stepped paging (separate from SwipeView).
- [ ] `MasonryView` / `StaggeredGridView` — variable-height image/product tiles.
- [ ] `VirtualFlow` — virtualized wrapping layouts and recycled delegates.
- [ ] `KanbanBoard` — draggable columns/cards, auto-scroll.
- [ ] `Timeline` — time-domain scroll/zoom and selection.
- [ ] `ChartView`, `LineChart`, `BarChart`, `AreaChart`, `PieChart`, `ScatterPlot` — data series, axes, legend and accessibility.
- [ ] `PlotAxis`, `Legend`, `ChartTooltip` — chart infrastructure.
- [ ] `DataFilter`, `SearchResults` — composable model filters and pagination.

## P2 — Graphics, media, and document-oriented widgets

- [ ] `Image` — decoding/rendering through `mata-image`, aspect-fill/fit and placeholder.
- [ ] `AnimatedImage` — frame scheduling and image fallback through `mata-image`.
- [ ] `SvgImage` / `VectorImage` — only after a permitted Mata vector decoder exists.
- [ ] `Icon` — theme glyphs and scalable icon assets.
- [ ] `Canvas` / `DrawingArea` — safe drawing commands with retained render layers.
- [ ] `Shape`, `Path`, `Line`, `Ellipse`, `Polygon`, `Arc` — geometry primitives.
- [ ] `Gradient` / `Brush` / `Shadow` / `Border` — composable paints and effects.
- [ ] `Mask` / `ClipPath` / `Transform` — clipping and transforms.
- [ ] `Video` / `VideoPlayer` — feature-gated `mata-video` backend and media controls.
- [ ] `AudioPlayer`, `Waveform`, `SpectrumVisualizer` — feature-gated `mata-audio`.
- [ ] `CameraPreview` / `MediaCapture` — permission-aware video input adapter.
- [ ] `WebView` — feature-gated platform-backed WebView with permission and navigation policy.
- [ ] `WebTabBar`, `AddressBar` — browser-style tabs and URL handling.
- [ ] `CodeEditor` — syntax highlighting, line numbers, folding and search.
- [ ] `MarkdownView` / `MarkdownEditor` — plain-text syntax and rendered preview.
- [ ] `DocumentView`, `PageView`, `PrintPreview` — paged document layout.
- [ ] `EquationView`, `SymbolPalette`, `MathEditor` — math/text editing.
- [ ] `HeaderFooterEditor`, `PageBreak`, `CoverPage`, `PageNumber` — document authoring.
- [ ] `TableEditor`, `TableCellEditor`, `Ribbon`, `RibbonTab`, `RibbonGroup` — productivity-style UI.
- [ ] `DrawingSurface`, `PenTool`, `ShapeTool` — stylus and mouse drawing.
- [ ] `FileBrowser`, `DirectoryTree`, `ThumbnailView`, `PropertyInspector` — common editor file/tooling views.
- [ ] `MoleculeView`, `ProteinView`, `CellView` — optional adapters to existing Mata visualization projects, not built into the core runtime.

## P2 — Interaction, animation and visual polish

- [ ] `MouseArea`, `PointerArea`, `HoverHandler`, `DragHandler`, `TapHandler`, `PinchHandler` — composable input event capture.
- [ ] `DropArea` / `DragSource` — cross-control drag and drop.
- [ ] `GestureArea` — combined touch/pointer arbitration and multi-finger gestures.
- [ ] `Shortcut`, `Action`, `ActionGroup` — keyboard, menus and command bindings.
- [ ] `Timer` — ticking, lifecycle pause and app foreground/background state.
- [ ] `Animation`, `NumberAnimation`, `ColorAnimation`, `PropertyAnimation` — time-based property interpolation.
- [ ] `ParallelAnimation`, `SequentialAnimation`, `PauseAnimation`, `SpringAnimation` — sequencing and physics.
- [ ] `Behavior`, `Transition`, `State`, `StateGroup` — declarative visual state changes.
- [ ] `ShaderEffect` — only via a permitted Mata GPU interface; graceful CPU fallback.
- [ ] `FocusScope` — local focus groups.
- [ ] `Accessible` attached properties — cross-platform screen-reader metadata.
- [ ] `Clipboard` adapter — explicit text/image copy/paste support.
- [ ] `UndoGroup`, `UndoAction` — shared editing history and event routing.

## P2 — Markup-language and command-bridge capabilities

- [ ] `Component` declarations and named reusable custom components (single-file and imports).
- [ ] `Loader`, `ComponentLoader` and lazily mounted nodes.
- [ ] `Connections` for subscribing to named signals outside a child hierarchy.
- [ ] `PropertyAlias`, `PropertyGroup` and structured object/map values (including role maps).
- [ ] Namespaces, modules, imports and versioned component registries.
- [ ] Custom methods/functions in markup, a safe expression subset and function definitions.
- [ ] Better parse errors, source spans and native stack traces for invalid event handlers.
- [ ] Tooling: formatting, syntax highlighter, autocomplete and language server.
- [ ] Typed enum/flags/object values, localization bindings and `mata-i18n` integration.
- [ ] Resource URL scheme and asset bundling using existing Mata tooling.
- [ ] Device/system style info and responsive breakpoints/media queries.
- [x] Synchronous `invoke("command", args...)` in event handlers only.
- [x] Attribute-based `#[mivl::command]` for owned typed arguments and typed return values, without `syn` or `quote`.
- [x] Explicit command registration, unknown/duplicate command errors, argument validation and basic Result propagation.
- [x] Async `#[mivl::command]` functions and `invoke_async()` with request IDs, std-only execution and completion handlers.
- [ ] Native `await` expressions/promises, progress notifications and cancellation of in-flight async commands.
- [x] Thread-safe Rust → MIVL event emission and UI-thread event/completion delivery, with a 128-concurrent-worker cap.
- [ ] Configurable pooled worker queue, bounded event queue/backpressure, cancellation and host-command state handles.
- [ ] A structured `InvokeError` object with error codes, retryability and source locations.
- [ ] Per-command permissions/capabilities, origin policies and sandboxing for untrusted `.mivl` documents.
- [ ] Named command arguments, strongly typed arrays/maps and structs with explicit serializers.
- [x] `mivl::register_commands![module::function, function]` for current-thread defaults and an explicit-engine variant.
- [ ] Method commands, alternate exported names, command namespaces and plugin loading.
- [x] `EventListener` with named Rust events, `eventData`, one-shot, counted and perpetual listeners, enable/disable and reset.
- [ ] Event payload schema validation, cross-window routing and event origin policies.
- [ ] Command tracing, rate limiting, telemetry hooks and cancellation on widget destruction.
- [ ] Input and output size caps, per-command timeouts and panic isolation policies.
- [ ] Configurable application lifecycle events and native platform callbacks.

## P3 — Verification, performance and release

- [ ] Windows x64/arm64, Linux Wayland/X11 x64/arm64, Android builds and CI matrices.
- [ ] Benchmark 1M-row virtual ListView with selection/scrolling and a changing model.
- [ ] Accessibility snapshot tests for every interactive widget.
- [ ] Stress fuzz tests for lexer/parser, AST size/depth, event dispatch and command inputs.
- [ ] Golden/screenshot tests for every supported widget at DPI 100/150/200%.
- [ ] App lifecycle and no-memory-leak tests for repeated view/page creation.
- [ ] Cross-platform visual theme parity and dark/high-contrast modes.
- [ ] Widget coverage dashboard: track parse support, layout, painting, events, semantics and tests separately.
- [ ] Versioned `.mivl` schema, migration/lint tooling and stable public API policy.

## Definition of done for each new widget

1. Syntax and property schema are documented and reject unknown/invalid values with helpful errors.
2. State bindings update reactively; two-way controls preserve cursor/selection and don't create feedback loops.
3. Rendering, clipping, focus, keyboard, wheel/touch and accessibility work correctly.
4. All native retained resources are recycled/deregistered appropriately, including virtualization where needed.
5. Demo `.mivl`, unit tests, native integration tests and platform-specific edge cases exist.

**Recommended order:** P0 core and controls (`CheckBox`, `Slider`, `ComboBox`, `TextArea`, `ScrollBar`) → menus/dialogs/navigation → two-axis tables/tree views → media/document widgets → animation and advanced custom components. A fully functioning command bridge is part of P0/P2, not a substitute for widget APIs.

## Embedded modules and Rust generation

- [x] Rename crate, docs, and sample extension from MQML to MIVL
- [x] Expose `mivl::include_modules!();` with generated component types
- [x] Build-time syntax and runtime-structure checks via `mivl-build`
- [x] Build-time rebuild hints (`cargo:rerun-if-changed`)
- [x] Multiple component files with deterministic generated names
- [ ] Generate strongly typed property getters and setters
- [ ] Generate named callback interfaces from MIVL declarations
- [ ] Static dependency analysis and incremental component compilation
- [ ] Dedicated static type checking across component imports
- [ ] Component-level imports and reusable view declarations
