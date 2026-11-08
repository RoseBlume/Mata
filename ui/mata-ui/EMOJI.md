# Color emoji fallback in mata-ui

Ordinary text continues to be rendered with `mata-fonts`. When a string contains
an emoji code point and `NotoColorEmoji.ttf` is installed on Linux (or in the
Android system font path), `FontRenderer` first looks up a matching glyph in
that font's Unicode cmap. Supported color bitmap glyphs are loaded from the
font's `CBLC`/`CBDT` OpenType tables (PNG glyph image formats 17/18) and decoded
through the **existing** `mata-image` PNG decoder, with no new external crate.
The font is loaded only on the first eligible emoji and shared between windows;
scaled glyphs are cached per renderer. No font files are bundled.

`mata-ui`'s `fonts` feature now enables the PNG decoder; the optional `image`
feature still enables the original image codecs. Text remains selectable only
when `text.set_selectable(true)` has been called.

## Linux installation

With the system Noto Color Emoji font installed, run the selectable example:

```sh
cargo run -p mata-ui --example selectable_text
```

On Arch/Manjaro, the font is distributed in the `noto-fonts-emoji` package. A
missing font is not a fatal error: normal text still renders, but unsupported
emoji may appear as missing-glyph squares.

## Current scope

This fallback renders **individual color bitmap emoji**. Emoji ZWJ ligatures,
regional-indicator flags and skin-tone sequences need grapheme shaping and font
GSUB support to appear as a single combined image. Windows currently uses the
separate native GDI font backend, and Apple's `sbix` format isn't handled by
this CBDT/CBLC reader. Those backends are not claimed to have color emoji
support from this change.
