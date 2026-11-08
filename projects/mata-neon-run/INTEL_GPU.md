> **Update:** Automatic multi-vendor Direct3D 11 adapter selection is now implemented on Windows. See `GPU_AUTO_SELECTION.md` in the workspace root. Sections below describing Intel-only selection are historical.

# Intel iGPU compositor (Windows)

This is a **native D3D11 GPU scene presentation/composition backend**, not a GPU
triangle rasterizer. `mata-3d` remains the CPU triangle rasterization engine.

## Build and enable

Use the existing game dependencies and the **Windows SDK** (standard MSVC
Windows development tools):

```powershell
cargo check -p mata-neon-run
cargo run -p mata-neon-run --release
```

When a real Intel graphics adapter is available, the game tries to initialize
D3D11 at startup. It prints its selected adapter name to stderr. On failure,
MataWin presentation continues normally. To force the fallback:

```powershell
$env:MATA_INTEL_GPU = 'off'
cargo run -p mata-neon-run --release
```

Press **F5** to toggle the compositor while the game is running. F2 continues
to control the *CPU scene* resolution. Every visible enemy still uses its full
4K articulated mesh: no distance-based quality reduction is enabled.

## Architecture

- DXGI enumerates GPU adapters and requires Intel vendor ID `0x8086`.
- D3D11 creates a native swapchain associated with the existing MataWin HWND.
- `mata-3d` renders to the game's existing `Framebuffer` at chosen resolution.
- HUD is rendered to a transparent **native-resolution** `Framebuffer`.
- D3D11 dynamic BGRA8 textures are uploaded with row-pitch-aware copies.
- One fullscreen GPU draw scales the scene and composites the HUD.
- Texture and pipeline objects are reused across frames; swapchain is rebuilt
  after a window resize. On graphics errors the game falls back to MataWin.
- No wgpu, winapi, windows, pollster, raw-window-handle or other Rust crates
  are introduced.

## Important limitations and checks

1. You need `d3d11.lib` and `dxgi.lib` from the Windows SDK. The optional
   shader compiler DLL (`d3dcompiler_47.dll`) is loaded at runtime. If it is
   unavailable, GPU setup fails safely and MataWin continues presenting.
2. This release's Windows backend has **not been compiled/tested on real Intel
   hardware** in this environment. Test window resizing, minimizing/restoring,
   Alt+Tab, F5 transitions, 3.4x sniper scope and all story screens.
3. Intel Arc discrete GPUs also use the Intel vendor ID. Vendor selection
   alone cannot guarantee that an Intel adapter is integrated.
4. The SDK GPU path does **not** accelerate triangle transformation or
   rasterization. That requires a separate `mata-3d` GPU implementation.
5. Linux and non-Intel systems retain the existing software/MataWin presentation.
6. GPU composition may or may not outperform the previous presenter, depending
   on frame sizes, Intel generation, GPU/CPU memory bandwidth and vsync.
