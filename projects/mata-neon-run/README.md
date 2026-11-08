# MATA // OUTPOST ZERO — THE ECHO DIRECTIVE

A **true 3D** arcade shooter, implemented in Rust using `mata-win` for native windows/input/presentation and the new reusable
**`mata-3d` software rasterization library** for objects, camera transforms,
clipped triangles, reciprocal-Z depth, 3D meshes, lighting and distance fog. It is not raycasting, 2.5D wall rendering, or billboard enemies.

Five authored mission floorplans with interactive story objectives, cover that has real height, jumps, sprint/dash, five weapon types, magazine reload, enemy projectile combat, health/shield/ammo pickups, combo scoring, minimap and HUD. Robot enemies are intentionally stylized and non-graphic. Guns and enemies now use original 3D OBJ/MTL meshes embedded at compile time; the game requires no OpenGL, Vulkan, or Direct3D renderer. The `mata-3d` library supports indexed meshes with independent flat and glowing materials.

## Camera fix and new station environments

**Unrestricted mouse look**: On Windows (Win32 user32) and Linux X11 (libX11), `src/mouse.rs` now samples native pointer position while captured and immediately warps it back to the capture anchor. The cursor is hidden during play and restored when gameplay is paused or focus is lost. Pointer recentering avoids the edge-of-window camera lock without introducing any new Cargo dependencies. The game continues to use MataWin window input and exposes keyboard arrow look as an alternative. Pure Wayland (without XWayland) still uses the old CursorMoved fallback; implementing its true relative-pointer protocol requires support in `mata-win` itself.

The 3D map has an overhead **solid ceiling** with modular seams and structural ribs; the sky is no longer visible from station corridors. New reusable, source-controlled, low-poly OBJ/MTL assets include:

- Ceiling strip lights and hanging refinery lamps, with emissive panels and local floor light-pool cues.
- Rotating-vent style fan housings and large suspended air ducts.
- Bulkhead frames, two powered sliding airlock panels, and lock-status lighting; an exit cannot be used until its objective is complete.
- Interactive detailed terminal consoles, triple command desks, relay switchgear, witness data terminals, and the independent emergency uplink.
- Server racks with indicator strips, pipe bundles, coolant tanks, crates, chairs, desk workstations, wall monitors, warning beacons, wall signs, and floor grates.

In total: **21 new mesh designs and matching MTLs (42 asset files)** stored at `assets/models/env_*.obj` and `env_*.mtl`. They are embedded with `include_str!` and parsed once when the renderer is constructed; camera-distance culling limits draw cost. The deterministic `tools/make_environment_models.py` script reproduces the models with only the Python standard library. Lighting is implemented as emissive material plus stylized floor highlights, not real-time physically based illumination.

## LANCER-7 futuristic sniper replacement

Weapon slot **3** now loads a new, high-detail original game-prop mesh inspired by the three supplied reference photos: an angular light-coloured chassis, long dark forward assembly, large glass optic with adjustment details, skeletal stock, bipod and cyan illuminated inserts. The mesh is **9,472 triangles**, stored as `assets/models/gun_sniper.obj` and `gun_sniper.mtl`, with **21 flat/emissive materials**. A standalone rendered illustration is `assets/models/gun_sniper_preview.png`. Model geometry is decorative only and does not alter weapon statistics or firing simulation.

**Smooth scope animation:** hold **right mouse** while LANCER-7 is equipped. The model translates and rotates into the optic-aligned position, the world FOV interpolates toward its zoom level, aim sensitivity reduces smoothly, and the circular optical UI fades in at the end. Releasing right mouse reverses the transition; switching weapons resets the scope pose. The motion lasts approximately 0.26 seconds in and 0.22 seconds out and remains fixed-step (120Hz). The weapon's existing reload animation is retained.

The OBJ and MTL files are source-controlled. Regenerate them without Python packages:

```bash
python tools/make_futuristic_sniper.py
```

The current Mata software renderer uses per-face diffuse/emissive materials; it does not sample PBR texture maps. The sniper uses the original `Model::from_obj_mtl` code path; no new Cargo dependencies are needed.

