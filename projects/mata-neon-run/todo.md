# Mata Neon Run — Comprehensive graphics and rendering improvement roadmap

> **Scope:** the current `mata-win` + `mata-3d` software-rendered Rust game, including its 5 story missions, 4K enemy meshes, articulated rigs, environment OBJ assets, and sniper scope. This is a list of possible enhancements, **not** a claim that they are already implemented. No new Cargo dependencies are required for the near-term tasks. More ambitious capabilities (e.g., full GPU rendering) would require work within the existing Mata crates or separate approval before introducing a new dependency.
>
> **Non-negotiable quality policy:** **All enemies visible to the camera use the highest-quality 4K geometry, at any distance, with no LOD reductions, no two-enemy budget, and no screen-size cutoff.** Reject an enemy's whole rig *before skinning/joint transforms and rasterization* when completely outside the view frustum. Retain simulation and AI for unseen enemies. If performance is poor, optimize shared work, materials, triangle throughput, and visibility—**not character polygon count**.

## Completed in this revision

- [x] Remove range-based enemy detail switching and the two-hero character cap.
- [x] Render the existing 4K articulated OBJ rig for **every camera-visible enemy**, even distant actors and crowded scenes.
- [x] Add conservative 3D whole-rig view-frustum rejection *before* rendering and joint animation.
- [x] Avoid loading unused lower-detail rigs into each renderer/worker; keep old assets in the project without deploying them.
- [x] Keep combat/AI running offscreen independently from graphics visibility.
- [x] Add obstacle, range, and field-of-view checks to enemy detection; no wall vision.
- [x] Alert only the actual enemy hit by player fire, with a last-known-position search and grid navigation.
- [x] Add static tests for enemy awareness, navigation, and visibility edge cases.
- [ ] Build and FPS-test on Windows and Linux with the user's sibling `mata-3d`/`mata-win` crates (unavailable in this environment).

## Automatic graphics backend selection — current implementation

- [x] Enumerate Intel, NVIDIA, AMD, and other hardware DXGI adapters at runtime on Windows.
- [x] Prefer dedicated GPUs with more local video memory without reducing enemy quality.
- [x] Retry remaining GPU adapters when an earlier device or shader initialization fails.
- [x] Support `MATA_GPU_BACKEND=auto|d3d11|intel|nvidia|amd|cpu` and F5 GPU/CPU toggle.
- [x] Display selected rendering backend in the F3 statistics overlay; report vendor/name in startup logs.
- [x] Keep platform-independent ranking tests and scalar renderer fallback.
- [ ] Validate on multi-adapter Windows PCs and hybrid iGPU/dGPU laptops; a DXGI ranking heuristic is not a measured FPS comparison.
- [ ] Implement and benchmark native Linux Vulkan/OpenGL GPU rendering for Wayland/X11; current Linux GPU execution is **not** implemented.
- [ ] Implement actual CUDA/HIP/OpenCL device dispatch for compute intrinsics before claiming native vendor compute backends are selected.

## Intel iGPU backend — current implementation and follow-ups

- [x] Add Windows Intel-adapter discovery via DXGI (PCI vendor ID `0x8086`).
- [x] Add a Direct3D 11 hardware swapchain, fullscreen triangle and HLSL compositor using native Windows system APIs without adding Rust crates.
- [x] GPU scale the CPU-rendered scene and composite native-resolution transparent HUD; avoid CPU nearest-neighbour scaling when Intel backend is active.
- [x] Cache dynamic GPU upload textures and update only when resolution changes; copy respecting mapped row pitch.
- [x] Preserve 4K enemy meshes at every distance, off-screen culling, scope, story overlays and adaptive CPU render pool.
- [x] Fall back to MataWin on unsupported hardware, adapter creation errors, resize errors or GPU present failures; F5 toggles on Windows.
- [ ] Compile on Windows with the actual `mata-win` and `mata-3d` workspace crates; this package has not been compiled here.
- [ ] Run on an Intel UHD / Iris Xe / Arc integrated device and verify swapchain ownership does not conflict with the specific MataWin Windows presenter.
- [ ] Benchmark GPU copies, driver scheduling, frame latency, vsync, GPU use and frame pacing against the old MataWin path. Improvements are not established yet.
- [ ] Add a genuine GPU 3D geometry backend inside `mata-3d`: index buffers, vertex buffers, camera matrices, depth testing, native transforms for all 12 rig pieces and GPU shader materials. No such path is claimed in this revision.
- [ ] Port Intel hardware presentation to Linux using platform-native Vulkan or OpenGL where suitable, preserving Wayland/X11 integration.
- [ ] Add GPU timestamp queries, partial HUD updates/dirty rectangles, adaptive filtering, and HDR/sRGB correctness after performance baselines.

