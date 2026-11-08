# mata-docx feature overview — stages 1–3 continuation

**Experimental, uncompiled in this environment, one local `mata-fonts` Rust dependency (no other direct external dependencies).**

| Feature | Status | Implementation |
|---|---|---|
| ZIP64, native DEFLATE, CRC, archive limits | Existing | `zip` |
| Relationship inspection, strict URI normalization, package preservation | Existing | `package` |
| OPC namespace/metadata repair | Expanded, conservative | `Package::repair_word_package` |
| Read-based incremental SAX-like UTF-8 XML | Added | `stream::read_events` |
| UTF-8/UTF-16 bounded XML DOM | Existing | `xml` |
| Unknown part injection/removal | Added | `Document::put_part` / `remove_part` |
| Unknown paragraph property and imported run preservation | Improved, partial | `merge_paragraph_extensions`, `run_xml` |
| XMLDSig crypto signing and verification | **Not implemented** | No unsafe partial verification |
| Section overrides/columns/page numbering | Expanded, partial | `Section`, `final_section`, `section_full_xml` |
| Moves and tracked changes with metadata | Expanded, partial | `RevisionKind`, `RevisionInfo` |
| Threaded comments, resolution, author identities, durable IDs | Expanded, partial | `CommentMetadata` and Word parts |
| Math operators, matrix, delimiters, function, accents | Expanded, partial | `MathExpr`, raw OMML fallback |
| Floating image positioning | Expanded, partial | `FloatingOptions` |
| Theme colors, defaults, common style inheritance | Expanded, partial | `theme_colors`, `resolved_style`, `resolved_run` |
| Complex Word fields and cached results | Added, partial | `Inline::ComplexField` |
| XML-bound content controls | Added, partial | `StructuredContentControl`, safe restricted XPath |
| Font obfuscation and OS/2 embedding checks | Added | `fonts::obfuscate`, `deobfuscate`, `embedding_allowed` |
| Complete ISO/IEC 29500 OOXML model | **Not implemented** | Raw XML preserves many unmodeled elements |
| PDF/PNG/HTML/Markdown output | Existing, limited | `export` |

Compatibility remains unverified until the user runs Rust tests and generated
documents are independently checked in Word/LibreOffice.

## Stages 4–6 additions

| Feature | Scope | Status |
|---|---|---|
| Typed templates, nested `if`/`each`, block repetition | `template.rs` | Implemented for modeled paragraphs/tables |
| Run-spanning literal/regex replacement | `template.rs`, `pattern.rs` | Implemented |
| Full regex semantics | `pattern.rs` | **Partial**: missing advanced PCRE features |
| Unicode 15.1 canonical/compat normalization | `unicode.rs`, `ucd_tables.rs` | Implemented |
| UAX #29 grapheme rules | `unicode.rs` | Implemented standard boundaries |
| Bidi | `unicode.rs` | Common cases; **partial** full UAX #9 |
| OpenType metrics / GSUB / GPOS / fallback | `opentype.rs` | Basic shaping; **partial** complex scripts |
| Page/column flow and positioned objects | `layout.rs` | Implemented geometry; **partial** Word parity |
| Word-identical pagination | `layout.rs` | **Not complete** |


## Stages 4–6 update

See [IMPLEMENTATION_STATUS.md](IMPLEMENTATION_STATUS.md) for details about newly
added template text-carrier support, Unicode tables, font selection, TTC parsing,
hyphenation, and pagination, alongside explicit limitations.

### Latest source follow-up: stages 4–6 (not compiler-validated)

Additional regex recursive subroutines/casefold, GSUB contextual/chaining
format-3, GPOS single/cursive/mark-to-ligature, HVAR variable advances,
section/metadata fields, rotated wrap estimates, corrected line justification
and Mata-font PNG preview APIs. See `STAGES_4_6_LATEST.md`. Standards-level
remaining gaps prevent full stage completion claims.


### Current color/shaping/layout additions

Native COLR v1 variation/ClipList support, PNG-backed bitmap font strikes (`sbix`/`CBDT`), additional Southeast Asian syllable clustering, and independent Word table border read/write and positioned rendering are included. See `ADVANCED_LAYOUT_COLOR_REVISION.md` for format limitations and unverified status.

### Native font rendering continuation (source-only, unverified)

- Local Mata `mata-image` decoder integration for sbix JPEG/TIFF/PNG and CBDT
  PNG with additional color depth/interlace support provided by that crate.
- TrueType graphics-state single-width cut-ins and contour flag changes.
- OpenType SVG rasterizer: path arcs/smooth commands, affine transformations,
  basic local gradients, simple strokes and foreground `currentColor`.
- Full SVG/CSS and complete TrueType VM platform parity **not yet supported**.

See `NATIVE_FONT_RENDERING_REVISION.md` for implementation status.
