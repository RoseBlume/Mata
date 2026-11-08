# Five upgraded enemy meshes

These OBJ/MTL assets were procedurally created for this game, are embedded via
`include_str!` in `src/render.rs`, and do not require textures, downloads, or
additional Cargo dependencies. Model +Z is forward, +Y is up, feet are at Y=0.

| Enemy | Close triangles | Mid triangles | Distant triangles | Visual identity |
|---|---:|---:|---:|---|
| Runner | 3468 | 960 | 120 | Amber interceptor; agile leg units, dorsal thrusters, optical sensor |
| Striker | 3384 | 900 | 128 | Magenta pincer unit; attack arms, enlarged shoulders, paired blades |
| Tank | 3940 | 1216 | 156 | Heavy violet siege robot; armour, defensive pods, cooling banks |
| Marksman | 3944 | 1112 | 96 | Cyan long-range robot; telescopic array, tracking mast, emitter |
| Gunner | 4068 | 1208 | 120 | Amber heavy gunner; dual forward rotary pods, recoil housing |

Source geometry is regenerable with Python standard library:

```bash
python tools/make_detailed_enemies.py
```

The models each use up to 12 material definitions (several deliberately share
metallic or illuminated accents); emitted OBJ faces are grouped by material to
avoid excessive material changes. LODs are switched at 13 and 23 world units
when using the normal quality setting; the close range contracts to 7.5 at
fast/adaptively-reduced quality. Original silhouette models are retained with
`_far` suffix. Behaviour, enemy health, weapons, and ammo drops are unchanged.

**Important**: `tools/build_models.py` is the historic base-model generator. If
you regenerate it, rerun `tools/make_detailed_enemies.py`, and then
`tools/make_futuristic_sniper.py`, to restore these upgraded models.

Preview screenshots from the actual OBJ geometry are in `previews/`; they are
static offline renders (not captures from a running build).