## Five-mission story campaign

The three-wave arena is replaced by five independently laid-out 18 × 18 maps with connected passages and cover. ROOK-7 investigates a forged rescue beacon at the ASTRAEUS station:

1. **Outpost Zero:** Locate a blue security key and unlock the extraction hatch.
2. **Blackout Refinery:** Throw the two manual power relays.
3. **The Iron Vault:** Recover the physical vault key and disable every hostile guard.
4. **Zero Hour:** Operate three command relays, get the Director's key, and clear command security.
5. **The Last Transmission:** Cut three shield resonators, disable the Archivist's chassis and activate the separate evidence uplink.

Three optional evidence records per mission (=15 total) persist in an in-game journal. Story screens introduce the mission and close on an epilogue. The game retains its five weapons, class-specific ammo drops, damage, scope aiming and adaptive multithreaded renderer.

Use **E** on an objective once close enough. On-screen prompts identify interactive consoles, switches and doors. Use **T** to open the journal and **[ ]** to browse files. The exit changes color and slides its panels open when the objective is complete.

No changes have been made to `Cargo.toml` or its dependency list.


## Full 4K enemy models, off-screen culling and smarter enemies

Every enemy that intersects the camera's conservative 3D frustum now uses its **complete 12-part 4K OBJ rig**, at any visible distance. The previous two-enemy hero limit, resolution threshold, and range-based 4K/full/medium mesh switches are removed. Only 4K enemy parts are loaded by the runtime renderer; the original mesh tiers remain in the source tree as editable assets. `src/visibility.rs` rejects fully outside-frustum actors **before the walking rig is evaluated or geometry is submitted**. Frustum testing is deliberately conservative to prevent parts of an animated robot disappearing at image boundaries. A future task is to share `mata-3d`'s exact projection planes for tighter culling.

Enemies are no longer omniscient. An idle enemy scans until Rook is **within its class-specific detection distance, inside its horizontal vision cone, and unobstructed by solid walls or cover**. Being hit immediately alerts that enemy even if it was looking away. Once it has detected the player, it pursues, but if sight is lost it investigates only the *last known location*, uses 18 × 18 grid routing around obstacles, and eventually returns to idle. Enemies shoot only if they currently see the player and their firing line is clear; no firing through walls. Enemy AI and gameplay physics continue when the enemy is off-screen, but the offscreen rig performs no per-frame graphics work.

The current **`todo.md`** is the comprehensive graphics roadmap: visibility, shared scene work, triangle/tile rendering, hardware optimization, textures and materials, lighting, shadows, antialiasing, effects, animation, high-DPI HUD, asset pipelines and test criteria. Rendering *all* visible robots at full 4K mesh detail is substantially more demanding in crowded scenes; there is intentionally no automatic geometry downgrade to conceal that cost.

**No Cargo dependencies were added.** The Rust compiler and sibling Mata crates are not available in this artifact environment, so compilation and in-game FPS benchmarks remain to be done in your workspace.

## Install

Copy **both** `mata-neon-run` and `mata-3d` next to your existing **split** `mata-win` crate in your Mata workspace:

```text
Mata/
  mata-neon-run/
  mata-3d/          # new rendering library
  mata-win/
  mata-fonts/
  mata-image/
  mata-audio/
  mata-video/
```

Compile using:

```bash
cd mata-neon-run
cargo run --release
```

The source archive contains the game and expects your existing sibling `mata-3d`, `mata-win` and (for the default audio feature) `mata-audio` crates. Add `"mata-3d"` and `"mata-neon-run"` to your root workspace members list when needed. `mata-win` provides the window, events and framebuffer; `mata-3d` owns the software 3D pipeline. The game uses native OS pointer positioning on Windows and X11 to provide unbounded mouse look; MataWin still owns pointer visibility and game input events.

## Controls

