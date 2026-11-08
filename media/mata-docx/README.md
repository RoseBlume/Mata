> **New font revision:** A native TrueType bytecode VM, OpenType SVG glyph
> reader/restricted renderer, EBDT/EBLC grayscale strikes, CBDT format 19 and
> BGRA support, and raw sbix image APIs have been added. See
> [NATIVE_HINTING_COLOR_STATUS.md](NATIVE_HINTING_COLOR_STATUS.md) for precise
> coverage and outstanding limitations. This source has not been compiled or
> tested, in accordance with the user's request. No FFI was added.

> **Latest source revision:** See [ADVANCED_LAYOUT_COLOR_REVISION.md](ADVANCED_LAYOUT_COLOR_REVISION.md) for variable COLR v1 paints, PNG-backed bitmap fonts, Southeast Asian syllable grouping and per-edge Word table rendering. These additions are source-only and have **not** been compiled or tested. Only the existing local `mata-fonts` crate remains an external dependency; no FFI is used.

> **Regex engine:** Pure Rust (no FFI). Additional PCRE-style constructs are supported, but full PCRE2 compatibility has **not** been verified or achieved. See [PCRE_COMPATIBILITY.md](PCRE_COMPATIBILITY.md).

# Integration update — stages 4–6, `mata-fonts` dependency

**Important:** The current revision requires the existing local Mata `mata-fonts`
crate and is **not** std-only at the `mata-docx` crate boundary. All *new
support modules* use the standard library, plus the explicit `mata-fonts`
font API for rasterization. See [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md)
for what is implemented versus still incomplete.

Expected workspace layout:

```text
Mata/
├── core/mata-docx/       # place this source directory here
└── media/mata-fonts/     # your existing font crate
```

The `mata-docx/Cargo.toml` dependency is:

```toml
mata-fonts = { path = "../../media/mata-fonts", default-features = false }
```

If your workspace uses a different layout, adjust that relative path. The
existing `mata-fonts` implementation is not bundled. The implementation pass
was not compiled or tested at your request.

---

# mata-docx

**DOCX/WordprocessingML tooling implemented in Rust `std`, with the existing local `mata-fonts` path dependency.**

