# Molecule Studio rendering performance update (2026-10-16)

## Implemented in this package

1. **Conservative world-space visibility culling:** `mata-3d::Rasterizer::sphere_visible` and `bounds_visible`; the molecule, cell and virus viewers skip off-screen atom/segment meshes before allocating or transforming triangles. Bounding-sphere intersections of the near/frustum planes are retained rather than cut off.
2. **No fixed 65-unit molecular cutoff:** `Rasterizer::max_distance: Option<f32>` replaces the formerly hard-coded limit. Existing game rasterizers retain `Some(65.0)` by default; molecular viewers explicitly disable it.
3. **Parallel software rasterization with reusable band buffers:** `mata-3d::parallel::ParallelSoftware` records a scene once and replays depth-tested triangles in non-overlapping CPU bands. It starts with one thread, engages multiple threads after sustained expensive/dense frames, and gradually reduces worker count after sustained fast frames. It never exceeds logical cores minus one (or one on single-core machines). `MATA_RENDER_THREADS=N` provides a starting count for testing.
4. **Parallel microscopy isosurface ray marching:** density rays run across independent rows using scoped threads. No voxel data is modified or copied to workers; nearest-ray samples, threshold interpolation, and gradient-based surface lighting follow the previous algorithm.
5. **Cross-viewer framebuffer caching:** molecule, generated cell, virus and experimental microscopy canvases retain their latest render when the scene, selected component, viewport, camera and relevant settings match. UI redraws can composite the cached canvas without rerasterizing the scene. Reconstructions invalidate the molecule canvas explicitly.
6. **Reusable bond geometry:** bonds and cell/virus segments use a shared scratch mesh, avoiding repeated vertex/index vector allocation for every line segment.
7. **Existing adaptive resolution and spherical LOD** remain active. The frustum test is applied *before* submitting atom meshes, reducing work without discarding any scientific atom coordinate or modifying source files.
8. **CPU mesh instance API:** `Rasterizer::mesh_instances` takes shared geometry plus per-instance transforms, colors, and conservative bounds. It works with the existing GPU-capture API, but transforms instances on the CPU.
9. **Regression tests added:** frustum/distance checks, serial-versus-parallel depth rasterization, and serial-versus-banded microscopy surface rendering.

## What is not completed

- **True GPU hardware instancing and GPU-side transforms** are **not** implemented. `mata-3d` has Windows D3D11 triangle capture/rasterization facilities from Neon Run, but the Molecule Studio UI currently presents a `mata_win::Framebuffer`. Wiring D3D11 geometry into its multi-panel compositor requires GPU-side instance buffers, a vertex shader and proper presentation orchestration, and must be tested on Windows. Existing shader source export is not execution.
- The protein backbone/ribbon path retains the original sampling policy; the general molecule view retains its original 2,000-atom safeguard. Rendering every atom in a very large structure without a cutoff requires a spatially indexed streaming/LOD representation and validation of picking.
- The experimental microscopy surface is a *ray-marched* isosurface, not a persistent triangle mesh. The cached canvas accelerates unchanged views, not camera motion; volumetric isosurface mesh caching would need a new extraction and triangulation implementation.

## Files modified

- `../mata-3d/src/lib.rs`
- `../mata-3d/src/parallel.rs` (new)
- `src/view.rs`
- `src/cell_view.rs`
- `src/virus_view.rs`
- `src/main.rs`

## Verification

With the matching Mata workspace (`mata-win`, `mata-3d`, `mata-fonts`, `mata-gpu-intrinsics`) and a Rust toolchain, run:

```sh
cargo test --manifest-path mata-3d/Cargo.toml --no-default-features
cargo test --manifest-path mata-molecule/Cargo.toml --no-default-features
MATA_RENDER_STATS=1 MATA_RENDER_THREADS=2 cargo run --release --manifest-path mata-molecule/Cargo.toml
```

On Windows, omit `--no-default-features` if your normal configuration requires OS-specific features. Use `MATA_RENDER_THREADS=1` for a serial comparison, and test a molecule, a generated cell, and an imported MRC/MAP volume with identical camera/settings. **No performance multiplier is claimed:** the Rust toolchain and a Windows GPU are not available in this environment for compilation or device benchmarking.

This patch makes no new crate dependencies. It uses only Rust's standard library plus the existing Mata crates.
