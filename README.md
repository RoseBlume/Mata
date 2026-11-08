# Mata Unified Workspace — Standardized Crate Names

The Mata Cargo workspace use consistent **kebab-case** Cargo package names and physical folder names. Rust imports use equivalent **snake_case** identifiers. The 30 Tauri/ChromeOS/desktop-environment packages previously removed are still absent.

## Renamed packages

| Old package | New package / directory | Rust crate identifier |
|---|---|---|
| `mataimage` | `mata-image` | `mata_image` |
| `matafonts` | `mata-fonts` | `mata_fonts` |
| `matawin` | `mata-win` | `mata_win` |
| `matahtml` | `legacy/mata-html` (archived; not built) | `mata_html` (legacy only) |
| `mataarc_metadata` | `mata-arc-metadata` | `mata_arc_metadata` |
| `mata-visualengine` | `mata-visual-engine` | `mata_visual_engine` |

Unchanged crates already follow kebab-case naming; their public API and imports remain intact. The optional native CUDA/HIP decoder retains its original `mataimage_*` foreign-function symbols and dynamic-library names for binary compatibility; these are not Cargo crate names.

## Workspace inventory

**projects** (8): `mata-hole`, `mata-hole-browser`, `mata-neon-run`, `mata-raycast`, `mata-rpg-editor`, `mata-rpg-engine`, `mata-tower-defense`, `mineral-empire`

**macros** (2): `mivl-macros`, `remata-macros`

**ui** (3): `mata-ui`, `mata-win`, `mivl`

**media** (8): `mata-arc-metadata`, `mata-audio`, `mata-docx`, `mata-fonts`, `mata-image`, `mata-metadata`, `mata-playback`, `mata-video`

**web**: `mata-net`, `mata-networking`, `mata-script`, `mata-script-runtime`, `mata-script-syntax`, `mata-webview` (Servo only)

**graphics** (5): `mata-3d`, `mata-gpu-ffi`, `mata-gpu-intrinsics`, `mata-gpu-runtime`, `mata-visual-engine`

**apps**: `mata-browser` (Servo), `mata-document`, `mata-lockdown`, `mata-molecule`, `mata-recover`

**tools** (3): `mata-bundler`, `mivl-build`, `ncbi-asset-downloader`

**core** (2): `mata-i18n`, `mata-sqlite`

## Validate

```bash
python scripts/verify_workspace.py
cargo metadata --no-deps --format-version 1
cargo check --workspace
```

`MODULE_TREE.md` and `MODULE_TREE.txt` list all crate-level Rust source trees; `FULL_FILE_TREE.txt` lists all workspace files. `SOURCE_MAP.json` retains source-archive paths for provenance, `RENAME_MAP.json` maps old/new crate names and folders, and `REMOVED_CRATES.json` tracks removed desktop and Tauri projects.

## MIVL native UI migration (October 2026)

The older `mata-qml` and `mata-qml-macros` crates have been removed. MIVL
is the maintained declarative UI implementation. Its `mata-ui` and `mata-win`
dependencies are mandatory; native rendering does **not** require a feature flag.

The pointer, scroll and touch `WidgetEvent` API required by MIVL is now
implemented in `ui/mata-ui` rather than supplied by an out-of-tree override.


## Servo-only browser migration (October 2026)

`mata-browser` and `mata-webview` now embed **Servo 0.7.0**. Servo handles
HTML/CSS layout, DOM, JavaScript (SpiderMonkey), navigation, resource fetching,
and page painting. The previous standalone `mata-html` + Boa renderer and old
browser custom HTTP loader are archived in `legacy/` and excluded from Cargo
workspace members. The existing native `WebView2`/`WebKitGTK` widget backend
source was removed. `mata-ui/webview` is an alias for `mata-ui/webview-servo`.

```sh
cargo run -p mata-browser -- https://servo.org/
cargo run -p mata-webview --example embed
cargo check -p mata-document --features browser
```

The previous `Cargo.lock` was moved to `legacy/Cargo.lock.before-servo` because
it contained Boa/ICU 2.3 pins that are not part of this workspace anymore.
Cargo will create a fresh lockfile on the first build. Check
`SERVO_MIGRATION.md` for caveats and the exact changed API.
