# Molecule viewer changes

## Camera-distance sphere LOD

The application reuses five immutable sphere meshes from `src/lod.rs` (8, 96,
320, 768, and 1,728 triangles). Sphere mesh selection is based on apparent
on-screen radius rather than a hard-coded number of atoms. It therefore responds
both to zoom and to each atom's depth in the model. The existing model modes,
chain selection and highlighted residues are retained. The same sphere meshes
are reused by the cell and schematic virus views.

A larger viewer changes the projected size of an atom and may increase its
polygon budget; distant, small atoms use simpler meshes to save CPU work. The
meshes are constructed once when the viewer is created, not per atom or frame.

## Controls

* **F11** toggles the application window fullscreen on Windows. `mata-win`'s
  current `Key` enum doesn't have an F11 variant, so the Windows implementation
  polls the Win32 virtual key while the app has foreground focus, with
  edge-triggering to prevent repeated toggles. It preserves window placement
  and decorations when exiting fullscreen.
* **Ctrl+Shift+F** is an additional cross-platform keyboard shortcut to toggle
  app fullscreen through the same existing window-management path. On Linux X11,
  that path needs the optional `wmctrl` utility; Wayland needs native fullscreen
  support in `mata-win` and can't use `wmctrl`. F11 on non-Windows also requires
  mata-win to expose a physical function-key event.
* **FULL VIEW [F]** in the upper-right corner of the 3D, cell, and virus
  viewers expands the viewer within the current application window, hides the
  navigation/header/footer, and docks information on the right.
* **F** toggles that view when the viewer has keyboard focus (click or drag in
  it first). **ESC** exits the expanded viewer. The button also switches to
  **EXIT FULL VIEW** and restores the prior layout when clicked.
* F, Ctrl+Shift+F, and native Windows F11 are edge-triggered to avoid rapid toggles from keyboard repeat.
* F11 and F can be combined: first go application fullscreen, then expand the
  viewer with its inspector alongside.

The renderer accepts viewports up to 4096x2304 instead of the previous
2400x1600; this avoids an empty scene on standard 3840x2160 fullscreen
monitors. Very large viewports still require substantial CPU and memory.

## Build and test

From the parent Mata workspace, with sibling `mata-win`, `mata-3d`, and
`mata-fonts` crates present:

```
cargo test -p mata-molecule
cargo run -p mata-molecule --release
```

No new crate dependencies were added; the existing Windows crate gains its
`Win32_Graphics_Gdi` feature for monitor bounds. The archive is a standalone
project subtree and doesn't include its sibling path dependencies. Accordingly,
compilation cannot be validated from this zip by itself.
