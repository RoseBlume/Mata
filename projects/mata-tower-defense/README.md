# AEGIS // GRID DEFENSE

A complete, asset-free native tower defense game using your exact uploaded **mata-win** and **mata-fonts** crates. The tower defense simulation, visuals, menu interface, input, and game loop are custom Rust. There are **no other direct dependencies** in the game crate.

## Getting started

Open a terminal in the *root of this archive*, containing `Cargo.toml`, `mata-tower-defense`, `mata-win`, and `mata-fonts`:

```sh
cargo run --release -p mata-tower-defense
```

Rust stable with Edition 2024 support (Rust 1.85 or later) is required. On Linux, the existing `mata-win` backend requires a working Wayland or X11 session. On Windows, the included crate uses its existing Windows backend. Rust's target platform may require the native build environment already required by `mata-win`.

The game itself uses **only** `mata-win`, `mata-fonts`, and `std`. Platform dependencies of `mata-win` are left unchanged from your archive.

## Gameplay

Defend your power core across 20 escalating waves. Hostiles follow a fixed zigzag road; build on empty terrain and earn credits for destroying enemies. You begin with 330 credits and 20 integrity. Defeating a wave awards additional credits. Upgrade or sell towers to react to fast, heavy, and armored foes.

| Tower | Initial cost | Purpose |
|---|---:|---|
| Sentinel | 85 | Rapid-fire general defense |
| Artillery | 145 | High-damage area explosions |
| Cryo Array | 115 | Slows enemies and deals damage |
| Prism Beam | 210 | Fast, instant-hit energy weapon |

Towers can be upgraded up to level 4, improving damage, attack speed, and range. Selling refunds 70% of total invested credits.

### Controls

| Input | Action |
|---|---|
| Left-click shop card / keys 1–4 | Choose a tower |
| Left-click empty map tile | Place chosen tower |
| Left-click placed tower | Inspect it |
| U / Upgrade button | Upgrade selected tower |
| X / Sell button | Sell selected tower |
| Space / Launch button | Start the next wave |
| P / Pause button | Pause / resume |
| Tab / 2X button | Toggle simulation speed |
| Right-click / Escape | Cancel placement / deselect |
| R | Restart after victory or defeat |

## Architecture

* `src/game.rs` — frame-rate-independent game rules, enemies, pathfinding along a authored route, projectiles, damage, credits, 20 waves, win/loss logic, and unit tests.
* `src/paint.rs` — framebuffer rasterizer (shapes, lines, gradients, blending) and cached TrueType glyph rasterization via `mata-fonts`. On machines without a supported installed TTF, a built-in bitmap fallback is used.
* `src/view.rs` — native game HUD, tile field, enemy and tower visuals, firing effects, and input feedback, all drawn without external image assets.
* `src/main.rs` — native `mata_win::Window`, event routing, fixed-step update loop, input mapping, game pause/speed, and frame pacing.

## Checks

```sh
cargo test -p mata-tower-defense
cargo check -p mata-tower-defense
```

**Note:** The archive supplied by the user also contained `mata-ui` and `mata-audio`, but neither is necessary for this software-rendered game. `mata-ui` declares optional path dependencies on `mata-image` and `mata-video`, which were not included in the upload, so it is deliberately not included in the workspace. The game does not need them to compile.

**Android:** A native application entry point and tap-based controls are included, using MataWin touch events. Android packaging, signing, device UI sizing, and on-device testing are not included.

### Native-resolution rendering and resize alignment

The game keeps a **1440 × 900 logical coordinate system** for gameplay and UI layout,
but allocates its framebuffer at the window's current `Window::client_size()` and
re-rasterizes all sprites, lines, shapes, and font glyphs at that native resolution.
It preserves the logical 16:10 aspect ratio; resizing to a different aspect ratio
adds non-interactive letterbox bars instead of stretching the text or graphics.

Hover and click positions use the same viewport as drawing. Never use
`framebuffer_size()` to convert mouse positions after resizing: it can describe the
previously presented buffer rather than the resized native client area. No additional
dependencies are needed. At very small window sizes, text detail is necessarily limited
by the available physical pixels.

