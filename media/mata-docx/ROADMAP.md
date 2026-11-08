# mata-docx roadmap — continued stages 1–3 implementation

Zero external Rust crates, zero linked native libraries. This update did **not**
run Rust compilation, unit tests, office compatibility checks, or fuzzing, at
user request. It is an implementation pass, **not a verified release**.

## Stage 1 — build correctness and QA

- [x] Cross-platform CI workflow, fixtures, offline-compatible unit and property tests, and optional Office/LibreOffice scripts (existing code).
- [x] ZIP/OPC structural verification script (existing code).
- [ ] Run builds, tests, Miri, fuzzing, sanitizer checks, and Word/LibreOffice interoperability. **All testing intentionally deferred by user.**

## Stage 2 — OPC ZIP, XML, package roundtrip

- [x] ZIP64 read/write, fixed Huffman DEFLATE, limits, CRC checks (existing code).
- [x] Strict internal OPC URI resolution, relationship validation, and dangling-reference repair (existing code).
- [x] `Package::repair_word_package` repairs known namespace declarations, missing content-type defaults and overrides, missing root officeDocument relationship, and missing document body (with data-loss warning).
- [x] `stream::read_events` incremental, bounded UTF-8 SAX-style XML event reader accepting any `Read` source. Does not build a complete XML tree.
- [x] `Document::put_part`, `part` and `remove_part` for independent preservation/editing of unknown package parts.
- [x] Preserve original root relationships, untouched XML paragraphs/tables/parts, unknown inline OOXML, and unknown paragraph properties when editing; retain untouched imported run XML, and merge unrecognized run formatting when text changes.
- [x] Drop stale package signature artifacts on edited saves, retain on unmodified roundtrips.
- [ ] Streaming UTF-16 XML transcoding with constant-memory buffers; UTF-16 currently supported in the DOM-based reader only.
- [ ] Full XMLDSig W3C canonicalization/signing/cryptographic verification and key/certificate policy. **Not implemented.** Never interpret detected signatures as verified.
- [ ] Absolutely lossless editing of arbitrary XML content in *every* nested OOXML construct. Unknown nodes in regenerated tables/inline wrappers can still be affected.
- [ ] Recover badly corrupted ZIP payloads, invalid relationship graphs and unknown namespace declarations: repair is conservative and cannot recover deleted user content.

## Stage 3 — OOXML document model

- [x] Multi-section model extended with page numbering formats, columns/spacing/column widths, header/footer distances, gutter, first-page mode, imported header/footer relationship overrides, and unknown section extensions. Separate final-section representation.
- [x] Tracked insert/delete and move-from/move-to with explicit revision author/date/id and preserved formatted runs; flatten via `accept_revisions`/`reject_revisions`.
- [x] Threaded comment replies, resolved state, stable paragraph IDs, modern durable IDs, initials/date, people-provider/user IDs, and commentsIds relationships/parts.
- [x] Expanded typed Office Math expressions: matrices, delimiters, functions, sub/superscript combinations, accents, equation arrays; unsupported math operators retained as raw OMML rather than discarded.
- [x] Floating picture positioning API: horizontal/vertical anchors, offsets, distances, z-order, page text position, rotation, basic wrapping and re-import.
- [x] Theme color and `docDefaults` ingestion, style inheritance with explicit false booleans, and resolved style/run helper APIs.
- [x] Complex field representation supporting any field instruction with cached result and field lock/dirty flags; parse simple field sequences.
- [x] Structured content-control API with IDs/aliases/locks, text/rich text, data bindings and repeating section flag; conservative XPath evaluator for absolute child/attribute paths, and binding refresh API.
- [x] ECMA-376 OpenType embedded font XOR obfuscation/deobfuscation and OS/2 embedding restriction detection (font license still authoritative).
- [x] Preserved support from previous stages: table layout/merge/borders/shading, notes, glossary, basic drawing, custom XML itemProps, styles, charts.
- [ ] Complete section-specific authored header/footer *content*, all page/line/grid section policy settings and section-level page layouts. Imported relationship refs are preserved but authored override parts are not fully generated.
- [ ] Full structural revision families and metadata including all property and row revisions, and acceptance/rejection of nested table changes.
- [ ] All Office generations' people/mentions/threaded-comment structures, identity providers and authentication metadata.
- [ ] Full typed OMML operator coverage with editable generic AST. Unknown operators currently use opaque raw XML.
- [ ] All DrawingML geometry, charts/shapes transforms, rotation for non-picture shapes, anchors and tight/through wrap polygon generation. Tight/through currently serialize as square-wrap fallback.
- [ ] All style inheritance (table/numbering styles, formatting suppression, theme font mapping and authored theme changes). Current resolution covers common run properties and theme colors.
- [ ] All field evaluation/result updating in Word-compatible semantics.
- [ ] Full XPath 1.0, XSD schema validation and rich repeating-content controls. Bound-content refresh intentionally accepts only a safe XPath subset.
- [ ] End-to-end font-family style selection and embedding-rights review. OS/2 flags cannot replace a license review.

