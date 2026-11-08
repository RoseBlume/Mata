# MIVL 0.7 — responsive layouts and native scrollbars

See [STYLING.md](STYLING.md) for all new styling properties and the
[responsive example](examples/responsive.mivl). Previous command/async/event
features remain described in [BRIDGE.md](BRIDGE.md).

# MIVL (`mivl`)

**MIVL is a new interpreted, declarative UI language inspired by QML.** `.mivl` files can be parsed directly at runtime or validated and embedded through the optional build-time generator. The language frontend, AST, property-binding engine, expression interpreter, layout calculation, and event dispatch use **Rust's standard library only**. The built-in native renderer uses **your existing `mata-ui` and `mata-win` crates**, with no new direct third-party crate dependencies.

This is a functional **QML-like subset**, not a complete Qt QML / JavaScript / QtQuick implementation. Source compatibility with Qt QML, compiled JavaScript, CSS, animations, imports, and all QtQuick controls are **not** claimed.


## Embed MIVL at build time (`include_modules!`)

Use a build script to check and embed MIVL views, just like the normal
`slint::include_modules!();` workflow. **Only Mata crates and Rust's standard
library** are used. The `mivl-build` helper requires no third-party crates.

**Consumer `Cargo.toml`:**

```toml
[dependencies]
mivl = { path = "../mivl" }

[build-dependencies]
mivl-build = { path = "../mivl/mivl-build" }
```

**Consumer `build.rs`:**

```rust
fn main() {
    mivl_build::compile("ui/main_window.mivl").unwrap();
}
```

**Consumer `src/main.rs`:**

```rust
use mivl::command;
mivl::include_modules!();

#[command]
fn greet(name: String) -> String { format!("Hello, {name}!") }

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut window = MainWindow::new()?;
    register_greet(window.engine_mut())?;
    window.set("title", "MIVL app")?;
    window.run()?;
    Ok(())
}
```

`main_window.mivl` generates `MainWindow`. For multiple components, use
`mivl_build::compile_files(&["ui/main_window.mivl",
"ui/settings_window.mivl"])` and both `MainWindow` and `SettingsWindow`
become available. Generated windows dereference to `mivl::CompiledWindow`, so
`get`, `set`, `emit`, `register_command`, and `engine_mut` remain available.

**Validation model:** `.mivl` documents are parsed and checked at build time,
then embedded and interpreted when `MainWindow::new()` is called. This is not
AOT compilation of property expressions or layout logic into Rust. This first
version exports one `Window` root per MIVL file; it does not yet support
imported reusable components.

Try the bundled example:

```sh
cargo test --manifest-path examples/compiled_host/Cargo.toml
cargo run --manifest-path examples/compiled_host/Cargo.toml 
```

See [`MIGRATION.md`](MIGRATION.md) for the original MQML rename.


## Directory placement

Place `mivl` beside `mata-ui` and `mata-win` in your Mata workspace:

```text
Mata/
├── mata-win/
├── mata-ui/
├── mata-fonts/        # Existing optional font dependency of mata-ui
├── mivl/         # This new crate
└── Cargo.toml        # Add mivl to workspace members if desired
```

The **direct dependencies** declared in `mivl/Cargo.toml` are `mata-ui` and `mata-win` (both required), plus `mivl-macros`, a bundled zero-third-party procedural-macro crate that uses only Rust `proc_macro` and `std`. They use the exact existing path-based API conventions of the provided Mata repository archive. The crates' *existing* platform/transitive dependencies are unchanged; MIVL no longer has a `native-ui` feature; its native dependencies are always present.

## Run a `.mivl` application

```bash
cd mivl
cargo run --bin mivl -- examples/counter.mivl
```

The native UI dependency enables `fonts`, `wayland`, and `x11` features on your Mata crates. On Linux, `mata-win` tries Wayland and falls back to X11 using its existing backend selection. On Windows, `mata-win` uses its existing native implementation. To build a leaner Linux binary you can change these forwarded features in `Cargo.toml` to only one backend.

The language engine can be exercised without opening a native window (MIVL still links to `mata-ui` and `mata-win`):

```bash
cargo test --no-default-features
```

Programmatic use:

```rust
use mivl::{Engine, Value};
let mut ui = Engine::parse(r#"
    Window {
        property int counter: 0
        Text { id: label; text: "Count: " + counter }
        Button { id: add; onClicked: { counter += 1 } }
    }
"#)?;

ui.emit("add", "clicked")?;
assert_eq!(ui.get("label.text")?, Value::String("Count: 1".into()));
# Ok::<(), mivl::Error>(())
```

To embed the native UI in your application:

```rust
use mivl::{Engine, native::NativeApp};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let source = std::fs::read_to_string("examples/counter.mivl")?;
    let mut app = NativeApp::new(Engine::parse(&source)?)?;
    app.run()?;
    Ok(())
}
```

## Supported language constructs

| Feature | Implemented |
|---|---|
| Declarative typed/nontyped properties (`property int count: 0`, `text: "..."`) | Yes |
| Nested components, `id:` references, scope lookup, `parent.*`, `root.*`, `window.*` | Yes |
| Reactive property bindings after state mutation | Yes (evaluated on demand) |
| Arithmetic, comparisons, logical operators, unary operators, ternary conditional | Yes |
| Builtin functions `str`, `string`, `number`, `bool`, `len`, `abs`, `round`, `floor`, `ceil`, `min`, `max`, `clamp` | Yes |
| Event blocks such as `onClicked: { count += 1 }` | Yes, assignment and `invoke(...)` statements |
| `onCompleted`, `onClicked`, `onTextChanged`, `onAccepted`, `onResized` | Yes (for supported native event sources) |
| `Window`, `Rectangle`, `Item`, `Row`, `Column`, `Text`, `Button`, `TextField`, `ScrollView`, `Flickable`, `ListView`, `GridView`, `StackView`, `SwipeView` | Yes through built-in native rendering |
| `x`, `y`, `width`, `height`, `visible`, `enabled`, `color`, `fontSize`, `fontFamily` | Yes where applicable |
| `anchors.fill: parent/root/window`, `anchors.centerIn: parent/root/window`, `anchors.margins` | Yes |
| String/number/bool/list values, quoted strings, #RGB/#RGBA/#RRGGBB/#RRGGBBAA colors | Yes |
| Model-backed `ListView`/`GridView`, `delegate: Component { ... }`, `index`, `modelData` | Yes (array or integer models) |
| Dynamic model regeneration and property-preserving static items | Yes |
| `contentY`, `contentX`, `contentHeight`, `contentWidth`, `count`, viewport clipping | Yes for view-specific supported properties |
| `StackView`/`SwipeView` active page selection via `currentIndex` | Yes |
| Native view navigation/scroll buttons, `onCurrentIndexChanged`, `onContentYChanged` | Yes |
| Circulating property bindings or unknown symbols reported as errors | Yes |
| Full JavaScript engine, QtQuick controls, `Connections`, states, transitions, animations | Not yet |
| `Repeater`, Qt `ListModel` role objects, external component imports, custom functions | Not yet |
| Mouse wheel, touch/flick momentum and virtualized delegates | Yes, with native input patches (see INSTALL_INPUT.md) |
| True GPU scissor clipping and variable row-height virtualization | Not yet |
| Pixel clipping and advanced overlapping/z-index/window decorations | Not yet |

## Working views

Run the interactive sample (after applying [INSTALL_INPUT.md](INSTALL_INPUT.md)):

```bash
cargo run --bin mivl -- examples/virtualized.mivl
```

This example displays a 100,000-item model without 100,000 widgets. Run the existing smaller view sample:

```bash
cargo run --bin mivl -- examples/views.mivl
```

`ListView` and `GridView` accept an **integer model** (`model: 100`) or a **list** (`model: ["H", "He", "Li"]`), and one `delegate: Type { ... }` template. Each generated delegate has `index` and `modelData` bound in its local scope, also accessible to nested children. The integer form yields `modelData = index` from `0` through `count-1`.

```qml
Window {
    width: 640; height: 480
    property int total: 20
    ListView {
        id: results
        x: 24; y: 24; width: 280; height: 320
        spacing: 5
        model: total
        delegate: Button {
            height: 36
            text: "Entry " + index
            onClicked: { results.currentIndex = index }
        }
    }
    Button {
        x: 350; y: 24; text: "Add"
        onClicked: { total += 1 }
    }
}
```

