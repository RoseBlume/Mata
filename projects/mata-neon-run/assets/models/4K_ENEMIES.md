# 4K-view hero OBJ models

The 4K label refers to **mesh detail appropriate for close-up display on a
high-resolution monitor**, not a 4096×4096 texture. OBJ is a geometric file
format and has no fixed pixel resolution. The current Mata software renderer
uses per-face MTL colours; adding a 4096² bitmap would not improve its output
without introducing UV mapping, filtering, and an actual texture sampling path.

## Included files

Each of `runner`, `striker`, `tank`, `marksman`, `gunner` has:

- `enemy_NAME_4k.obj`, `.mtl`: assembled full-scale static, editable reference
  model with all original mesh triangles plus newly authored detailed geometry;
- `rig/enemy_NAME_4k_{body,head,thigh_l,shin_l,foot_l,thigh_r,shin_r,foot_r,upper_arm_l,forearm_l,upper_arm_r,forearm_r}.obj`: 12 individual meshes in
  **the same local joint space** as the original animated rigs;
- unchanged `enemy_NAME.obj` and `enemy_NAME_lod.obj` plus their respective
  original articulated rig files for lower on-screen pixel sizes.

The generator is `tools/make_4k_enemies.py` (Python standard library only).
Run it from the project root using `python tools/make_4k_enemies.py`.

## Material detail

All hero meshes retain the original colour/material slots and add three-
dimensional engineering details: bearings with chamfered rims, precision
fasteners, finned heat exchangers, bevelled armour trim, folded ventilation,
optic glass frames, illuminated ring elements, and mechanical joint details.
The additional structures are actual geometry rather than flat image decals.

## Gameplay integration and performance (updated)

The game **always draws the 4K hero geometry for every on-camera robot**, with all 12 individually animated mesh parts. It does not swap to any lower-poly source mesh based on distance, screen resolution, number of enemies or `high_quality`. Original full/medium/far models remain on disk for editing but **are not parsed by the runtime renderer**.

`src/visibility.rs` performs a conservative 3D bounding-sphere frustum check, including limbs, before the animator submits geometry. Whole robots outside the view have **no per-frame joint transformation or rasterization**. This is intentionally different from hiding the AI: enemy movement, damage, vision and combat remain active offscreen. Frustum precision can be improved further by exposing the exact current projection parameters from `mata-3d` (see `todo.md`).

The all-4K policy is more demanding when several enemies are on-camera. The roadmap prioritizes sharing rig/model memory between workers, computing animated transforms once, tile-based triangle processing, occlusion and depth rejection **without reducing visible character geometry**.

## Reproducibility and checks

Run:

```bash
python tools/make_4k_enemies.py
python tools/validate_models.py
cargo check -p mata-neon-run
```

The model validator verifies face indices, material references, a strict LOD
triangle hierarchy, consistency of 12-pivot rig geometries, and that the hero
files retain every triangle of the original detailed mesh. A Rust build and
native 4K visual benchmark are still required in the user's Mata workspace.