## SIMD computation pass — status (2026-10-16)

- [x] SIMD affine vertex transforms in `mata-3d` using SSE2 (x86/x86-64) and NEON (ARM64), with a scalar fallback.
- [x] Reuse an indexed mesh transform scratch buffer: one transform per unique vertex rather than three per face.
- [x] SIMD 4-wide software triangle coverage and depth tests with masked pixel/depth writes, scalar remainder pixels.
- [x] AVX2 nearest-neighbour framebuffer upscale (eight 32-bit pixels per gather) with cached X maps and a scalar fallback.
- [x] `MATA_SIMD=off` opt-out for paired measurements and debugging.
- [x] Add numeric/coverage/upscale tests, including non-SIMD-width tails.
- [ ] Compile and run tests on Windows Intel hardware, Linux x86-64, a 32-bit x86 SSE2 target, and AArch64 NEON hardware. **Not available in this environment.**
- [ ] Measure A/B frame timings with SIMD enabled/disabled. SSE2 gathers from AoS vertices and SIMD edges may not always win for tiny meshes/triangles.
- [ ] Add AVX2-specific 8-wide triangle raster tests after measuring whether they improve throughput over SSE2.
- [ ] Share prepared/animated transformed mesh buffers between band workers; current reuse happens *within each worker*, not across workers.
- [ ] Profile SoA or AoSoA interleaved vertex storage to reduce four-wide gather overhead.
- [ ] Investigate tile-based raster workloads to increase SIMD occupancy and write locality without reducing polygon detail.

## P0 — Reproducible baselines and correctness (do this first)

- [ ] **P0.01 Measure CPU and presentation times** at 720p, 1080p, 1440p, and 4K. Record median, p95/p99, 1% lows, actual CPU/GPU usage, thread count, and memory on target Intel iGPU hardware.
- [ ] **P0.02 Add a reproducible camera path** across all five campaign missions (including corridors, distant enemies, 20+ characters, boss, and scope), and record frame timing for each path.
- [ ] **P0.03 Add opt-in `std::time::Instant` timings** for simulation, AI, snapshot cloning, model transforms, clipping, rasterization, compositing, HUD, scaling, and OS framebuffer presentation. Keep instrumentation disabled in normal releases.
- [ ] **P0.04 Collect rendering counters:** submitted 4K enemy triangles, actually clipped/projected triangles, fragments tested, fragments rejected by depth, screen tiles visited, and objects culled.
- [ ] **P0.05 Check pixel correctness** at every resolution and aspect ratio: 4:3, 16:9, 21:9, portrait, window resize, and 3.4x scope view.
- [ ] **P0.06 Add before/after screenshot diff tests**, allowing tiny numerical tolerances but not disappearing geometry, broken materials, seams across worker bands, or incorrect depth.
- [ ] **P0.07 Validate bounding volumes** for each robot's animated extremities; don't wrongly cull swinging limbs at frustum boundaries or with scope zoom.
- [ ] **P0.08 Profile construction/loading time** (21 environment OBJs, five 4K rigs × 12 meshes, weapons), especially one renderer *per* worker.
- [ ] **P0.09 Create automated model validation** for index bounds, material names, duplicate faces, manifold issues, invalid normals, degenerate triangles, and collision versus visual bounds.
- [ ] **P0.10 Test sustained load** for several minutes (CPU thermals, allocations, stutters, frame pacing, and quality at long distance).