## Stages 4–8 (later)

4. Template engine and replacement across runs/parts with full regex semantics.
5. Unicode shaping, font fallback, GSUB/GPOS, bidi, segmentation and normalization.
6. Accurate Word layout and pagination.
7. High-fidelity PDF/PNG export with font embedding and graphics.
8. CLI/conversion/WASM and business templates.

**Status:** Stage 1 is implementation-complete with verification deferred. Stages
2–3 are *not fully feature-complete*; the outstanding items above should not be
misrepresented as implemented. `Cargo.toml` still has an empty `[dependencies]`.

---

## 2026-11-03 — stages 4–6 implementation update

See [STAGES_4_6.md](STAGES_4_6.md) for detailed coverage and remaining gaps.

- [x] Stage 4: Typed template engine, nested conditionals/iteration, body and
  table-row expansion, run-spanning replacement, typed regex captures and
  named replacements, limited opt-in package XML substitutions.
- [ ] Stage 4 remaining: comprehensive regex engine parity; all OOXML text
  carriers, in-place arbitrary XML editing, and complex repeated table blocks.
- [x] Stage 5: Unicode 15.1 normalization and grapheme segmentation, bidi
  classes and common reordering, OpenType cmap/metrics, selected GSUB/GPOS,
  kerning and font fallback.
- [ ] Stage 5 remaining: complete complex-script shaping, all GSUB/GPOS types,
  full UAX #9/#14 and script-specific line-breaking.
- [x] Stage 6: Independent geometry/pagination engine for sections, columns,
  paragraphs, tables, images, headers/footers and common keep/break behavior.
- [ ] Stage 6 remaining: Word-compatible line/table composition, footnotes,
  polygon wrapping, field repagination, detailed section balancing and
  end-to-end font/graphic layout fidelity.

**Rust tests/builds remain deferred.** No release-completion claim is made.

---

## Latest update — direct `mata-fonts` dependency, stages 4–6 expansion

The original std-only dependency constraint has been relaxed at the user's
request **only for the existing local `mata-fonts` crate**. See the full new
[implementation status](IMPLEMENTATION_STATUS.md).

- [x] Stage 4 additional OOXML drawing text carriers, multi-cell table row
  directives, and generalized Unicode regex categories.
- [ ] Stage 4 remaining: full PCRE regex behavior, arbitrary nested OOXML
  block editing, and lossless complex template formatting.
- [x] Stage 5 direct `mata-fonts` bridge, generated Unicode line/word/sentence
  property data, expanded script tags, Arabic joining, TTC parsing, style-aware
  font fallback, variable-axis metadata and dictionary hyphenation.
- [ ] Stage 5 remaining: complete UAX/GSUB/GPOS implementation, precise Indic
  and Arabic script engines, color emoji sequence shaping, variable outline
  interpolation, and verification with the **actual** `mata-fonts` crate.
- [x] Stage 6 body field repagination, section page numbering, Unicode line
  opportunities, table row splitting/repeating headers, footnote/endnote
  geometry, and rectangular float exclusions.
- [ ] Stage 6 remaining: full Word-compatible line/table/footnote/float
  composition and exact layout interoperability.

**No build or test execution was performed.** All new code remains unverified.

---

## Further stage 4–6 implementation — current revision

The prior entries are historical snapshots. For the authoritative updated
status, read [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md).

- [x] Stage 4: atomic / possessive / conditional regex support and OOXML-part
  nested paragraph/table-row block expansion.
- [x] Stage 5: GSUB 2/3, GPOS mark attachments, format-14 Unicode variation
  selectors and a glyph-ID rendering callback for `mata-fonts` adapters.
- [x] Stage 6: preflight footnote reservation, intrinsic table auto-fit, merge
  continuation layout, image collision avoidance, tight/through wrap polygons.
- [ ] Stage 4: exhaustive regex engine compatibility and arbitrary lossless OOXML.
- [ ] Stage 5: all complex-script shaping, font variations and UAX compliance.
- [ ] Stage 6: Word-level pagination and renderer/layout fidelity.

**No Rust tests or compilation were run, as requested.** The local workspace
still needs the real `Mata/media/mata-fonts` crate at the configured path.

---

## Latest source follow-up — advanced stage 4–6 engines

See [STAGES_4_6_LATEST.md](STAGES_4_6_LATEST.md) for implemented APIs, limits and the open work.

