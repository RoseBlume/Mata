# MataWin — complete public widget inventory

Source: `mata-win(6).zip`, checked against `src/widgets/catalog/*.rs`, `src/widgets/controls.rs`, and `src/widgets/webview.rs`. This lists the **110 distinct public widget names**, including **5 aliases**, and excludes non-widget types.

**API completeness vs functionality:** `Button` and `TextField` are the primary `WidgetWindow`-hosted interactive controls. The 50 `simple_widget!` types are thin wrappers around shared widget state, not full GTK implementations. The 6 `container_widget!` types are generic child containers. Text types share the `text_widget!` implementation. Some of the other catalog widgets have their own state and methods, but do not all have full automatic rendering/interaction support.

## Core and host controls (3)

`Widget`, `Button`, `TextField`.

## Buttons and toggles (4)

`ToggleButton`, `CheckButton`, `Switch`, `LinkButton`.

## Containers and layouts (6)

`Box`, `Paned`, `Stack`, `Notebook`, `Revealer`, `ScrolledWindow`.

## Primitive drawing (2)

`DrawingArea`, `GLArea`.

## Images (3)

`Image`, `Picture`, `SvgWidget`.

## Lists, grids and trees (7)

`ListBox`, `ListBoxRow`, `ListView`, `GridView`, `ColumnView`, `TreeView`, `IconView`.

## Values, indicators and selectors (13)

`Range`, `Scale`, `Scrollbar`, `ProgressBar`, `LevelBar`, `SpinButton`, `Spinner`, `DropDown`, `ComboBox`, `ComboBoxText`, `ScaleButton`, `VolumeButton`, `Calendar`.

## Text, labels and entry (11)

`Label`, `Inscription`, `EditableLabel`, `Entry`, `PasswordEntry`, `SearchEntry`, `Text`, `TextView`, `ShortcutLabel`, `ShortcutsShortcut`, `Statusbar`.

## Dialogs and application windows (12)

`AboutDialog`, `Dialog`, `AppChooserDialog`, `ColorChooserDialog`, `FileChooserDialog`, `FontChooserDialog`, `MessageDialog`, `PageSetupUnixDialog`, `PrintUnixDialog`, `ApplicationWindow`, `Assistant`, `ShortcutsWindow`.

## Chooser controls and option buttons (11)

`AppChooserButton`, `AppChooserWidget`, `ColorButton`, `ColorChooserWidget`, `ColorDialogButton`, `FileChooserWidget`, `FontButton`, `FontChooserWidget`, `FontDialogButton`, `EmojiChooser`, `LockButton`.

## Other structural widgets (13)

`ActionBar`, `AspectFrame`, `CenterBox`, `DragIcon`, `FlowBoxChild`, `Frame`, `GraphicsOffload`, `HeaderBar`, `PopoverBin`, `SearchBar`, `Separator`, `Viewport`, `Expander`.

## Navigation and popup widgets (6)

`MenuButton`, `Popover`, `PopoverMenu`, `PopoverMenuBar`, `StackSidebar`, `StackSwitcher`.

## Shortcuts, cells and trees (5)

`ShortcutsGroup`, `ShortcutsSection`, `ListBase`, `CellView`, `TreeExpander`.

## Information and window decoration (3)

`InfoBar`, `WindowControls`, `WindowHandle`.

## Generated container wrappers (6)

`Fixed`, `FlowBox`, `Grid`, `Overlay`, `ShortcutsGroupContainer`, `ShortcutsSectionContainer`.

## Window compatibility alias (1)

`Window`.

## Feature-gated media and web (4)

`Audio`, `MediaControls`, `Video`, `WebView`.

## Alias mappings

- `TreeView` → `ColumnView`

- `IconView` → `GridView`

- `ComboBoxText` → `ComboBox`

- `VolumeButton` → `ScaleButton`

- `Window` → `ApplicationWindow`


## Feature availability

- Default features include `fonts`, `audio`, `image`, `wayland`, and `x11`.

- `Audio` and `MediaControls`: require `audio`.

- `Video`: implemented under `video`; the current public `catalog::Video` re-export is located in the audio-gated `media_widgets` module, so usage currently also requires `audio`.

- `WebView`: requires `webview` (not enabled by default), available on Windows and Linux, not Android.

- `Image`, `Picture`, and `SvgWidget` type names are present even without `image`; image decoding/file loading methods have feature-gated portions.

- Text types remain present without `fonts`; font-based drawing depends on `fonts`.


## Supporting public types (not counted as widgets)

`WidgetWindow` (widget host), `ContainerState` (base container state), `WidgetContent` (renderable nested child content), `ContentGroup` and `ContentDirection` (nested content/layout), `WidgetExt`, `WidgetLike`, `ImageData`, `AudioSource`, `AudioSourceState` (feature gated), and styles, geometry, and event types.

`WidgetWindow` is distinct from the catalog alias `Window`, which aliases `ApplicationWindow`; `mata_win::Window` is the separate platform window type.


## Widget counts

- **110** public widget names, including aliases.

- **105** underlying named widget types when the 5 aliases are not double-counted.

- **50** macro-generated thin `simple_widget!` wrappers.

- **6** macro-generated general-purpose `container_widget!` wrappers.

- **11** macro-generated `text_widget!` text widgets.

- **5** type aliases.