## P1 — High-impact rendering performance without reducing visible enemy detail

- [ ] **P1.01 Share immutable loaded 4K model data among workers.** `RenderPool` constructs `Renderer::new()` in every worker. Replace duplicated model allocation and repeated OBJ parsing with shared `Arc`-backed meshes, after confirming Send/Sync in `mata-3d`. Keep each worker's framebuffer and depth buffer private.
- [ ] **P1.02 Transform/skinning only once per frame.** The 12 articulated joint transforms per visible enemy should be computed on the render coordinator and passed to every worker; avoid recomputing IK/trigonometry for each scanline band.
- [ ] **P1.03 Transform 4K vertices only once per frame, not once per worker.** Generate indexed camera-space vertex arrays once and give bands/tile workers immutable references to those results.
- [ ] **P1.04 Replace horizontal-band repeated triangle setup** with a pretransform/clip/bin stage and a non-overlapping tile rasterizer (e.g., 16x16 or 32x32). Workers process distinct bins, never racing writes to Z or colour memory.
- [ ] **P1.05 Use exact view-projection frustum planes from `mata-3d`**, instead of the current deliberately conservative hand-derived frustum, so fully offscreen actors are culled with minimal false-positive work.
- [ ] **P1.06 Add screen-band and tile bounding boxes** before processing mesh triangles. Entirely out-of-tile meshes and triangles never reach the raster loop.
- [ ] **P1.07 Cache static mission geometry:** floors, fixed walls, ceiling segments, beams, pipes, signs, static fixtures, and server racks. Build optimized command buffers/merged meshes only once when loading each mission.
- [ ] **P1.08 Use a spatial acceleration structure** (uniform grid, quadtree, or BVH), built from `std` arrays, to reject static props not intersecting the frustum before mesh traversal.
- [ ] **P1.09 Add portal/sector visibility for indoor rooms.** Traverse visible rooms through door openings rather than traversing all mission props. Preserve enemies visible through open doorways.
- [ ] **P1.10 Add conservative occlusion culling** using a coarse depth pyramid or room/door visibility. Skip *entire* fully occluded actors and props, never partially visible 4K geometry. Debug mode should reveal false culls.
- [ ] **P1.11 Stop cloning the entire `World` for every worker.** Instead create a compact immutable per-frame render snapshot with camera, animated joint matrices, visible objects, and reference-counted asset pointers.
- [ ] **P1.12 Add material batching by shader/material class**, keeping the full polygon count and sorting stable opaque geometry front-to-back to improve early-Z efficiency.
- [ ] **P1.13 Precompute `MTL` material values** and convert to packed RGB/A render parameters at asset load; never parse materials during a frame.
- [ ] **P1.14 Reuse allocations** for transformed vertices, clip lists, triangle bins, tile queues, Z buffers, and command vectors; track alloc count after warm-up.
- [ ] **P1.15 Improve idle/repaint policies** for unfocused windows, menus, and minimized frames while keeping active gameplay input responsive.
- [ ] **P1.16 Instrument quality-preserving worker scaling** to avoid more threads than the actual number of useful visible tiles. Benchmark fixed thread counts against adaptive threading.
- [ ] **P1.17 Exclude fully offscreen animation draw preparation**, but continue enemy AI, collision, and perception on the simulation thread.
- [ ] **P1.18 Maintain a persistent cached per-actor bounding sphere** updated from max animation extent, without traversing mesh vertices just to cull the actor.

## P2 — Software rasterizer throughput (`mata-3d`)