- [x] Recursive regex subroutines and full Unicode-default fold table, with explicit recursion/step bounds.
- [x] Additional GSUB contextual/chaining format-3 lookups and reverse chaining.
- [x] GPOS SinglePos, CursivePos and MarkToLigature support.
- [x] Variable-font HVAR advance interpolation with `fvar` axis settings and `avar` mapping.
- [x] Deterministic document metadata fields, corrected last-line justification, rotated wrap sampling and `mata-fonts` PNG raster preview.
- [ ] PCRE2 grammar/conformance, reversible editing of arbitrary OOXML wrappers, and full Unicode segmentation/bidi conformance.
- [ ] All OpenType/AAT script engines, variation outlines/feature variations and full `mata-fonts` glyph-ID interoperability.
- [ ] Word-compatible page balancing, every footnote/table/frame interaction and full fidelity rendered exports.

**Completion rule:** These still-open requirements prevent stages 4–6 from being accurately described as fully complete. Rust compilation and tests remain intentionally deferred.

---

## Subsequent implementation pass — 2026-11-03

See [STAGES_4_6_CURRENT.md](STAGES_4_6_CURRENT.md). The following **specific features** have source implementations, not runtime-verified conformance:

- [x] Stage 4: scoped regex options, `\\X` grapheme matches, `\\R` newline sequences, Unicode whitespace escapes, `\\Q...\\E` quoted literals, fallible budget-reporting search/replacement APIs.
- [x] Stage 5: GSUB 5/6 context/chaining formats 1/2; GPOS 7/8 context/chaining formats 1/2/3 for nested single/pair positioning; MVAR hhea instance metrics.
- [x] Stage 6: `w:keepLines` import/export and flow, cell vertical alignment, table origin from alignment/indent, variable-font line heights.
- [ ] Stage 4: full PCRE2/Oniguruma behavior and arbitrary lossless OOXML template mutation.
- [ ] Stage 5: exhaustive complex-script shaping/UAX compliance, feature variations, variable-font glyph outlines and verified `mata-fonts` integration.
- [ ] Stage 6: pixel/line/page-level Microsoft Word layout fidelity and complete high-resolution graphics/PDF/PNG export.

**These stages are not fully complete.** Rust compilation and tests remain intentionally deferred.

---

## Current regex work (PCRE2 compatibility)

See [PCRE_COMPATIBILITY.md](PCRE_COMPATIBILITY.md). The native Rust regex engine now recognizes additional PCRE2 expressions and has bounded error-returning APIs. The remaining full PCRE2 compatibility requirement is **open**, including advanced control verbs, complete conditional/assertion semantics, extended sets, Unicode properties, match API modes and differential verification. No FFI has been added. Rust compilation and tests remain deferred by user instruction.

## Current additional PCRE2 progress (pure Rust, no FFI)

- [x] Duplicate named capture parsing and resolution (`(?J)`).
- [x] Conditional assertions and recursion tests (supported subset).
- [x] Non-atomic positive lookaround variants and named lookaround aliases.
- [x] Initial PRUNE/THEN/COMMIT and named SKIP cut-control handling (semantics not exhaustive).
- [x] Perl-style extended character-set operations (supported subset).
- [x] Named replacement references, conditional substitutions, and extended replacement API.
- [x] Configurable newline conventions and ASCII-class option families.
- [ ] Complete PCRE2 language/VM parity, matching modes, callbacks, and differential conformance tests.

See `PCRE_COMPATIBILITY.md` for precise details. Tests and Rust compilation are intentionally deferred.


## Latest advanced shaping and pagination implementation pass (2026-11-03)

- [x] Native `glyf`/`loca` contour reading and supported `gvar` deltas, including IUP; unhinted outline glyph-ID rasterization.
- [x] Script-separated shaping, modern Indic tags, feature-order correction and conditional FeatureVariations.
- [x] Additional WordprocessingML tabs, widow/contextual spacing, auto/exact/atLeast line rules, mirrored margins, gutter and keep chains.
- [ ] Comprehensive script-engine conformance (Indic reordering, AAT, all GPOS flags and all languages).
- [ ] CFF2 variations, point-aligned compound glyf glyphs and hinting.
- [ ] Word-identical pagination, full footnote/table flow, all drawing shapes, interoperability validation.

Detailed implementation status: [ADVANCED_SHAPING_LAYOUT_STATUS.md](ADVANCED_SHAPING_LAYOUT_STATUS.md).

## Font-shaping and pagination continuation — 2026-11-03

- [x] TrueType point-attached composite glyphs and scaled component offsets.
- [x] Native CFF2 charstrings, local/global subroutines, variation-store
  `vsindex`/`blend`, and unhinted cubic-to-polygon outline output.
- [x] Indic pre-base vowel sign reorder pass for selected Indic scripts.
- [x] Text-only final-page column balancing, plus footnote line continuation
  without overwriting the document body.
- [ ] Exhaustive language-specific script shaping and all Unicode/OpenType
  script and font fallback semantics.
- [ ] Hinting and fully conformant font rendering (including complex color
  fonts and all CFF2 edge cases).
