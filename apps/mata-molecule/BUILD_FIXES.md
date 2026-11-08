# Build fixes for the performance update

This revision fixes the errors reported from `cargo build -p mata-molecule`.

1. **E0277/E0308 in `orbitals.rs`**: The closure that constructs d and f
   orbital surface groups now uses `collect::<Vec<Surface>>()`. Without a
   concrete collection type, type inference incorrectly settles on the
   unsized `[Surface]` slice in the constructor.
2. **E0599 in `main.rs`**: Removed both uses of nonexistent `Key::F11`.
   The Windows build detects F11 with `GetAsyncKeyState(VK_F11)` while this
   window is foreground, and edge-detects it in the event loop. It uses the
   existing `windows` dependency with one additional feature:
   `Win32_UI_Input_KeyboardAndMouse`. This does **not** add another crate.
3. Added `Ctrl+Shift+F` as a fallback on all targets using the Key variants
   already present in MataWin. The Linux X11 fullscreen code still needs
   `wmctrl` installed; native Wayland app fullscreen must be implemented in
   the windowing crate before that mode can toggle.

Try in your parent Mata workspace:

```bash
cargo check -p mata-molecule
cargo test -p mata-molecule
cargo run --release -p mata-molecule
```

**Validation limitation:** Rust and the sibling path dependencies (`mata-win`,
`mata-3d`, and `mata-fonts`) aren't available in this environment, so a complete
build cannot be claimed here. All reported invalid enum uses are removed.