| Key/mouse | Action |
| --- | --- |
| W A S D | Walk / strafe |
| Shift | Sprint |
| Space | Jump onto / over raised platforms |
| Q while moving | Dash (2 second cooldown) |
| Move mouse | Unlimited first-person look with recentering on Windows/X11 |
| Arrow keys | Look left/right/up/down without using mouse |
| Hold left mouse / J | Fire |
| E or Enter | Interact with consoles, switches, doors, keys, and uplink |
| T | Open recovered evidence journal |
| [ and ] | Browse recovered records |
| R | Reload magazine |
| 1 / 2 / 3 / 4 / 5 | Pulse Carbine / Arc Cannon / Sniper / Shotgun / Minigun |
| Esc | Pause / resume; quit after match finishes |
| Enter | Advance story; restart after win or retry current mission after defeat |
| F1 | Toggle control help |
| F2 | Cycle Auto / Fast 50% / Native 100% rendering quality |
| F3 | Show rendered triangle and pixel statistics |

## Notes

- The software 3D world is rendered at a fraction of the actual window size by default, then upscaled. The HUD stays sharp at native resolution. F2 cycles **Auto** (starts at 55%, adjusts between 40% and 80% based on frame timing), **Fast** (50%), and **Native** (100%). F3 shows frame rate, internal scale and triangle/pixel statistics.
- The cursor remains hidden in focused gameplay and reappears during pauses and on focus loss. Windows/X11 pointer recentering avoids the monitor-edge lock; native Wayland requires a dedicated relative-pointer API in its window backend.
- Software rasterization favors compatibility and no graphics API dependencies over high-end 3D performance. Release mode is strongly recommended.
- Game logic uses a fixed step and frame delta clamping.

## Source files

`mata-3d/src/lib.rs`: reusable math, mesh, transforms, camera, clipping and depth-tested triangle rasterization;
`mata-3d/src/model.rs`: OBJ/MTL material importer and multi-part model drawing;
`math.rs`: reexports shared Vec3/Aabb plus gameplay ray-sphere queries;
`campaign.rs`: five fixed floor plans, mission text and 15 evidence records;
`world.rs`: combat/AI, mission objectives, interaction, campaign progression and Archivist boss;
`render.rs`: complete 3D scene composition, enemies, weapons and boss;
`environment.rs`: ceiling panels, emissive fixtures and reusable 21-model scene prop catalogue;
`hud.rs`: native-resolution mission HUD, interactive prompts and story journal;
`mouse.rs`: desktop native cursor recentering and Wayland-compatible input fallback;
`main.rs`: adaptive render loop and MataWin Direct2D presentation.

## Library integration

The game actually renders through `mata-3d`: every box/quad uses its depth-tested
primitive calls; all pickups and projectiles use a cached indexed octahedron
mesh with per-instance transforms; both first-person weapons use different
camera-space OBJ models after a depth clear. Enemy bodies likewise use
embedded multi-material OBJ models with near/far detail selection. There is no
second independent 3D rasterizer left in the game.
No dependency on `mata-ui` is necessary for Neon Run.

### Included model

`assets/models/reactor.obj` is an original low-poly reactor mesh, loaded at
compile time with `include_str!` and parsed once by `Mesh::from_obj`. A stage
coloured rotating beacon appears above the station floor. It has no
collision or gameplay effects and does not require any runtime asset paths.

### Weapon and enemy models

The previous placeholder robot cuboids and gun boxes are replaced by original
flat-shaded, editable OBJ meshes. `Model::from_obj_mtl` parses them once during
`Renderer::new` and the game uses `Model::draw_world` or `draw_view` thereafter:

- **Runner:** slim orange pursuit bot with cyan visor, articulated legs and
  rear thruster details.
- **Striker:** broad magenta/violet brawler with long forward-mounted blades,
  sharp shoulders and glowing torso shield.
- **Tank:** heavy purple armoured walker with chest reactor, guarded sensor
  head and large limbs.
- **Pulse Carbine:** single-barrel graphite/cyan energy weapon, animated recoil,
  subtle walking bob and glowing muzzle flash.
- **Arc Cannon:** wide dual-barrel violet weapon with independent emitter glows,
  stronger muzzle flashes, recoil and bob.