- [x] **P2.01 SIMD baseline** for vertex transform and raster inner loops via SSE2/NEON and scalar fallback. **Still pending:** benchmark, tune kernels, add AVX2 raster variant if faster.
- [x] **P2.02 Reuse transformed indexed vertices** within each mesh call rather than repeating transformation for every triangle. **Still pending:** reuse across workers and materials.
- [ ] **P2.03 Perspective-correct interpolation** for depth, UV, and normals; ensure surfaces don't warp or swim during movement.
- [ ] **P2.04 Stable near-plane and frustum clipping**, including adjacent triangles sharing clipped edges to avoid visible cracks.
- [ ] **P2.05 Consistent triangle winding and backface culling**, with two-sided materials handled explicitly rather than disabling culling globally.
- [ ] **P2.06 Coarse hierarchical-Z** to reject entire occluded triangles/tiles before pixel setup.
- [ ] **P2.07 Optimize edge equations** (fixed-point incremental barycentric stepping), compare against float edge tests, and use a consistent top-left fill rule.
- [ ] **P2.08 Fast clear techniques** for colour and depth, including dirty-tile tracking or generation counters when beneficial.
- [ ] **P2.09 Depth format comparison:** `f32`, reverse-Z float (if supported by projection), or packed fixed depth; benchmark precision and bandwidth.
- [ ] **P2.10 Depth-first opaque draw** for large overlapping indoor geometry. Avoid separate depth prepass unless measurement demonstrates a win.
- [ ] **P2.11 Material grouping and cache locality** to minimize random writes and branch divergence.
- [ ] **P2.12 Profile fine vs. huge triangles**; optimize short horizontal spans, line segments, and near-camera fullscreen polygons separately.
- [ ] **P2.13 Implement a safe tile work queue** with `std::thread` synchronization and workload balancing; benchmark synchronization overhead.
- [ ] **P2.14 Spatially bin point lights and effects** to tiles (forward-plus style) so material shading samples only relevant lights.
- [ ] **P2.15 Conservative surface-area clipping** to trim large props exactly to the screen and avoid per-pixel work outside viewport.
- [ ] **P2.16 Apply deterministic rendering options** for tests and benchmarking; disable random visual noise during screenshot comparisons.

## P3 — Display quality, resolution and antialiasing

- [ ] **P3.01 Native 4K rendering mode** with transparent reporting of true internal render scale and actual window output size.
- [ ] **P3.02 TAA or temporal supersampling** with motion vectors and jittered projection; provide ghosting diagnostics for moving 12-joint robots.
- [ ] **P3.03 FXAA / SMAA-like edge smoothing** as a lighter CPU option, controllable independently from geometry quality.
- [ ] **P3.04 Selectable MSAA** only if supported efficiently by the renderer; quantify depth/sample bandwidth cost.
- [ ] **P3.05 Better spatial upscale filters** (bicubic, edge-adaptive, Lanczos variants), with comparisons on thin wires and scope reticles.
- [ ] **P3.06 Sharpening with overshoot controls**, prevent halos on illuminated objects and fine 4K enemy features.
- [ ] **P3.07 Correct viewport and aspect handling** for ultrawide, high-DPI, letterbox/pillarbox, and fullscreen transitions.
- [ ] **P3.08 Improve texture sampling** (nearest, bilinear, trilinear and anisotropic) after implementing textured surfaces.
- [ ] **P3.09 Reduce colour banding** with linear-light processing and optional dithering at final present.
- [ ] **P3.10 HDR output and tonemapping**, only when the platform presentation backend and monitor support it.
- [ ] **P3.11 Accurate sRGB/linear conversions** in lighting, blending, brightness controls, and bloom passes.
- [ ] **P3.12 Screen-space motion blur** for dynamic camera movement; optional to avoid blurry 4K models.

## P4 — Materials, surface detail and lighting

