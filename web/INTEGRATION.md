# Mata HTML workspace integration

Renamed `web/mwe-html` to `web/mata-html` and the Cargo package to `mata-html`
(the Rust crate is imported as `mata_html`). Its original tests and headless
rendering APIs are retained, with new `native.rs` for direct Mata UI drawing.

- Primary HTML/CSS display backend: **`mata-ui`**, located at
  `../../ui/mata-ui`. The HTML crate does not import `mata-win` directly.
- Added `WidgetWindow::remove_control` to remove native HTML controls when
  changing pages; native labels now paint styled rounded backgrounds with
  caller-selected alignment.
- Optional JavaScript dependencies remain registry `boa_engine` and `boa_gc`
  at **0.22.0** (not local upstream Boa source crates).
- No upstream Boa or non-Mata source crate was imported into the workspace.
- Workspace `default-members` now includes `web/mata-html`.
- `Cargo.lock` originated with the existing workspace and may require Cargo to
  re-resolve the new package's transitive dependencies. It is not verified.

Build from the Mata root directory with Rust 1.91 or later:

```bash
cargo check -p mata-html
cargo test -p mata-html
cargo check -p mata-html --features javascript
cargo run -p mata-html --example window
cargo run -p mata-html --features javascript --example interactive_window
cargo run -p mata-browser
```

**Validation:** The workspace manifests and all local path dependencies are
checked programmatically; Cargo compilation was not available in this session.
