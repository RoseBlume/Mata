# Student-laptop rendering profile (Mata Molecule Studio)

This source update builds on the previous D3D11 UI compositor. It does not install new crates or change imported data or saved files.

## User controls

- **P** in any 3D viewer cycles **Auto** (default), **Economy**, and **Detail**.
- **G** toggles D3D11 (Windows) / software fallback, as before.
- **Auto** targets integrated GPUs and common four- or six-core laptops. If rendering slows, existing frame-feedback logic progressively reduces geometry detail and now reduces the number of displayed markers as well.
- **Economy** uses tighter atom/bond/ribbon limits and lower-cost MRC/MAP ray sampling. Use it for weaker machines or high-resolution virus/cell datasets.
- **Detail** increases the budgets and microscopy quality for careful close inspection. It disables automatic sphere-quality downgrade but is not guaranteed to be real-time on very large structures.

## Implementation

1. `render_budget.rs` places an approximate total triangle allowance around all-atoms rendering. Heavily overlapping points are grouped into projected screen tiles **and element-color classes**, with a real frontmost source atom chosen as visual representative. It does not generate artificial atomic coordinates. Samples remain bounded and spread across the image if the candidate groups exceed the requested cap.
2. Bond rendering is uniformly thinned as needed in dense scenes, avoiding millions of cylinders. Original connectivity is unchanged in memory and when saved. Selected residues/atoms retain their full structure in focused inspection.
3. Very large ribbon views automatically fall back to the faster real C-alpha trace at low display detail; moderate-size ribbons retain the original estimated secondary-structure geometry. Backbone trace resolution is bounded.
4. The pre-existing five cached sphere meshes are chosen with a per-scene triangle budget. The current pixel footprint and the count of **drawn** markers matter, so zooming in on a smaller visible region restores finer atom spheres.
5. MRC/MAP surface modes use different sampling spacings and ray-march lengths for Economy / Auto / Detail. The original volume values and voxel inspection path are not downsampled or changed.
6. `GpuFrame` receives a monotonically increasing generation ID for completed GPU captures, allowing D3D11 to skip the large vertex-buffer `Map`/upload on UI-only repaints. Manually constructed or changed command buffers must use `generation=0` to disable this cache. The existing D3D11 hardware depth/rasterization and CPU fallback remain.

## Approximate rendering allowances

These are heuristics, **not a global hard count** (extra selection overlays, special meshes and multiple bonds can exceed them).

| Mode | Software | D3D11 | Approx. max source-atom proxies |
| --- | ---: | ---: | --- |
| Economy | 30,000 triangles | 55,000 triangles | budget / 18 |
| Auto | 75,000 | 135,000 | budget / 18 |
| Detail | 140,000 | 300,000 | budget / 18 |

During rotation/panning the allowed budget is reduced by one third; Auto and Economy also respond to measured redraw times. Actual triangulation depends on screen-space sphere LOD, contour modes, bonds, and camera placement. The D3D11 backend **does not yet implement true hardware instancing or GPU-side atom transformation**; it continues to transform vertices on the CPU, so there will still be limits on inexpensive hardware.

## Run and evaluate

From the Mata workspace containing this project and your existing `mata-fonts` dependency:

```powershell
cargo check -p mata-molecule
cargo test -p mata-molecule
$env:MATA_RENDER_STATS="1"
cargo run --release -p mata-molecule -- --open path\to\your-large-protein.cif
```

Compare the same view size and structure in Auto, Economy and Detail. Test both D3D11 and software fallback, rotating and panning, full-atom display, ribbon view, residue focus, virus protein inspection and MRC surface/MIP modes. Check that exports preserve original atom counts/coordinates. For power/heat, leave D3D11 VSync on (default); do not disable it in ordinary use.

## Limits / safety

- Performance decimation affects **display only**. An overview is not a complete representation of all atom/bond geometry. Use Detail and focused residue inspection for precise visual review; original files and calculations still use complete data.
- On very large datasets, up-front parsing and retained source arrays still require RAM; this is not out-of-core streaming.
- Automatic visual simplification cannot guarantee a specific FPS or a particular temperature/power draw. Actual performance must be profiled on the target laptop.
- This package was prepared without a Rust compiler, Windows graphics device or the external `mata-fonts` crate. Compilation, GPU tests and measured FPS remain unverified.