- [ ] **P4.01 UV coordinates and true material textures** for all five enemies and environmental fixtures. Keep the full geometry; don't replace parts with fake billboard detail.
- [ ] **P4.02 High-resolution texture authoring** (2K/4K albedo and masks, shared intelligently), with offline atlas generation and compression inside existing asset tools.
- [ ] **P4.03 Normal mapping / tangent frames** for panel grooves, stamped metal, cloth, hoses, carbon fibre and etched lettering.
- [ ] **P4.04 Roughness/metalness shading (PBR)** with controllable physically plausible diffuse/specular response.
- [ ] **P4.05 Light-emissive material channels** separated from base colour, for sensors, screens, conduits and hazard strips.
- [ ] **P4.06 Dynamic point and spot lighting** from ceiling fixtures, muzzle flashes, energy projectiles and alarms.
- [ ] **P4.07 Accurate light attenuation and falloff**, exposure limits for cramped interiors, and per-room colour grading.
- [ ] **P4.08 Static lighting bake/lightmaps** for room interiors where real-time lights are too expensive; keep switches/alarms dynamic.
- [ ] **P4.09 Shadow maps or CPU shadow volumes** for key lights, with temporal filtering and stable shadow edges.
- [ ] **P4.10 Contact shadows and screen-space ambient occlusion**, protecting depth edges and excluding UI.
- [ ] **P4.11 Ambient probes / irradiance grids** for smooth shading in rooms and door thresholds.
- [ ] **P4.12 Specular reflections and reflection probes** for metal surfaces, visors, polished floors and glass.
- [ ] **P4.13 Screen-space reflections** as an optional expensive pass; fallback to probes when ray information is missing.
- [ ] **P4.14 Transparent and refractive materials** for glass, helmet optics, security windows and coolant tanks; correct depth sorting.
- [ ] **P4.15 Decals** for serial numbers, warnings, wear, grime, dents, key markings and player impact marks.
- [ ] **P4.16 Material wear variation** by mission (clean outpost, oxidized refinery, cold containment, control-room polish, archive heart).
- [ ] **P4.17 Animated UI screens on real terminals**, using render-to-texture surfaces, procedural scanlines, graphs and meaningful status.
- [ ] **P4.18 Room-specific light failure and flicker** linked to relay state; avoid overly fast flashing and allow accessibility adjustment.
- [ ] **P4.19 Character sensors and optic lens illumination** should respond to environment lighting and alert state, independent of geometry quality.
- [ ] **P4.20 Enable independent quality knobs** for shadows, volumetric light, reflections and effects, **never for visible enemy mesh detail**.

## P5 — Scene geometry and visual storytelling

- [ ] **P5.01 Enrich the station architecture**: doors that actually slide, seals, thick bulkheads, pipes, floor hatches, observation windows, working vents and maintenance shafts.
- [ ] **P5.02 Use physically placed area lights** and utility panels; ensure lights have supports, wiring and believable physical sources.
- [ ] **P5.03 Increase environment asset uniqueness** using procedural variations of metal panels, screws, labels and station insignias.
- [ ] **P5.04 Detailed first-person gun models for all weapon slots**, coherent style with LANCER-7 and consistent scope interaction.
- [ ] **P5.05 Mechanical animations for gun bolts, magazines, vents, rotors and cooling systems**, using independent bones/pivots rather than whole-model shaking.
- [ ] **P5.06 Articulated enemy hands, toes, pistons and neck joints**; ensure visual rig interpolation matches the fixed physics tick.
- [ ] **P5.07 Procedural reactions to terrain and obstacles** (foot tilt, crouch, collision response), preserving the existing 4K mesh vertices.
- [ ] **P5.08 Enemy alert-state visual feedback:** scanning eyes while idle, attention/aim stance on sight, search-light sweep when investigating, and hit reactions.
- [ ] **P5.09 Weapon projectiles with trailing emissions** and clear silhouettes; distinguish dangerous shots from ambient lights.
- [ ] **P5.10 Boss-specific animated mechanical shutters, targeting array, shield resonance and independent witness storage lights.**
- [ ] **P5.11 Additional fixed story props** tied to each archived record: ledger screens, chair arrays, sealed equipment boxes and maintenance marks.
- [ ] **P5.12 More realistic terrain collision and foot contacts**, verified separately from rendering so rich geometry doesn't trap characters.