> **Status: experimental foundation, not full Zavora DOCX feature parity.**
> The objective is to gradually replicate the capabilities listed by
> [zavora-docx](https://github.com/zavora-ai/zavora-docx) while maintaining a **strict
> `std`-only dependency rule**. Read [FEATURES.md](FEATURES.md) for the honest
> feature-by-feature matrix and [ROADMAP.md](ROADMAP.md) for remaining work.
>
> **Build verification:** `rustc` and `cargo` were not available in the authoring
> environment, so this version has **not** been compilation-tested. Please run
> `cargo test --all-targets` locally before using this version.

## Requirements

- Recent stable Rust (target; build not tested); 2021 edition.
- No third-party Rust crates; no native libraries, subprocesses, font files, or
  office-suite runtime dependencies.
- A ZIP/ZIP64 OPC reader and compressed writer, bounded namespace-aware XML parser, CRC32, DEFLATE encoder/decoder and output writers are included in the source tree.

## Integrating into a workspace

```toml
[dependencies]
mata-docx = { path = "../mata-docx" }
```

Or add the crate as a workspace member:

```toml
[workspace]
members = ["mata-docx", "other-crate"]
```

## Writing DOCX

```rust
use mata_docx::{Document, Length, Alignment};

fn main() -> std::io::Result<()> {
    let mut doc = Document::new();
    doc.properties.title = "Example report".into();
    doc.add_heading("Project Status", 1);
    doc.add_paragraph("Hello from mata-docx.");
    let para = doc.add_paragraph("A sentence with ");
    para.add_run("bold text").bold(true);
    para.add_run(" and italics").italic(true);
    doc.add_paragraph("See also ")
        .add_hyperlink("https://www.rust-lang.org", "Rust");

    let table = doc.add_table(2, 2);
    table.cell_mut(0, 0).unwrap().set_text("Task");
    table.cell_mut(0, 1).unwrap().set_text("Status");
    table.cell_mut(1, 0).unwrap().set_text("Rust std only");
    table.cell_mut(1, 1).unwrap().set_text("In progress");

    doc.add_paragraph("Aligned text").alignment(Alignment::Center);
    doc.save("report.docx")?;
    doc.save_html("report.html")?;
    doc.save_markdown("report.md")?;
    doc.save_pdf("report.pdf")?;
    doc.save_png("report.png")?;
    Ok(())
}
```

> Images: call `let id = doc.add_image("photo.png", std::fs::read("photo.png")?, Length::inches(2.0), Length::inches(1.5));`
> then `doc.add_image_paragraph(id);`. The embedded bytes are stored as a native DOCX media part; image **decoding** is not required for embedding.

## Reading compressed DOCX

```rust
use mata_docx::Document;
fn main() -> std::io::Result<()> {
    let doc = Document::open("input.docx")?;
    for paragraph in doc.paragraphs() { println!("{}", paragraph.text()); }
    for table in doc.tables() {
        for row in table.rows() {
            for cell in row.cells() { print!("{}\t", cell.text()); }
            println!();
        }
    }
    Ok(())
}
```

ZIP reading accepts both uncompressed members (method 0) and compressed DEFLATE
members (method 8) and checks CRC and size limits. ZIP writing now uses a native fixed-Huffman/LZ77 DEFLATE compressor, with STORE for members whose compressed representation would be larger. ZIP64 is supported.

## Stages 1–3: newer APIs

```rust
use mata_docx::{Document, MathExpr, SectionBreak};
let mut doc = Document::new();
doc.add_paragraph("First section");
doc.add_section_break(SectionBreak::NextPage).page_number_start = Some(1);
doc.add_paragraph("One half:").add_math_expression(
    MathExpr::Fraction(
        Box::new(MathExpr::Text("1".into())),
        Box::new(MathExpr::Text("2".into()))
    )
);
let parent = doc.add_comment("Reviewer", "Is this correct?");
doc.add_comment_reply(parent, "Editor", "Yes.");
doc.save("modern.docx")?;
# Ok::<(), std::io::Error>(())
```

`package::Package` exposes low-level inspection and relationship validation;
`package::resolve_target` performs traversal-safe OPC URI resolution. No
third-party library is used for ZIP64, DEFLATE, or XML parsing. See
[ROADMAP.md](ROADMAP.md) for a precise list of remaining work in stages 1–3.

## Template replacement and merging

```rust
use mata_docx::Document;
use std::collections::HashMap;
fn main() -> std::io::Result<()> {
    let mut a = Document::open("template.docx")?;
    a.replace_text("{{name}}", "Example Name");
    // A regex-like subset: character classes, ., *, +, ?, ^, $, and escapes.
    a.replace_regex(r"\d+", "NUMBER")?;
    let b = Document::open("appendix.docx")?;
    a.append(&b);
    a.save("merged.docx")
}
```

`replace_text` and `replace_regex` work within individual text runs (not across
split Word runs). Regex-like syntax currently excludes groups, alternation,
lookaround and `{n}` quantifiers. Merge remaps images, footnotes, endnotes,
comments, and numbering IDs and deduplicates or renames colliding custom styles.

## CLI

```bash
cargo run --bin mata-docx -- inspect example.docx
cargo run --bin mata-docx -- text example.docx
cargo run --bin mata-docx -- validate example.docx
cargo run --bin mata-docx -- repair broken.docx repaired.docx
cargo run --bin mata-docx -- signatures example.docx
cargo run --bin mata-docx -- convert example.docx example.html
cargo run --bin mata-docx -- convert example.docx example.pdf
cargo run --bin mata-docx -- convert example.docx example.png
cargo run --bin mata-docx -- replace template.docx Draft Final result.docx
cargo run --bin mata-docx -- diff before.docx after.docx
```

## Running tests

```bash
cargo test --all-targets --offline
cargo run --example showcase --offline
cargo run --example stages123 --offline
python3 scripts/verify_package.py showcase.docx stages123.docx
# Optional on a machine with LibreOffice:
bash scripts/check_libreoffice.sh showcase.docx
# Or run all local checks:
bash scripts/check.sh
```

For Word on Windows, optionally run `scripts/check_word.ps1` in PowerShell with
an installed/licensed Microsoft Word version. These **test tools** are not
library dependencies.

Tests now cover ZIP64, native DEFLATE compression, malformed ZIP/XML inputs, mutation fuzzing, OPC relationships, sections, revisions, replies, math, glossary, custom XML and font parts. A compressed DOCX fixture and a tiny PNG are bundled. No build-time or runtime Rust dependencies are used.

## Project layout

| File | Purpose |
| --- | --- |
| `src/model.rs` | Document, paragraph, run, table, annotations and styles |
| `src/ooxml.rs` | DOCX serialization/parsing and OPC relationships |
| `src/zip.rs` | ZIP64 archive reader/writer, native DEFLATE encoder/decoder and CRC32 |
| `src/package.rs` | OPC part/relationship resolution, validation, signature invalidation and repair |
| `src/xml.rs` | Bounded namespace-aware XML parser and writer, UTF-8/UTF-16 decoding |
| `src/pattern.rs` | Dependency-free basic regex-like matching |
| `src/export.rs` | HTML, Markdown, lightweight PDF and PNG rendering |
| `src/bin/mata-docx.rs` | CLI |
| `examples/showcase.rs` | API demonstration |
| `tests/interop.rs` | Compressed file and annotation interoperability fixtures |

## Output fidelity

DOCX creation targets basic Office/LibreOffice interoperability. The PDF exporter
uses built-in Courier and approximate line wrapping; it replaces unsupported
Unicode glyphs with `?`, does not include pictures or advanced layout, and is
**not** a pixel-identical DOCX renderer. PNG rendering draws simple ASCII
bitmap-font previews and omits sophisticated OOXML layout.

## License

MIT. This is an independent implementation using std and the existing Mata font crate; the linked Zavora
repository is used as a feature reference, not imported as a dependency.

## Continuing stages 1–3 (unverified)

The `stream`, `fonts`, and expanded `package`, `model`, and `ooxml` modules
support incremental UTF-8 XML events, conservative OPC repair, section
formatting, modern comment metadata, revision details, additional mathematics,
floating image options, custom XML data binding, and embedded font obfuscation.
The crate continues to use the Rust standard library exclusively.

This implementation has **not** been compiled or tested in this environment,
and it is **not** yet fully conformant with Zavora DOCX or ISO/IEC 29500.
Consult `FEATURES.md`, `ROADMAP.md` and `BUILD_STATUS.md` before relying on it.

## Stages 4–6 — templates, Unicode/OpenType, and pagination

The following APIs were added in the stage 4–6 implementation pass. All
Rust source is still compiled from `std` alone; the generated Unicode tables
are committed to the source archive, and Cargo runs **no build script**.

### Template values, loops, and conditions

```rust
use mata_docx::Document;
use mata_docx::template::{Options, Value};

let mut doc = Document::new();
doc.add_heading("Hello {{person.name}}", 1);
doc.add_paragraph("{{#each jobs}}");
doc.add_paragraph("{{@index}} — {{this}}");
doc.add_paragraph("{{/each}}");
let values = Value::object([
    ("person".into(), Value::object([("name".into(), Value::from("Alex"))])),
    ("jobs".into(), Value::Array(vec![Value::from("Designer"), Value::from("Engineer")])),
]);
let report = doc.render_template(&values, &Options::default())?;
let matches = doc.replace_regex_across_runs(r"(?i)(designer|engineer)", "[$1]")?;
```

Supported template directives: `{{path}}`, `{{this}}`, `{{@root.name}}`,
`{{../name}}`, `{{@index}}`, `{{@first}}`, `{{@last}}`,
`{{#if path}}...{{else}}...{{/if}}`, `{{#unless path}}`, and
`{{#each list}}...{{else}}...{{/each}}`. A block directive on an otherwise
empty paragraph can repeat/remove complete paragraphs/tables, and a directive
on a row consisting of a single directive cell can repeat/remove table rows.

Template replacements can span adjacent rich-text runs, while preserving
untouched run formatting. Headers, footers, notes, comments, glossary and
custom XML model parts are supported. Optional unknown package XML rewriting
is controlled by `Options::process_package_xml` and touches XML text nodes,
not element markup or attributes. XML template editing can never guarantee
lossless fidelity of untouched XML serialization details such as whitespace
or original quote choice.

Regex support includes capture groups, named captures, alternation, backrefs,
character classes, common Unicode property classes, anchors, lookahead,
bounded lookbehind, greedy/lazy quantifiers, and `$1`/`${name}` replacement.
It is **not** PCRE-equivalent: for example, conditional groups, atomic
subexpressions, branch-reset groups and all Unicode property aliases are
not implemented. Matching uses a bounded backtracking budget.

### Unicode and fonts

```rust
use mata_docx::{unicode, opentype};
assert_eq!(unicode::nfc("Cafe\u{301}"), "Café");
let mut fonts = opentype::FontCollection::new();
// fonts.add("My OpenType Font", std::fs::read("my-font.ttf")?)?;
let shaped = fonts.shape("Hello", None, opentype::ShapeOptions::default());
```

Unicode 15.1.0 tables are checked into `src/ucd_tables.rs`. NFC, NFD,
NFKC and NFKD use full generated canonical/compatibility decomposition,
composition and combining-class tables. Grapheme segmentation supports UAX #29
emoji, Indic conjunct, Hangul and regional-indicator rules.

`opentype::Face` reads sfnt tables and maps characters using cmap format 4/12;
`FontCollection` selects fallback fonts. A common subset of GSUB and GPOS
(Single, Ligature and pair position adjustments) and `kern` is handled.
Advanced contextual/Indic shaping, variation fonts, Arabic joining and all
OpenType lookup types remain incomplete. Bidirectional reordering covers common
cases but does not implement all UAX #9 paired-bracket and isolate behaviors.

### Page geometry

```rust
use mata_docx::{Document, layout};
let mut doc = Document::new();
doc.add_paragraph("Hello, multi-page text layout!");
let mut engine = layout::Engine::new();
// Supply font data to get actual OpenType advances:
// engine.fonts.add("My Font", std::fs::read("font.ttf")?)?;
let result = doc.layout(&engine)?;
println!("{} pages", result.pages.len());
for page in &result.pages {
    println!("page {}: {} positioned text fragments", page.number, page.text.len());
}
```

The standalone layout engine handles page settings, sections, columns,
paragraph wrap/spacing, most common run formatting, paragraph alignment,
headers/footers, fixed table rows, page/column breaks, floated-image position,
and basic widow/orphan and keep-with-next behavior. It does **not** guarantee
pixel-identical Microsoft Word pagination and does **not** replace the existing
PDF/PNG exporters. High-fidelity rendering is still roadmap stage 7.

`examples/stages456.rs` demonstrates all three modules. Rust compilation and
test execution are deferred at the user's request; see `BUILD_STATUS.md`.

## Additional stage 4–6 implementation

The latest changes add regex atomic groups, possessive quantifiers, conditional
captures; more GSUB/GPOS and Unicode variation handling; nested OOXML-part
templates; preflight footnote reservation; and DrawingML tight/through polygons.
See `examples/advanced_stages_4_6.rs` and `IMPLEMENTATION_STATUS.md`.

These APIs are **not a guarantee of full Word, PCRE, OpenType or Unicode standards
parity**. The Rust build and interoperability tests have been intentionally
deferred. `mata-fonts` is an actual external local workspace path dependency,
not a std-only font reimplementation.

## Most recent stage 4–6 APIs (unverified)

The latest update adds recursive regex calls (`(?R)`, `(?1)`, `(?&name)`),
Unicode-default `pattern::full_case_fold`, expanded GSUB/GPOS processing,
and variable-font `FontCollection::set_variation(index, *b"wght", 600.0)`.
The chosen axes affect HVAR glyph advances; outline interpolation must be
provided by the installed rasterizer.

With a host-created `mata_fonts::decoders::ttf::Ttf`, the layout-aware bitmap
preview may be generated via:

```rust,ignore
let engine = mata_docx::layout::Engine::new();
let png_pages = document.to_png_pages_with_mata_font(&engine, &mut font, 144.0)?;
```

This preview uses scalar rasterization, not the font's GSUB glyph-ID path. Full
Word/PCRE/OpenType standards-level parity is **not claimed**. The unverified
status and remaining gaps are documented in [STAGES_4_6_LATEST.md](STAGES_4_6_LATEST.md).


## Latest stage 4–6 implementation pass

See [STAGES_4_6_CURRENT.md](STAGES_4_6_CURRENT.md) for the implemented regex, OpenType, and layout additions, source-only verification status, and unresolved standards-compatibility work. The crate still requires the existing local `mata-fonts` source.

### Pure-Rust PCRE2 compatibility status

The regex engine has additional duplicate captures, assertion conditions, selected
control verbs, Unicode set algebra and replacement syntax. **It is not a full
PCRE2 implementation**; see `PCRE_COMPATIBILITY.md` for the feature boundary.
This code uses no FFI or external regex crate. Builds and tests have not been run.


## Advanced shaping / layout update

The library includes native TrueType `gvar` outline interpolation, an unhinted glyph-ID rasterizer, modern Indic script-tag selection, variable-font feature substitutions, and additional OOXML layout controls. See [ADVANCED_SHAPING_LAYOUT_STATUS.md](ADVANCED_SHAPING_LAYOUT_STATUS.md) for implemented behavior and open compatibility gaps. The real `mata-fonts` workspace crate is required at the configured relative path; no bundled fonts or FFI are used.

### CFF2 outlines and multi-column layout (latest revision)

`Face::outline_instance(glyph_id)` supports native `glyf`/`gvar` point-variation
outlines and a bounded CFF2 Type 2 charstring/blend path. Both routes produce
unhinted outlines suitable for `rasterize_glyph_instance`. Word layout now
includes limited text-only section column balancing and footnote carryover.

See [`SHAPING_PAGINATION_STATUS.md`](SHAPING_PAGINATION_STATUS.md) for the
precise scope and unsupported conformance requirements. No Rust tests or builds
were run for this revision.

### Recent font and page-grid extensions

The `gdef` and `colr` modules provide native OpenType metadata and COLR v0
rasterization. `Face::rasterize_colr_glyph_instance` and
`Document::to_png_pages_shaped` expose the new rendering path. `DocumentGrid`
controls section line pitch and `Paragraph::snap_to_grid` overrides snapping.
See [SHAPING_PAGINATION_REVISION.md](SHAPING_PAGINATION_REVISION.md) for exact
limits. No additional dependencies or FFI have been introduced.


### New advanced text and table features

This snapshot adds native COLR v1 solid/gradient paint graphs, Indic syllable-aware shaping boundaries, Word row-height rules, percentage table widths and omitted row grid slots. See [COLOR_SHAPING_WORD_STATUS.md](COLOR_SHAPING_WORD_STATUS.md) for supported formats and outstanding conformance gaps.

### Optional Mata workspace font-image codecs

Color bitmap glyph decoding now requires the local `mata-image` crate alongside
`mata-fonts`. When installed at `Mata/core/mata-docx` the configured path
`../../media/mata-image` refers to `Mata/media/mata-image`. PNG, JPEG, and TIFF
`sbix` images are decoded in Rust; no FFI is introduced by `mata-docx`.
See `NATIVE_FONT_RENDERING_REVISION.md` for details and known limitations.
