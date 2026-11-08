# Rename from Mata QML / MQML to MIVL

**Official name:** MIVL — Mata Interface View Language.
**File extension:** `.mivl`.
**Crate:** `mivl` (Rust path `mivl::`).
**Native CLI:** `mivl`.
**Build helper:** `mivl-build` (Rust path `mivl_build::`).
**Attribute macro crate:** `mivl-macros` (used through `#[mivl::command]`).

| Before | After |
| --- | --- |
| `mata-qml` package | `mivl` package |
| `mata_qml::Engine` | `mivl::Engine` |
| `#[mata_qml::command]` | `#[mivl::command]` |
| `register_greet(&mut engine)` | Same generated registration helper |
| `app.mqml` | `app.mivl` |
| `mata-qml examples/app.mqml` | `mivl examples/app.mivl` |

**Syntax compatibility:** The existing QML-inspired syntax and `invoke(...)`
command bridge are unchanged. Old `.mqml` files still parse if passed to
`Engine::parse` or to the CLI, but the file extension for build integration
is `.mivl`. The package no longer exports the `mata_qml` crate name.

**Workspace migration:** Move `mata-qml` to `mivl` and replace your dependency
with `mivl = { path = "../mivl" }`. Remove old
`mata-qml` workspace entries; add `mivl`, `mivl/command-macros`, and optionally
`mivl/mivl-build` (not required unless you want it as a workspace member).

**Build-time modules:** Add to your application's `Cargo.toml`:

```toml
[dependencies]
mivl = { path = "../mivl" }
[build-dependencies]
mivl-build = { path = "../mivl/mivl-build" }
```

`build.rs`:

```rust
fn main() {
    mivl_build::compile("ui/main_window.mivl").unwrap();
}
```

`src/main.rs`:

```rust,ignore
mivl::include_modules!();

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut main_window = MainWindow::new()?;
    main_window.set("title", "Hello MIVL")?;
    main_window.run()?;
    Ok(())
}
```

MIVL files are **embedded and validated at build time, then interpreted at
runtime**. The macro generates Rust wrappers; it does not precompile UI markup
into native widget machine code or dynamically make all Rust functions available.
Register `#[mivl::command]` functions explicitly before calling `run()`.
