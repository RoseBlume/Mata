# mata-3d

Reusable, CPU-based, **true 3D** renderer targeting `mata_win::Framebuffer`.
It does not create windows, implement game physics, use `mata-ui`, require a GPU,
or add OpenGL/Vulkan/wgpu dependencies. On Linux the default `x11` feature
forwards to MataWin; use `--no-default-features --features wayland` for Wayland.

## Main types

| Type | Role |
| --- | --- |
| `Vec3` | 3D vector, dot/cross products and normalization |
| `Aabb` | Axis-aligned cuboid, circle-footprint and ray intersection helpers |
| `Camera` | Position, forward/right/up orientation, projection focal scale and near plane |
| `Rasterizer` | Clipping, Z-buffer, lighting/fog, indexed triangle and primitive rendering |
| `Mesh` | Indexed vertices and triangles, built-in cube/octahedron and OBJ parsing |
| `Transform` | Translation, scale and Euler rotation for each mesh instance |
| `Material` | Solid argb color, emissive or shaded, optional two-sided faces |
| `RenderStats` | Triangles rasterized, pixels written |
| `Model`, `ModelPart` | Editable OBJ + MTL models with per-part materials |

## Usage

```rust
use mata_3d::{Camera, Material, Mesh, Rasterizer, Transform, Vec3};
use mata_win::Framebuffer;

let mut fb = Framebuffer::from_color(960, 600, 0xff091827);
let mut graphics = Rasterizer::new();
let camera = Camera::new(
    Vec3::new(0.0, 1.0, -3.0),
    Vec3::new(0.0, 0.0, 1.0),
    Vec3::new(1.0, 0.0, 0.0),
);
graphics.begin_frame(&fb, camera);
graphics.mesh(&mut fb, &Mesh::cube(),
    Transform::from_translation(Vec3::new(0.0, 1.0, 3.0)),
    Material::lit(0xff22ccff));
```

The renderer **does not clear the framebuffer's color pixels** at the start of
`begin_frame`. That means you can draw a gradient/sky/background after calling
`begin_frame` and before submitting geometry. Call `clear_depth()` to draw a
view-space weapon over world geometry while keeping the framebuffer colors.
The depth vector is reused across frames and only grows/resizes when resolution
changes; triangles clip against the near plane and viewport, avoiding huge
out-of-bounds writes. `mesh()` caches rotation trigonometry per object and does
not allocate temporary transformed vertex arrays.

`Mesh::from_obj` imports geometry-only OBJ strings. For authored models with
multiple colours and emissive surfaces, `Model::from_obj_mtl` also imports
`usemtl`, `newmtl`, `Kd`, and `Ke` and draws a separate material per group.
UV texture sampling, normal maps and skeleton-based animations remain outside
the current renderer.

## Build / test

Place `mata-3d`, `mata-neon-run`, and your **split** `mata-win` crate in the same
workspace. Run:

```sh
cargo check -p mata-3d
cargo test -p mata-3d
cargo run -p mata-3d --release --example offscreen_cube
cargo run -p mata-neon-run --release
```

The offscreen example writes `mata3d-demo.ppm` with no window required.
Tests cover geometry, OBJ parsing, near clipping, depth, framebuffer resizing
and an indexed mesh actually drawing visible pixels.

## Multi-material models (OBJ + MTL)

The renderer now includes `Model` / `ModelPart` in addition to `Mesh`. This
allows real multicolour 3D objects without requiring a texture stack:

```rust
use mata_3d::{Model, Material, Rasterizer, Transform, Vec3};
use mata_win::Framebuffer;

let model = Model::from_obj_mtl(
    include_str!("../../mata-neon-run/assets/models/enemy_runner.obj"),
    include_str!("../../mata-neon-run/assets/models/enemy_runner.mtl"),
).expect("valid model");
// After Rasterizer::begin_frame:
model.draw_world(&mut rasterizer, &mut framebuffer,
    Transform::at_scale(Vec3::new(4.0, 0.0, 6.0), 1.0), None);
```

Supported OBJ records: `v`, `usemtl`, `f` (including quads, polygons,
negative/positive vertex indices and the vertex portion of `f v/vt/vn`).
Supported MTL records: `newmtl`, `Kd` diffuse and `Ke` emissive. The importer
splits the model into compact indexed submeshes grouped by material, and the
renderer shades each submesh separately. The `Model::draw_view` method renders
camera-space meshes (such as first-person weapons) after clearing the depth
buffer. It supports camera-space transformations and face lighting without fog.

**Not supported yet:** UV texture sampling, skinning, bone weights or real-time
shadows. OBJ files from Blender should be exported with material names and
plain polygon geometry; using `.mtl` colours is sufficient for Neon Run.

The new `model` module contains unit tests for material parsing, polygon
triangulation, negative OBJ indices and invalid geometry. The model-based game
also tests that all embedded assets are loadable and have glowing parts.

## Windows Intel GPU rasterization

The `intel_gpu` module (Windows only) adds a Direct3D 11 triangle rasterizer
without introducing any Cargo dependency. It exposes `IntelGpu::from_hwnd`
and `IntelGpu::present_geometry` for applications with a MataWin window.

The existing `Rasterizer` can record screen-space depth-tested triangles by
calling `set_gpu_capture(true)` before rendering, and `take_gpu_frame()` after.
The output `GpuFrame` preserves first-person depth-clear boundaries. This
replaces the expensive software **pixel** loops while keeping all authored
geometry, face-based lighting, fog, and CPU-side culling. The GPU also composites
the HUD and presents the frame.

**Scope:** the CPU still transforms and shades vertices. This is not yet a
fully GPU-transformed/indexed mesh renderer. Hardware DXGI adapters from Intel,
AMD, NVIDIA and others can be selected on Windows. The CPU renderer remains
available on all operating systems.

Run `cargo test -p mata-3d --no-default-features` for platform-independent
command capture validation. A Windows machine with Intel graphics is required
to verify device initialization, shader compilation, graphics output, resize,
and fallback.

## Intrinsic function-table integration

The D3D11 shader code is generated with the first-party `mata-gpu-intrinsics` HLSL dispatch table. This validates `mix`/`saturate` operations before GPU startup, and the generated HLSL is compiled on Windows by the existing Direct3D 11 renderer. See `../GPU_DISPATCH_INTEGRATION.md` for details, how to test, and the distinction between shader emitters and native CUDA/OpenCL/HIP execution.

## Intel iGPU shared-memory bandwidth optimizations

The D3D11 geometry path uses packed 16-byte vertices, recycles native vertex
streams, and can use `present_geometry_procedural` to generate the Neon Run sky
on the GPU instead of uploading a full-screen background image. See
`../INTEL_IGPU_PERFORMANCE.md` for performance details and caveats.
