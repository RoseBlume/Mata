# MIVL text rendering (v0.7.3)

This version completes the two text-rendering tasks from v0.7.2: correct
partially-visible text in scrolling viewports and width-aware multiline text.

## Text properties

| Property | Type | Default | Meaning |
|---|---|---|---|
| `wrap` | bool | `false` | Enables word wrapping; oversized words break at character boundaries |
| `wrapMode` | string | `"auto"` | `"auto"` honors `wrap`; `"none"`, `"word"`, `"wordOnly"`, or `"anywhere"` explicitly select behavior |
| `maxLines` | number | `0` | Zero means unlimited; a positive value caps visible lines |
| `textOverflow` | string | `"clip"` | `"ellipsis"` appends `…` if lines are truncated or a no-wrap line exceeds available width |
| `lineHeight` | number | `1.25` | Font-relative line advance; e.g., `1.4` |
| `textAlign` | string | `"left"` | `"left"`, `"center"`, or `"right"`, applied separately to each line |
| `verticalAlign` | string | `"top"` | `"top"`, `"center"`, or `"bottom"`, applied to the complete multiline block |

For wrapping to occur, supply a constrained width (`width: 100%`,
`width: 24em`, or `width: auto` with explicit `maxWidth`). With
`height: auto`, MIVL computes the total line height plus padding; parent
`Column` objects can then auto-size to fit the result. An explicitly fixed
height is instead clipped. Existing units, margins, borders, command macros,
async commands, and virtualized views remain available.

## Native implementation

The Rust-only text layout module determines line breaks using glyph advances
from the same `mata-ui::widgets::FontRenderer` used to paint. Each retained
`Text` is rendered into a transparent `DrawingArea` with Mata's
`FontRenderer::draw_text_styled`. This is a cropped paint surface, not a
smaller text layout rectangle: the original x/y of each line is preserved
when a ScrollView exposes only a fragment of the text.

Surfaces are reused across scroll events, retain cached glyph advances, and
skip drawing rows outside their visible viewport. The std-only interpreter
runs the same line-breaking algorithm using an approximate advance function;
exact sizing requires the native feature and installed fonts.

## Run

```powershell
cargo test -p mivl --no-default-features
cargo test -p mivl --all-features
cargo run -p mivl  --bin mivl -- mivl/examples/text_rendering.mivl
```

## Remaining limitations

This is not a complete browser text engine: it does not implement Unicode
bidirectional reordering, shaping complex scripts, grapheme-cluster-aware
breaks, hyphenation dictionaries, inline rich-text styling, or variable-height
virtualized delegates. Ordinary Unicode text remains UTF-8 safe, but scripts
requiring complex shaping should use a future dedicated Mata text shaper.
TextField and native Button controls still use their own Mata drawing paths;
this change specifically makes the MIVL `Text` component fully viewport aware.