Robots scan while idle; only enemies that have detected Rook or been hit pursue them. They preserve detailed articulation and full 4K geometry regardless of distance, provided any part intersects the camera frustum. No runtime distance-based enemy mesh swaps remain. Base collision, damage, pickups, weapon controls and dynamic pixel resolution scaling remain; mission progression and environments are substantially expanded.

All `.obj` / `.mtl` files live under `assets/models/` and can be opened in
Blender. The source-controlled developer tool `tools/build_models.py` recreates
these meshes; it is **not** used at runtime. `previews/` contains PNGs generated
by an optional Python preview script from the actual OBJ geometry.

## Restored sound effects (2026)

**Sound is now implemented, not merely documented.** The earlier original and
3D-model editions contained no audio output code, so they were silent. This
edition adds a bounded, polyphonic procedural sound engine in `src/sound.rs`.
It does not require external audio files or alter `mata-3d`.

Sounds include the Pulse Carbine, Arc Cannon, enemy projectiles and destruction,
hit confirmations, shield and health damage, pickups, footsteps, dash/jump,
reload/reload complete, sector starts/clears, victory and defeat, and UI clicks.
A quiet electronic ambience runs during active play.

The game uses one default-device Mata Audio output stream (WASAPI on Windows, ALSA on
Linux) with separate float/int16/uint16 sample conversion. The audio callback
consumes a bounded event queue and mixes up to 32 voices without dynamic audio
assets. The sound engine is enabled by default:

```toml
[features]
default = ["audio"]
audio = ["dep:mata-audio"]
```

**M** toggles mute/unmute. The HUD shows `SOUND ON`, `SOUND MUTED`, or
`NO AUDIO`. The terminal prints a confirmation with the selected output
format, or a specific initialization failure. If the sound device changes
while running, restart the game to reacquire the default output device.

Build/test:

```powershell
cargo check -p mata-neon-run
cargo test -p mata-neon-run --no-default-features
cargo run -p mata-neon-run --release
```

To build without an audio device or without a Linux ALSA development package:

```bash
cargo run -p mata-neon-run --release --no-default-features
```

On Linux, Mata Audio's selected output backend may require system audio
development headers. The release archive contains source, not a compiled binary.

## Five-weapon arsenal and new enemies

The game now ships five independently supplied weapons, each with a different
reloading animation, rate of fire, damage profile, reserve cap, and distinct
procedural firing sound. Every gun has its own magazine and reserve: switching
weapons does not transfer ammunition. Reloading can be interrupted by
switching to another gun; ammunition is only moved into the magazine on
successful completion of that gun's reload.

| Key | Weapon | Ammo type | Magazine | Reload | Gameplay |
|---|---|---|---:|---:|---|
| 1 | Pulse Carbine | Pulse Cells | 32 | 1.05s | Reliable fast shots |
| 2 | Arc Cannon | Arc Charges | 10 | 1.65s | Heavy slow blast |
| 3 | Sniper | Rail Slugs | 5 | 2.10s | High damage; **hold right mouse** to aim through a 3.5× scope |
| 4 | Shotgun | Shells | 8 | 1.80s | Nine independent pellet rays with wide spread |
| 5 | Minigun | Belt Rounds | 100 | 3.00s | Sustained rapid fire and rotating muzzle indicators |

Press **R** to reload, **1–5** or **mouse wheel** to switch guns, **left mouse/J** to fire,
and **right mouse** to aim down sights with the Sniper. The scope uses an
increased projection focal length plus a circular optic HUD with a reticle.
The gun model is hidden while looking through the scope. Movement sensitivity
is reduced while scoped. All other controls and quality settings are retained.

Five enemy classes correspond to the five weapons; **every defeated bot drops
ammo for its matched weapon**, with a randomized but bounded amount:

| Enemy | Matching ammo | Amount per defeat |
|---|---|---|
| Runner | Pulse Cells | 13–27 |
| Tank | Arc Charges | 3–7 |
| Marksman | Rail Slugs | 2–5 |
| Striker | Shells | 4–10 |
| Gunner | Belt Rounds | 25–55 |

Ammo pickups show the associated weapon's signature colour. There is also a
small independent chance of additional health/shield supplies; these never
replace the guaranteed ammo drops. Each of the five campaign missions contains all
five enemy types. Player damage and hit detection are calculated from separate
pellet rays, each independently blocked by arena geometry. There is no gore.

