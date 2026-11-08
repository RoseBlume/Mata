# Molecule Studio rendering performance update

This patch addresses the software rasterizer's heavy geometry load rather than assuming a particular GPU vendor.

## Changes

- **Screen-space LOD:** the top-detail 1,728-triangle sphere is now reserved for a projected atom radius of at least 95 render pixels, instead of 25. Intermediate cutoffs are 5, 15, and 38 pixels.
- **Per-frame geometry allowance:** `SphereLods::configure` caps the atom-sphere triangle cost (initial allowance 200,000 triangles across visible atoms, smaller during manipulation). One isolated amino acid retains finer geometry than thousands of atoms.
- **Frame-cost feedback:** after >44 ms draw times, sphere detail is progressively reduced, down to a bounded quality factor. It only recovers after repeated fast frames (<19 ms), avoiding flicker.
- **Dynamic viewport resolution:** scales molecule/atom rasterization to 65% linear resolution during drag and 66–90% if consistently overloaded; UI elements stay at native framebuffer resolution. When frames are fast and the camera is stationary, 3D rendering restores native resolution.
- **d/f orbital cache:** orbitals are oriented only when the camera angles change. Individual bonded atoms reuse the already built triangle buffers via mesh transforms (avoiding repeated per-atom mesh allocations and triangle copying).
- **Reusable cylinder geometry:** most displayed bonds reuse a scratch mesh instead of creating new vertex/index allocations for every bond each frame.
- **No needless pointer redrawing:** moving the mouse over a stationary 3D canvas no longer forces rerasterization. Controls still repaint when hovered.
- **Measurements:** `MATA_RENDER_STATS=1` prints average draw times and current dynamic LOD quality every 60 rendered molecule frames to stderr. Measured draw time includes the application's redraw/present path, not only 3D raster time.

## Mata GPU intrinsics

The supplied `mata-gpu-intrinsics` crate is bundled at `../mata-gpu-intrinsics` and added as a path dependency, with its default features disabled. **It generates shaders, not GPU execution.** The current `mata-3d` software rasterizer has no programmable GPU command submission API in the provided molecule project, so claiming GPU-accelerated rendering would be incorrect.

The `gpu_shaders` integration exposes **shader source generation** for WGSL, GLSL, HLSL and an Intel OpenCL C per-atom projected-radius kernel. To export the sources, run:

```bash
cargo run --release -p mata-molecule -- --export-gpu-shaders gpu-shaders
```

This requires the normal Mata workspace layout, including `mata-win`, `mata-3d`, and `mata-fonts`. The shader outputs are ready for compilation/dispatch by a compatible GPU backend, **but are not used by the existing CPU renderer**. Execution would require adding GPU resources, pipeline/shader compilation, and device launch to `mata-3d`; no hardware speedups are claimed here. Exporting source needs no GPU or GPU SDK.

## Build / benchmark

Run in your Mata workspace:

```bash
cargo test -p mata-molecule
cargo run --release -p mata-molecule -- --open path/to/structure.pdb
```

On PowerShell, enable timing:

```powershell
$env:MATA_RENDER_STATS="1"
cargo run --release -p mata-molecule -- --open path/to/structure.pdb
```

On Linux:

```bash
MATA_RENDER_STATS=1 cargo run --release -p mata-molecule -- --open path/to/structure.pdb
```

Compare steady camera movement, clicking/focusing a residue, and the S/P/D/F overlay at the same viewer size. Total triangle and draw performance also depends on `mata-3d`'s rasterization internals.

## Constraints and limits

- Geometry caps apply to atom spheres; complex ribbons and density/surface renderers do not yet share the same budget.
- Changing render resolution changes the antialiasing/sharpness of the molecule portion during interaction.
- Hardware acceleration remains unimplemented in the provided CPU rendering backend. Shader-source generation is an integration building block, not runtime acceleration.
- Tests were added for budgets, frame feedback, viewport scaling/clipping and shader emission. The complete Mata workspace dependencies and Rust toolchain were not provided, so full Rust compilation and on-device benchmarks have not been run here.
