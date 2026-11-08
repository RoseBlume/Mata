# MIVL responsive styling (0.7.0)

MIVL v0.7 adds responsive lengths and a subset of CSS-inspired box/layout
properties without a CSS parser or additional Rust dependencies. These are
MIVL properties, **not** browser CSS; the syntax remains declarative MIVL.

## Units

| Unit | Meaning | Example |
|---|---|---|
| `px` | Logical pixels | `width: 240px` |
| `%` | Percentage of containing width/height; `fontSize` uses parent's font size | `width: 80%` |
| `vw`, `vh` | 1/100 of Window's current width/height | `height: 50vh` |
| `vmin`, `vmax` | 1/100 of smaller/larger viewport dimension | `width: 90vmin` |
| `em` | Current element font size, or parent's when setting `fontSize` | `padding: 1em` |
| `rem` | Root Window font size | `fontSize: 1.2rem` |
| `ch`, `ex` | Approximate glyph measures, 0.56em and 0.5em | `width: 40ch` |
| `pt` | 1pt = 96/72 logical pixels | `height: 12pt` |

`parent.width`, `parent.height`, `root.width`, `window.height` and arbitrary
`id.width`/`id.height` expressions continue to work and update whenever their
source changes. Arithmetic is allowed: `width: 50vw - 1rem`. Keep a space
around `%` for **remainder**, e.g. `11 % 4`; write `50%` for percentage. Units
are resolved during property evaluation; they are not concatenated strings.

A root Window whose own size uses viewport units resolves those units against
an initial 800×600 viewport until the platform supplies an explicit resize.

## Size / box model

- `width`, `height`: numbers, lengths, or `auto`. Text defaults to `auto`.
- `minWidth`, `maxWidth`, `minHeight`, `maxHeight`: size constraints.
- `implicitWidth`, `implicitHeight`: computed intrinsic dimensions, read-only
  unless explicitly overridden.
- `padding`, `paddingLeft`, `paddingRight`, `paddingTop`, `paddingBottom`:
  inner space between content and border. Alias `borderPadding` / `border-padding`.
- `margin`, `marginLeft`, `marginRight`, `marginTop`, `marginBottom`: outer spacing.
- `borderWidth`, `borderColor`, `borderRadius` / `borderRadii`, and individual
  `borderTopLeftRadius`, `borderTopRightRadius`, `borderBottomLeftRadius`,
  `borderBottomRightRadius`. The native Rectangle renderer draws these with a
  Mata software framebuffer; it does not introduce image/rendering crates.
- `textAlign: "left" | "center" | "right"` and
  `verticalAlign: "top" | "center" | "bottom"` align individual lines and their
  multiline block within the original text rectangle. `wrap: true` uses word
  wrapping with long-word fallback; `wrapMode`, `maxLines`, and `textOverflow`
  provide character wrapping and ellipsis options. See `RENDERING.md`.
- `lineHeight`: a unitless multiple of `fontSize` used for intrinsic measurement.

Both camelCase and kebab-case work in declarations (e.g. `text-align: "center"`,
`border-radius: 12px`, `margin-top: 1em`); the stored property/API name is camelCase.

With built-in native rendering, text sizes and line breaks use actual Mata glyph advances;
without it, headless sizes use approximate glyph advances. Parent `auto` sizing is based on descendants,
with row widths/column heights accumulated and other axes taking the maximum.
Content layouts with mutually recursive percentage + `auto` sizing are not
supported and are reported as cyclic bindings instead of guessed values.

## Positioning and alignment

- `position: "relative"` (default), `"absolute"`, `"fixed"`.
- `x`, `y`: relative to the containing box (or offsets within Row/Column).
- `left`, `right`, `top`, `bottom`: pin an item against the content edge.
- `anchors.fill: parent`, `anchors.centerIn: parent`, `anchors.margins` remain
  supported.
- `Row` and `Column` accept `spacing`, `alignItems: "start" | "center" | "end"`,
  `justifyContent: "start" | "center" | "end" | "spaceBetween" |
  "spaceAround" | "spaceEvenly"`.

`fixed` elements use root coordinates, ignoring ancestor scroll offsets, but
this is not full CSS stacking-context behavior.

## Overflow and scrollbars

- `overflow: "visible"` (default), `"hidden"`, `"scroll"`, `"auto"`.
- `overflowX` and `overflowY` also recognize `"hidden"`, `"scroll"`, `"auto"`.
- `contentX`/`contentY`, mouse wheel, pointer/touch panning, and kinetic
  scrolling work on overflow-enabled containers, as with ScrollView/ListView.
- `scrollbarPolicy: "auto" | "always" | "hidden"` controls native tracks.
- `scrollbarWidth` (4–32px), `scrollbarColor`, `scrollbarTrackColor` customize
  the draggable vertical/horizontal bars. `scrollbarArrows: true` restores the
  older up/down button controls.
- `visible_layout` clips scrollable and overflow-hidden descendants to their
  ancestor boxes. Axis-specific overflow currently clips both axes and is not
  equivalent to full browser overflow propagation.

The bars are Mata DrawingArea surfaces and receive mouse/pointer dragging on
native hosts with mouse/pointer events. Touch support depends on the existing
Mata event patches under `patches/` (see `INSTALL_INPUT.md`).

## Example

Run `examples/responsive.mivl` with the existing `mivl` CLI with native rendering
feature. The example includes a rounded card, viewport-relative dimensions,
`parent.width` constraints, auto-sized text, and a scrollable overflow region.

## Known limitations

This isn't a CSS implementation: no flex-wrap, CSS grid tracks, selectors,
shadows, gradients, CSS transitions, `calc(...)` parser, exact glyph shaping,
fully variable-height virtualization, bidirectional RTL text, or high-DPI
font-measurement integration. Headless text layout uses estimated glyph advances; native rendering uses
Mata font measurements. Border rasterization currently applies to
`Rectangle`, not to every native button/input control.

### Text intrinsic measurements (v0.7.2)

For `Text { width: auto; height: auto }`, the native backend now measures glyphs with Mata's `FontRenderer` before resolving layout. This prevents the heading/subtitle cut-off seen in `examples/responsive.mivl` when the estimated width was smaller than the rendered text. The same measurements affect `implicitWidth` and `implicitHeight`, and recompute when text, font size or font family changes. Text with an explicit width remains constrained to that width, and alignment operates within its padded content box. On headless builds, conservative approximate character widths are still used because native fonts are not loaded.

MIVL v0.7.3 adds multiline word wrapping with native advances, line caps and
ellipsis, and preserves off-screen glyph origins with cropped native text
surfaces. Consult `RENDERING.md` for supported semantics and remaining complex
script limitations.