Editable new OBJ/MTL assets: `gun_sniper`, `gun_shotgun`, `gun_minigun`,
`enemy_marksman`, `enemy_gunner`, plus `enemy_marksman_lod` and
`enemy_gunner_lod`. Their builders are in `tools/build_models.py`.

Test the new mechanics with:

```powershell
cargo test -p mata-neon-run --no-default-features
cargo check -p mata-neon-run
cargo run -p mata-neon-run --release
```

This source package has not been compiled or played in the authoring
environment because Cargo is not available there.

## Idle rendering, adaptive threading, and MataWin hardware presentation

The 3D rasterizer is still the independent `mata-3d` crate. Worker lanes
expand and shrink based on the target FPS while respecting the cap of logical
CPU count minus one. Static/paused/unfocused screens skip unnecessary 3D
redraws. Press **F3** for performance statistics and **F4** to cycle target
FPS (30 / 60 / 90 / 120).

**No `wgpu`, CPAL, `windows`, `x11rb`, `pollster`, or `raw-window-handle`
dependencies appear in this game's Cargo.toml.** The game uses only first-party
`mata-win`, `mata-3d`, and `mata-audio`, plus Rust's standard library.

On Windows, this release includes a **native Intel Direct3D 11 presentation backend**
implemented directly against the Windows SDK system libraries (`dxgi`, `d3d11`,
`user32`, and an optional dynamically loaded `d3dcompiler_47.dll`). It enumerates DXGI adapters, chooses Intel vendor ID
`0x8086`, and creates a Direct3D device and swapchain. Each frame it uploads
the software-rendered scene and an independently rendered, transparent HUD,
then uses a GPU shader for scene scaling, HUD alpha composition, and display.
This eliminates the game's CPU scene upscale and composite/present copy on
supported Intel hardware. GPU resources are cached and reused, including
across ordinary frames. Resize and device errors trigger fallback to MataWin.

**Scope:** `mata-3d` still performs **all 3D vertex transformations, clipping,
shading, and triangle rasterization on the CPU**. GPU geometry rendering is a
separate project requiring changes in that sibling crate; this is not a
Direct3D geometry/compute renderer. Performance improvement is not guaranteed
without benchmarking on your machine.

Press **F5** to switch the Intel presenter on/off at runtime (Windows). Set
`MATA_INTEL_GPU=off` to force the existing MataWin presenter at startup. If no
compatible Intel adapter or D3D11 shader runtime is available, the game logs
why and runs with the existing presenter. On Linux this version retains the
MataWin software path; an Intel Vulkan/OpenGL Linux backend is future work.

This adds **no Rust crate dependencies**. Windows builds must link against the
standard Windows SDK's `dxgi.lib` and `d3d11.lib`;
`d3dcompiler_47.dll` is loaded dynamically for the shader compile step.
When absent, the game uses the MataWin fallback. No Win32/D3D bindings are added
in `Cargo.toml`.

Background detection uses the only cross-platform state currently exposed by
MataWin (`cursor_focused`), which is not the same as true OS keyboard focus.
Reliable Alt+Tab detection with the cursor remaining inside the window would
need a future `mata_win::Window::is_active()` method. The game does not pretend
that this limitation has been solved.

Run from the Mata workspace root:

```powershell
cargo check -p mata-3d
cargo check -p mata-audio
cargo check -p mata-neon-run
cargo run --release -p mata-neon-run
```

The authoring environment does not include Cargo/rustc or an Intel GPU; these
changes are not claimed to have passed native compilation or benchmarking.

## Detailed enemy models and 2026-10-16 fixes

All five robot classes now have higher-detail, hard-surface OBJ/MTL assets,
including armoured panels, exposed servo joints, layered optics, lit emitter
clusters, and distinct fighting-role silhouettes. The game uses three mesh
levels per enemy with material-grouped faces to reduce software rasterizer
cost. The asset counts and regeneration instructions are documented in
[`assets/models/DETAILED_ENEMIES.md`](assets/models/DETAILED_ENEMIES.md).