## P6 — Effects and postprocessing

- [ ] **P6.01 Selective bloom** for emissive lights, energy impacts, consoles and visor displays; avoid washing out text.
- [ ] **P6.02 Volumetric fog / light shafts** in limited zones; animate conservatively and offer an off switch.
- [ ] **P6.03 Particle emitters** with distance-independent silhouette clarity for sparks, dust, coolant, debris and smoke.
- [ ] **P6.04 GPU-like particle batching on CPU**, with pooled allocations and a single tile-aware path.
- [ ] **P6.05 Better explosion and hit feedback** with physically located glow, debris, surface scorch and short-lived light.
- [ ] **P6.06 Heat distortion and refraction** localized to hot refinery piping, engine cells and shield emitters.
- [ ] **P6.07 Optional film grain, chromatic aberration, vignette and lens dirt**, with per-effect toggles and minimal default use.
- [ ] **P6.08 Temporal stability** for thin emissive wires and high-frequency surface decorations; avoid flicker and moiré at 4K.
- [ ] **P6.09 Fullscreen material preview mode** for comparing shading before and after new postprocessing stages.
- [ ] **P6.10 Limit particles and postprocessing through explicit independent budgets** instead of reducing enemy mesh quality.

## P7 — Animation and motion presentation

- [ ] **P7.01 Interpolate enemy joint transforms between fixed simulation ticks**, avoiding 120 Hz movement appearing jittery at mismatched render FPS.
- [ ] **P7.02 Add turn-in-place, deceleration and stop transitions** independent of walk; keep foot plants stable on abrupt stops.
- [ ] **P7.03 IK improvements** for sloped ground, stair edges, doors and collision boundaries.
- [ ] **P7.04 Upper/lower body layers** for accurate aiming while walking, search poses, and reload cycles.
- [ ] **P7.05 Hit, stagger, recovery and shutdown animations** tied to bot awareness/state, with no graphic violence.
- [ ] **P7.06 Reactive animation detail** such as damping springs, cables, pistons, head tracking and rotating sensor drums.
- [ ] **P7.07 Independent LANCER-7 ADS camera sway/reticle stability**, seamless scope transition and appropriate reticle visibility at different resolution scales.
- [ ] **P7.08 Deterministic pose snapshots** so all raster worker tiles use precisely the same joint matrices.

## P8 — UI, legibility and accessibility

- [ ] **P8.01 Resolution-independent HUD scaling**, readable without pixel bleed at 4K and ultrawide.
- [ ] **P8.02 High-DPI fonts** with sharp vector/outline rasterization and proper subpixel positioning, optionally via existing `mata-fonts` if permitted.
- [ ] **P8.03 Clear mission/objective signage**, distinct archive terminal glyphs and in-world navigation without requiring minimap.
- [ ] **P8.04 Better minimap occlusion/scale**, accurate doors and line-of-sight cues (avoid showing enemy detection in a misleading way).
- [ ] **P8.05 Scope reticle sharpness at native display resolution**, even when 3D world is internally upscaled.
- [ ] **P8.06 Adjustable colour-vision palette, gamma, text size, and screen flash reduction.**
- [ ] **P8.07 Visor effects that reinforce shield/hurt state** without obscuring enemies or important objectives.
- [ ] **P8.08 Native-resolution story records and usable scrolling**, high contrast against noisy scenes.
- [ ] **P8.09 Video settings menu with separate resolution, antialiasing, light/shadow and postprocessing toggles; enemy quality remains locked to full 4K.**

## P9 — Asset pipeline and maintainability

