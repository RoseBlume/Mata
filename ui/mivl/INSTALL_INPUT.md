# MIVL native input integration

MIVL links directly to `mata-ui` and `mata-win`; `native-ui` is **not** a
Cargo feature. Pointer, wheel, and touch event variants and forwarding are
integrated into `ui/mata-ui` in this workspace. Do not apply an old
`mata-ui-input.patch` or overwrite the current widget sources.

From the workspace root:

```bash
cargo check -p mata-ui -p mivl
cargo test -p mivl
cargo run -p mivl --bin mivl -- ui/mivl/examples/virtualized.mivl
```

The Windows backend may need additional WM_POINTER integration for direct
touchscreen contacts; the legacy patch in `patches/mata-win-windows-touch.patch`
is provided as reference but is not applied automatically. Mouse input and
wheel input are forwarded on supported platforms without extra features.