When the model or its visible range changes, the engine re-instantiates the current small set of generated delegates and the native projection updates; static controls and their user-modified properties are preserved. Generated delegates intentionally cannot have their own `id` because repeated ids would be ambiguous. Their per-row local state is regenerated when the model changes.

`ListView` places items vertically and `GridView` places them in cells (`cellWidth`, `cellHeight`, `columns`, `spacing`, `padding`). `contentY` and `contentX` adjust the viewport origin; visible parts of child widgets are clipped to the view's rectangle. `contentHeight`, `contentWidth`, and `count` are computed for list/grid views. `ScrollView` and `Flickable` can scroll ordinary nested children using content offsets.

`StackView` and `SwipeView` show **only** the child at `currentIndex`, starting from `0`. Children without explicit width/height fill the page. Native up/down controls change the page; you can also change the index from a button or Rust with `set()`.

The **v0.3 input bridge** (included as Mata patches) forwards wheel and touch input. Scrolling targets the deepest view under the mouse, and touch or mouse dragging scrolls vertically and horizontally with bounded inertial movement. Horizontal gestures change `SwipeView.currentIndex`. Android already has native touch events in MataWin; the package adds a Windows `WM_POINTER` bridge, while Wayland/X11 still use mouse dragging if the backend does not provide touch contacts. Up/down controls remain available as a fallback.

Model-backed ListView/GridView now **instantiate only items in or near the viewport**, using a `cacheBuffer` of 120 logical pixels by default (up to 512 materialized items). Numeric models up to 1,000,000 elements do not allocate an array of model entries. Array models can have up to 1,000,000 values but the source array remains in memory. `index` always refers to the full model index and `count` the full model size, not the visible slice. ListView assumes fixed `itemHeight`: set it explicitly if the delegate's height is not a numeric literal.

Programmatic scrolling remains available via `Engine::scroll("list", 0.0, 60.0)`. Content clipping still intersects retained widget bounds; this is not yet GPU scissor clipping, and partially clipped text/widgets can display imperfectly. All input patches and platform limitations are detailed in [INSTALL_INPUT.md](INSTALL_INPUT.md).

## Architecture

```text
.mivl source
     │
     ▼
 lexer.rs  →  parser.rs  →  ast.rs
                              │
                              ▼
                        runtime.rs
                       ┌─────────┐
                       │ Engine  │
                       │ bindings│
                       │  events │
                       │ layouts │
                       └─────────┘
                              │ optional native UI support
                              ▼
                          native.rs
                              │
                    mata-ui WidgetWindow
                              │
                          mata-win
```

The engine **does not evaluate raw Rust code, load arbitrary external packages, access the filesystem or network from scripts, or embed a JavaScript runtime**. This limits the available script operations to a deliberately small, analyzable expression/event subset.

When a property is assigned inside a handler, the assignment overrides its original binding (matching common imperative QML semantics). A dependent binding continues to re-evaluate against that new value. Circular bindings are rejected with a property chain diagnostic. The native host polls Mata window events and updates retained widgets only if observed values change, so static UIs avoid continuous GPU uploads.

## Limitations and verification

The parser/runtime integration tests are located in `tests/interpreter.rs`. They cover parsing, mutations, bindings, expression precedence, cycles, duplicate IDs, scope resolution, and layouts. **This environment did not contain a Rust compiler**, so the project was authored and its Mata API references checked against the available `mata-win(7).zip` source, but it could not be built or test-executed here. Run the commands above in your workspace to verify on your platform. The native backend specifically targets the snapshot of `mata-ui` bundled with that archive; reconcile if your local Mata API has changed since then.

## Rust commands: MIVL invoking Rust (v0.4.0)

Two APIs are included: `Engine::register_command` for closures and `#[mivl::command]` for attribute-wrapped synchronous free functions. **Only functions you explicitly register are exposed**. Calls are only allowed inside event handlers; property bindings must remain side-effect-free. Commands are synchronous, so avoid slow operations on the native UI thread until async support is implemented.

### Attribute macro (similar to `#[tauri::command]`)

