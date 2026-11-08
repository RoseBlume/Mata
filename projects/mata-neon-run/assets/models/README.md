# Neon Run — original low-poly 3D assets

All five visible model designs are **real editable Wavefront OBJ meshes**, not
2D sprites or boxes drawn in the old renderer. Each model is paired with an
MTL palette. The game embeds these files at compile time and loads them once.

| Model | Game use | Colours | Base triangles | Notes |
| --- | --- | --- | ---: | --- |
| `gun_pulse_carbine` | Weapon 1 | Graphite / electric cyan | 392 | Long single emitter, rails, sight, trigger/grip |
| `gun_arc_cannon` | Weapon 2 | Violet / magenta / cyan | 472 | Wide body, dual emitters, vented energy housing |
| `enemy_runner` | Runner | Orange / cyan | 540 | Slim legs, articulated limbs, visor, heel armor |
| `enemy_striker` | Striker | Magenta / violet | 484 | Armoured shoulders, striking pincers, angular head |
| `enemy_tank` | Tank | Purple / neon pink | 736 | Oversized chassis, shoulder armour, core and heavy feet |

Each enemy also has a lighter `enemy_*_lod.obj` / `.mtl` for draw distances beyond
17.5 world units. These preserve the same overall silhouette at a fraction of
the triangles. The model naming follows the `World::BotKind` variants.

## Edit in Blender or other 3D tools

Load an `.obj` with its matching `.mtl` in the same folder. Export with `v`,
`f`, and `usemtl` lines (triangles or quads), and preserve named `newmtl` sections
containing `Kd` RGB diffuse values and optional nonzero `Ke` emission values.
This CPU renderer supports **flat materials**, not UV/image texture sampling.
An unsupported MTL property is simply ignored; unrecognized material names use
a fallback grey colour.

Coordinates: local **+Y up**, **+Z forward**, **+X right**. Enemy feet are at
approximately Y=0; the world renderer rotates enemies to face the player. The
guns are camera-space objects with muzzle tips near positive Z, so their
barrels point outward from the player's view. Model dimensions are in game
units. No collision meshes are read from these OBJ assets; gameplay collision
remains unchanged in `src/world.rs`.

## Regenerating these assets

For optional developer use, `tools/build_models.py` creates the original OBJ and
MTL files without third-party Python packages. `tools/render_preview.py` creates
contact-sheet previews using optional Pillow. Neither Python nor Pillow is
required to build or run the Rust game.