Also fixed the reserved `crate` field name and invalid shorthand floating-point
literals in the environment and boss rendering source. No Cargo dependencies
were changed. The source and model assets were checked statically, but full
compilation is still needed in the parent Mata workspace.

## Articulated enemy locomotion (all five robots)

Each robot now uses a **12-piece high-detail rigid rig** rather than a single
bobbing OBJ. The pieces are: body, head, independently hinged left/right
thighs, left/right shins, left/right feet, left/right shoulders/upper arms,
and left/right forearms. The source exporter (`tools/make_detailed_enemies.py`)
partitions every original polygon into exactly one joint-local OBJ. It does
**not** voxelize, decimate, retexture or discard geometry or materials.

- Joint angles come from a two-bone inverse-kinematics calculation; knee and
  ankle positions are propagated through their parent hinges.
- Walking frequency follows the enemy's **actual distance travelled after
  collision**, rather than global time. Blocked bots stop stepping.
- Stance feet keep world-space anchors, while swing feet arc smoothly ahead
  to the next step. Each robot has its own stride length, shoulder counter-
  swing, body-weight transfer, and turning responsiveness.
- Resting robots don't walk in place. Subtle idle motion is separate from
  locomotion. The full-detail and medium-detail models use the same rig.
- Full-resolution models near the player are **not simplified**. For far
  enemies the animated medium model replaces the old static, lower-quality
  silhouette. The old distant OBJ files are kept as legacy source assets.
- Parsed mesh data is cached by each renderer instance. No per-frame mesh
  loading, new third-party crates, or modifications to the Mata dependencies.

In a full Mata workspace, run `cargo check -p mata-neon-run`, then
`cargo test -p mata-neon-run`. The purely static validator is available at
`python mata-neon-run/tools/validate_models.py`.

## SIMD acceleration

The recovered Molecule Studio `mata-3d` now supports SIMD indexed vertex transforms and SIMD software depth/coverage tests. The game uses AVX2 gathering when software-upscaling a framebuffer. These optimizations retain all 4K enemy polygons. See `../SIMD_INTEGRATION.md` for CPU feature detection, the `MATA_SIMD=off` scalar comparison mode, commands, and limitations.

## Intrinsic function-table integration

The D3D11 shader code is generated with the first-party `mata-gpu-intrinsics` HLSL dispatch table. This validates `mix`/`saturate` operations before GPU startup, and the generated HLSL is compiled on Windows by the existing Direct3D 11 renderer. See `../GPU_DISPATCH_INTEGRATION.md` for details, how to test, and the distinction between shader emitters and native CUDA/OpenCL/HIP execution.

## Automatic graphics backend detection

Windows: the game enumerates all non-software DXGI adapters and selects the best-ranked D3D11 adapter that actually initializes. All supported GPU vendors use **HLSL** intrinsics for the existing graphics pipeline; CUDA, HIP and OpenCL remain separate compute-code generation backends, not executable graphics drivers. On other OSes the game retains CPU rendering. Set `MATA_GPU_BACKEND=auto|cpu|d3d11|intel|nvidia|amd`; F3 shows the renderer; F5 toggles GPU vs CPU. See the workspace's `GPU_AUTO_SELECTION.md`.

## Intel integrated GPU optimizations

This version preserves all full-resolution models while reducing geometry upload bandwidth and avoiding the CPU sky upload. Windows Direct3D 11 now uses compact 16-byte vertices, buffer recycling and a GPU-generated sky; the CPU worker pool stays at one encoder thread while GPU rendering is active. Optional `MATA_GPU_PROFILE=1` logs CPU capture and present timings; `MATA_GPU_VSYNC=off` allows uncapped Present for benchmarks. Both variables are optional. See `../INTEL_IGPU_PERFORMANCE.md`.

## Optimized software rendering

See [`../SOFTWARE_RENDERING_PERFORMANCE.md`](../SOFTWARE_RENDERING_PERFORMANCE.md) for the
shared software-command pipeline, band binning, SIMD raster improvements,
CPU profiling instructions, and verification limits.