- [ ] **P9.01 Parse OBJ/MTL offline to a compact internal binary mesh format** with stable version and CRC, loaded through the same existing Rust crates. Keep editable OBJs in source.
- [ ] **P9.02 Deduplicate shared vertex streams and materials** across 12 articulated parts; ensure pivot transforms and correct normal seams.
- [ ] **P9.03 Fix tangents/normals at part boundaries** to avoid visible gaps or lighting seams in articulated limbs.
- [ ] **P9.04 Generate texture atlases and mip chains offline** without runtime conversion or new packages.
- [ ] **P9.05 Validate asset build reproducibility** on Windows and Linux, including deterministic material ordering and line endings.
- [ ] **P9.06 Add model preview tooling** for multiple camera angles, each animation state, and material/depth debug modes.
- [ ] **P9.07 Retain original 4K hero and lower-detail source OBJs as editable originals** but do not activate visual-quality fallback for visible enemies.
- [ ] **P9.08 Add debug wireframe, bounding spheres, frustum planes, occlusion/tile overlays and overdraw heatmap.**
- [ ] **P9.09 Add asset fingerprint/version metadata** to diagnose builds where assets differ from source or another worker.
- [ ] **P9.10 Integrate static geometry and rig validation into CI**, followed by real Rust compilation/tests against available Mata crate versions.

## P10 — Hardware backend and presentation (requires Mata crate changes)

- [ ] **P10.01 Benchmark current software framebuffer upload**; eliminate redundant full-buffer copies at presentation if `mata-win` supports persistent mapped buffers.
- [ ] **P10.02 Explore hardware rasterization via first-party Mata APIs**, keeping the exact 4K OBJ vertex data and shader-equivalent materials.
- [ ] **P10.03 Add an Intel iGPU-specific backend** only in the owning `mata-3d` or `mata-win` crates; no unapproved third-party library.
- [ ] **P10.04 Keep software fallback and pixel-accurate regression tests** across hardware vs. CPU paths.
- [ ] **P10.05 Double/triple buffering and vsync controls**, with real measured presentation latency and reduced input-to-photon delay.
- [ ] **P10.06 Separate render and present pacing** so slow monitors don't stall game simulation or mouse movement.
- [ ] **P10.07 Explore compute/raster hybrid** for character geometry, while maintaining same vertex/rig quality and accurate depth interaction with station props.
- [ ] **P10.08 HDR / wide-colour platform negotiation** with robust SDR fallback.
- [ ] **P10.09 Proper dynamic resolution only for *world pixel shading*** when requested by the user; never switch enemy geometry tiers or skip detail on visible characters.

## Suggested implementation sequence

1. **Correctness + evidence:** P0.01–P0.10. A frame counter alone cannot establish a real speedup.
2. **Easy architectural wins:** P1.01–P1.08 (share meshes, transform once, visibility, static scene command buffers).
3. **Foundational geometry throughput:** P2.01–P2.08, especially tile bins, clipping, indexed transforms, and early depth.
4. **Quality upgrades:** P4.01–P4.10 (textures, material lighting, shadows), followed by P3.02–P3.05 (antialiasing/upscaling).
5. **World fidelity and animation:** P5, P6, P7, then P8 readability and accessibility.
6. **Hardware acceleration:** P10 only after measured CPU baselines and a stable software reference.

### Hard acceptance criteria

- [ ] A fully offscreen 4K enemy performs **zero mesh/joint transform, triangle assembly, clipping and raster work** in the rendering pipeline (simulation still runs).
- [ ] Every visible enemy uses precisely the 4K authored meshes, including the farthest visible character and crowded scenes; no runtime LOD swaps.
- [ ] No lost triangles/limbs when crossing a screen edge, scoping, resizing or using multiple renderer workers.
- [ ] AI does not detect through solid obstacles; active search uses last known position rather than the player's hidden position.
- [ ] FPS improvement measurements include full-scene pixel output and are reported with unchanged visual-quality conditions.
- [ ] No Cargo dependencies added without permission.