```rust
use mivl::{command, Engine};

#[command]
fn greet(name: String) -> String {
    format!("Hello, {name}!")
}

let mut engine = Engine::parse(r#"
    Window {
        property string result: "Click me"
        Button {
            id: hello
            onClicked: { result = invoke("greet", "Mata") }
        }
        Text { text: result }
    }
"#)?;
register_greet(&mut engine)?; // generated by #[command]
engine.emit("hello", "clicked")?;
assert_eq!(engine.get("result")?.display(), "Hello, Mata!");
# Ok::<(), mivl::Error>(())
```

Every `#[command] fn greet(...)` generates `register_greet(&mut Engine) -> Result<()>`. Accepted owned argument types: `String`, `bool`, `f32`, `f64`, `i32`, `u32`, `i64`, `u64`, `usize`, `Value`, `Vec<Value>`. Accepted return types: the same primitives, `Value`, `Vec<Value>`, `()`, `&str`, or `Result<T,E>` where `T` is convertible and `E: Display`. Numeric conversions are range-checked; integer precision is limited by `f64`'s 53-bit significand. No `syn`, `quote`, `serde`, runtime reflection, JavaScript, or platform dependency is added. The attribute currently excludes async methods, references, receivers, generics and argument-name customization.

### Register a closure manually

```rust
use mivl::{Engine,Value};
let mut engine=Engine::parse(r#"Window { Button {id: go; onClicked: {invoke("log_message", "Hi")}} }"#)?;
engine.register_command("log_message", |args| {
    println!("MIVL: {}", args[0].display());
    Ok(Value::Null)
})?;
engine.emit("go","clicked")?;
# Ok::<(), mivl::Error>(())
```

Run the native example once the Mata input patches are installed:

```bash
cargo run --example command_host
```

See `examples/commands.mivl` and `examples/command_host.rs`. The regular `mivl` CLI has no custom host commands registered by default. The app embedding the engine must register command handlers first, before `NativeApp::new` triggers `onCompleted`.

**Safety and error semantics:** invalid argument counts/types and missing commands produce `mivl::Error`. Property mutations within a failed event are rolled back, but **external Rust effects cannot be rolled back**. The registry and capture state are shared across internal Engine snapshots and virtualized view recompositions. Register only trusted commands; arbitrary `.mivl` documents should not be allowed to invoke sensitive host operations. A command must not recursively invoke itself while borrowed.

## Backlog and names

- See [`todo.md`](todo.md) for the prioritized widget inventory and acceptance criteria.
- See [`NAMING.md`](NAMING.md) for names and suggested language file extensions.

## New in v0.6: async commands and Rust-to-MIVL events

See [BRIDGE.md](BRIDGE.md) for complete examples of the exact
`mivl::register_commands![resources::add_numbers, greet]` macro, nested Rust
command modules, `async fn` commands, `invoke_async`, `FrontendHandle`,
`EventListener`, and one-shot, counted and persistent listeners.

Runnable example: `cargo run --example bridge_host `.

### Text clipping fix (v0.7.3)

`examples/responsive.mivl` now uses Mata font metrics for intrinsic text widths and heights in the native renderer. `Text` labels are placed inside their padded bounds according to `textAlign` and `verticalAlign`. To test: `cargo test -p mivl --all-features` and `cargo run -p mivl  --bin mivl -- mivl/examples/responsive.mivl`.

### Text rendering (v0.7.3)

Width-constrained `Text` components now break words and measure auto-height
using native Mata font advances. Text partially scrolled out of a clipped
viewport retains its original position. For supported line wrapping, truncation,
line-height, and alignment properties see [RENDERING.md](RENDERING.md).
Run `examples/text_rendering.mivl` to exercise the new paths.

### Idle CPU/GPU usage (v0.7.4)

`NativeApp::run()` no longer performs text measurement and layout on every
8 ms poll. Property writes advance `Engine::revision()`, and Mata controls
are synchronized only when that revision (or the delegate generation) changes.
Static windows idle with a 50 ms input poll; recent input and active dragging
poll at 16 ms. `WidgetWindow::present()` already performs dirty-frame checks
so unchanged windows should not upload identical full frames.

Run `cargo run -p mivl  --bin mivl -- mivl/examples/responsive.mivl`
then leave the window untouched for several seconds. Profile CPU and GPU while
idle, then repeat while resizing and scrolling. This source change has not been
benchmarked on Windows hardware in this environment.