- [ ] Microsoft Word-identical multi-section, table, anchor, line break,
  baseline, footnote and compatibility-mode pagination and interoperability
  verification.

See `SHAPING_PAGINATION_STATUS.md`; the remaining unchecked work is real
implementation scope, not merely deferred compilation/testing.

## Additional font/layout implementation — 2026-11-03

Source additions (uncompiled, untested as requested):

- [x] Decode GDEF glyph classes and mark filtering sets.
- [x] Apply GDEF lookup exclusions to direct GSUB substitutions and ligatures.
- [x] Preserve ignored combining marks inside GSUB ligature substitutions.
- [x] Expose GDEF ligature caret values.
- [x] Respect explicitly selected OpenType LangSys feature tables in shaping.
- [x] Rasterize COLR v0 layers with CPAL colors and outline variation axes.
- [x] Expose glyph-ID-based shaped PNG previews.
- [x] Round-trip section `w:docGrid` and paragraph `w:snapToGrid` properties.
- [x] Apply line-pitch snapping in ordinary body pagination.
- [ ] Full Indic and other script-engine syllable algorithms and all GPOS/GSUB lookup flags.
- [ ] AAT/Graphite, COLR v1, SVG/bitmap color fonts, OpenType/TrueType hinting.
- [ ] Character-grid composition and all Word compatibility layout modes.
- [ ] Full Microsoft Word pagination and rendering conformance across versions.
- [ ] Rust compilation, unit tests, cross-application compatibility tests.


### Most recent source-only implementation pass

See [COLOR_SHAPING_WORD_STATUS.md](COLOR_SHAPING_WORD_STATUS.md) for implemented COLR v1 formats, Indic syllable-boundary segmentation, and additional Word table geometry. Full Word-pixel-equivalent pagination, universal complex-script shaping, TrueType hinting, bitmap color glyph formats and variable COLR paint deltas remain open. No tests have been run.


## Continued color-font and table geometry implementation (2026-11-03)

See [ADVANCED_LAYOUT_COLOR_REVISION.md](ADVANCED_LAYOUT_COLOR_REVISION.md) for precise API and format coverage.

- [x] COLR v1 variable solid/gradient/transform paints and ClipList rectangles.
- [x] `sbix` and `CBDT` PNG bitmap strikes with bounded native PNG decoding.
- [x] Conservative Myanmar, Khmer, Thai and Lao orthographic-syllable segmentation.
- [x] Preserve per-edge OOXML table borders and cell shading through Word input/output and render them in layout-aware PDF/PNG previews.
- [ ] Complete OpenType/AAT/Graphite script engines and exact Unicode shaping conformance.
- [ ] TrueType hinting bytecode interpreter, SVG glyph rasterization and every COLR/bitmap font format.
- [ ] Full Word table-border precedence, complex frame/section/footnote composition and Word-identical page-break behavior.
- [ ] Rust compilation, tests, reference-font conformance and multi-version Microsoft Word comparisons (all deferred by user).

## Native TrueType hinting and color-font expansion (2026-11-03)

- [x] Bounded native TrueType VM with font/prep/glyph execution phases, CVT,
  function calls, point movement, interpolation and hinted phantom metrics.
- [x] Native EBDT/EBLC grayscale and monochrome glyphs.
- [x] CBDT format 19, CBLC sparse/fixed indices and raw BGRA bitmap decoding.
- [x] OpenType SVG glyph document extraction/gzip and restricted static rendering.
- [x] Read raw sbix JPEG/TIFF/PNG data and dupe aliases without FFI.
- [ ] Exhaustive TrueType opcode and interpreter-behavior conformance.
- [ ] Full SVG 1.1 + CSS color-font renderer, JPEG/TIFF codecs, complete COLR
  variation behavior and comprehensive cross-font conformance.
- [ ] Rust compilation and test execution (intentionally deferred).

See `NATIVE_HINTING_COLOR_STATUS.md` for exact scope and limitations.

### Continued rendering work (November 2026)

- [x] Decode sbix JPEG/TIFF/PNG using the local Rust `mata-image` codec.
- [x] Replace CBDT 8-bit-only PNG decoder with `mata-image` integration.
- [x] Implement common SVG transforms, S/T/A path commands and local static gradients.
- [x] Implement basic SVG strokes, opacity and currentColor.
- [x] Improve TrueType SSWCI/SSW and MIAP/MIRP cut-in handling and FLIP contour flags.
- [ ] Verify against the actual `mata-fonts` and `mata-image` crates (not run).
- [ ] Complete TrueType VM interpreter mode, hinting and grid-fit reference conformance.
- [ ] Complete SVG 1.1, CSS cascade, paint effects, font-glyph text and SVG stroke semantics.
- [ ] Complete COLR v1 and platform-specific color-font reference conformance.
