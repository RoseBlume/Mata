# ASTRAEUS LANCER-7 — original game-only prop

- `gun_sniper.obj`: the new editable Wavefront geometry, +Z forward, +Y up.
- `gun_sniper.mtl`: 21 complementary diffuse/emissive game materials.
- `gun_sniper_preview.png`: simple offline mesh preview (not an engine screenshot).
- `tools/make_futuristic_sniper.py`: deterministic standard-library model generator.

**Reference interpretation:** high-contrast futuristic stylization of the three provided photographs (angular pale chassis, raised optical tube, dark forward barrel silhouette, skeletonized rear profile and two-legged support). The resulting geometry is original, decorative and deliberately nonfunctional. No photos or physical dimensions were embedded in the game.

**Runtime path:** `src/render.rs` loads this model with `Model::from_obj_mtl` at startup in place of the previous sniper mesh. `World::aim_blend` stores a fixed-update 0..1 animation driver. The renderer applies a smoothstep transition to the weapon transform and camera FOV; the HUD cross-fades to an optical vignette and reticle near completion. Existing projectile, reload and hit logic is unchanged.

**Regenerate:** from the game root, run `python tools/make_futuristic_sniper.py`. No Python third-party packages required. The PNG preview is an optional offline illustration, not part of the runtime loading path.
