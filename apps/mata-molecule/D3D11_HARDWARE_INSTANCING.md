# Native D3D11 hardware instancing (atoms + bonds)

This update replaces the old CPU-expanded per-atom and per-bond triangle batches with **real** Direct3D 11 `ID3D11DeviceContext::DrawInstanced` calls, while retaining all original molecular data and the CPU software renderer.

## What changed

- `mata-3d::GpuFrame` has two additional, independently cached instance streams: `GpuSphereInstance` (**32 bytes per atom**) and `GpuBondInstance` (**52 bytes per bond**).
- `Rasterizer::capture_sphere` and `Rasterizer::capture_bond` transform/project **only the center or two endpoints on the CPU**. Viewport clipping, near-plane crossing fallback, software rendering, and frustum culling remain supported.
- `mata-3d/src/intel_gpu.rs` allocates shared shared unit geometry: a **96-triangle unit sphere** and a **16-triangle unit cylinder**. These unit-mesh vertex buffers are created once on first use and retained, not regenerated for each frame. Atom and bond transform/colors use persistent, grow-only dynamic instance buffers. Each stream has its own generation-aware upload cache.
- HLSL `GSI` / `GBI` vertex shaders expand each instance on the GPU; `PSI` / `PBI` shade it on the GPU. Native D3D11 input layouts use slot 0 for the shared mesh (per vertex) and slot 1 for instance records (per instance). There are just **two instanced draw calls** per visible viewport (one for bonds and one for atoms), even with thousands of distinct pieces of geometry.
- Molecule Studio maps instances into its actual viewport NDC region; its D3D11 scissor, reverse-depth buffer and alpha compositing continue protecting surrounding controls.
- `view.rs` replaces atom and bond geometry submission in both simple-molecule and macromolecule/all-atom paths, including selection highlights and CA traces. `virus_view.rs` uses the same instancing for beads/spikes and the segmentation cylinders. `cell_view.rs` uses it for tubular wire/segment geometry; **ellipsoidal organelles, ribbon segments, generated molecular orbital meshes, density surfaces and other complex geometry remain on the existing triangle path**.
- `render_budget.rs` allows more real atom/bond representations on GPU while retaining Economy/Auto/Detail limits, screen-space proxy selection, and CPU-specific limits.

## Boundaries and visual differences

Instance spheres use one fixed low-polygon 96-triangle mesh; unlike the previous software path, they do not use the five-tier `SphereLods` quality range. Under high magnification, the fixed sphere may appear less smooth than the Detail-mode software rendering. Instance cylinders use eight sides. Per-pixel lighting is not guaranteed to exactly match the CPU shading/fog path. These are rendering approximations only; **atomic coordinates, bond orders, annotations, and imported data are never modified**.

The shader still receives CPU-projected centers/endpoints; **GPU work includes local unit-mesh transformations, native rasterization, depth tests and lighting**. This update does not claim entire-scene GPU-side culling, camera-motion reprojection or GPU skinning.

## Windows testing

Requires your sibling `mata-fonts` crate; no new dependencies were added. On Windows, from the containing workspace, run:

```powershell
cargo test --manifest-path mata-3d/Cargo.toml --no-default-features
cargo test --manifest-path mata-molecule/Cargo.toml --no-default-features
cargo build --manifest-path mata-molecule/Cargo.toml --release --no-default-features
```

Open a large PDB/mmCIF protein or a file-backed virus. Toggle **G** for D3D11 vs software, and **P** for Economy/Auto/Detail. Verify rotation, panning, zoom, selection highlights, bonds, clipping against UI panels, and idle performance. Check three cameras: an overview, a close-up and near-plane clipping. The app should fall back to CPU if D3D initialization, shader compilation, or presenting fails. Run FPS comparisons with the exact same model, viewport, preset and camera motion.

**Verification status:** This source was modified and archive-checked here, but is **not Windows-compiled or GPU-executed** in this environment; a Windows Rust toolchain and device run are required to confirm driver and shader behavior and measure actual gains. `mata-fonts` is not bundled because it was not available in the provided archive.
