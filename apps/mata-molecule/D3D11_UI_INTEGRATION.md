# Direct3D 11 viewport integration — Mata Molecule Studio

## Included changes

* On Windows, attempts native DXGI hardware adapter selection (Intel, AMD, NVIDIA, other D3D11-capable DXGI adapters), same as mata-neon-run. The D3D11 renderer is initialized with MataWin's own HWND. If initialization fails, the application continues on the existing CPU renderer.
* Molecule, generated cell and schematic virus viewers capture illuminated and transformed scene triangles directly from `mata-3d::Rasterizer` while skipping software triangle rasterization. Native D3D11 fills and depth-tests them in the appropriate part of the Studio window. Volumetric MRC/MAP/NRRD microscopy views keep their existing CPU rendering; they are **not** converted to GPU raymarching.
* D3D11 scissor clipping prevents world geometry from overwriting neighboring UI panels. The editor itself (toolbar, sidebar, text, labels, dialogs) remains drawn by MataWin, with a selectively opaque HUD composited above GPU geometry.
* The existing viewer framebuffer cache is paired with a retained GPU triangle command cache: redrawing chrome need not reconstruct the same molecular scene. Reallocations are reduced by recycling the capture buffer. Software atom/bond rasterization is bypassed while the GPU path is active.
* The renderer choice is displayed in the application footer. Press **G** while a 3D molecule, cell, or virus viewer is active to switch between D3D11 and CPU. Pressing G after initialization failure attempts to create the GPU device again.
* On a D3D11 present/resize error, the device is released and the current frame is immediately re-rendered on the CPU. Linux continues to use the parallel CPU renderer; there are no additional Cargo dependencies.

## Runtime configuration

Windows PowerShell examples:

```powershell
$env:MATA_GPU_BACKEND = 'auto' # or d3d11, intel, amd, nvidia, cpu
$env:MATA_GPU_VSYNC = '1'     # '0' disables vertical-sync pacing
cargo run --manifest-path mata-molecule/Cargo.toml --release --no-default-features
```

Set `MATA_GPU_BACKEND=cpu` to ensure D3D11 is not initialized on startup. For interactive comparison, press G in a supported 3D viewer to enable/disable hardware rendering. Dynamic shader compilation uses the system's `d3dcompiler_47.dll`. No NVIDIA CUDA or AMD HIP SDK is required.

## Verification

Use the original workspace with the existing `mata-fonts` crate alongside the included crates. From the parent directory:

```sh
cargo test --manifest-path mata-3d/Cargo.toml --no-default-features
cargo test --manifest-path mata-molecule/Cargo.toml --no-default-features
cargo run --release --manifest-path mata-molecule/Cargo.toml --no-default-features
```

Windows manual GPU tests:

1. Open the molecule viewer with multiple PDB/mmCIF chains and rotate/zoom/pan; observe that the 3D viewport changes without redrawing 3D fragments outside its bounds.
2. Enable the ribbon mode and atom-selection annotations. Check overlay text, footer, controls and toolbars remain visible above atoms and are clickable.
3. Select generated cell and schematic virus tabs; try selection, cutaway, zoom and window resizing.
4. Load an imported microscopy volume and verify it still uses the correct CPU volume renderer, without stale triangles from another tab.
5. Compare `MATA_GPU_BACKEND=cpu` against `auto`. Press G repeatedly and resize/fullscreen. Force device loss if a test environment allows it; the app should display a software frame after a GPU error.
6. On a host without a working D3D11 driver, verify CPU fallback, no crashes, and an error in the console.

## Deliberate limitations

* D3D11 performs **triangle fill, GPU depth testing, composition and presentation**, not true GPU-side atom instancing or mesh vertex transformations. Those remain CPU-side in `mata-3d`. The existing protein atom-count safeguards remain unchanged.
* The GPU viewport renders at full physical size, while the CPU renderer keeps its adaptive-resolution and parallel fallback. GPU capture/projection may be internally bounded for extremely large viewports, but mapped triangles rasterize at native window resolution.
* Windows GPU compilation and on-device validation were not possible in the present environment (no Rust compiler or Windows graphics session). No FPS improvement has been measured or claimed. The included static changes and tests are prepared for local verification.
* The supplied project does not bundle `mata-fonts`, which is still required as an existing path dependency for the application to build.
