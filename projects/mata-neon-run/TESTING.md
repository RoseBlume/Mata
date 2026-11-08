# Build and gameplay checks

Build inside your existing Mata workspace so `../mata-win`, `../mata-3d`, and (if audio enabled) `../mata-audio` exist:

```bash
cargo test -p mata-neon-run --no-default-features
cargo check -p mata-neon-run
cargo run -p mata-neon-run --release
```

- Look around continuously while holding the mouse far in either direction. On Windows/X11 the pointer should recenter, not stop at the edge. Release focus and confirm visible cursor returns.
- Start mission 1 with Enter twice. Find the blue key, then the powered exit hatch. Re-approach and press E.
- Mission 2: use both relays. Mission 3: defeat all wardens and retrieve a key. Mission 4: operate three relays and retrieve the key.
- Mission 5: use all three shield switches, fire on the Archivist after the shield goes down, dodge telegraphed beam, and send evidence through uplink.
- Evidence screens should preserve each recovered record across level transitions.
- Check overhead ceilings, visible lamps, status-lit doors, computer terminals, server racks, pipe bundles and crates in 3D at different internal quality settings.

**Caveat:** the game is packaged here without its sibling Mata libraries or a Rust compiler, so compilation and real gameplay testing were not possible in this environment. Static map reachability, OBJ geometry and unchanged Cargo manifest were validated separately.

## Futuristic sniper and optic transition

1. Select weapon **3** (LANCER-7): the detailed pale-and-black mesh should be visible when unscoped, with optic above and bipod below.
2. While standing still, hold right mouse: weapon physically slides up and to the screen centre over ~0.26 seconds, world field of view narrows gradually, and a scope ring/reticle fades in.
3. Release right mouse: reticle fades, FOV widens, sniper lowers smoothly over ~0.22 seconds.
4. Confirm left-click while aiming still fires the original sniper ammo and R still animates a reload.
5. While zooming, switch to weapon 1: scope instantly clears; switching back starts unzoomed.
6. Check all screen resolutions; ensure scope UI preserves story/HUD overlays and framerate is reasonable on the target system.
7. Run unit tests (`cargo test -p mata-neon-run --no-default-features`) to check aim interpolation and weapon-switch reset.

The provided assets were validated as OBJ triangles with existing MTL material names and finite geometry; the Rust build must be verified in a full Mata workspace containing `mata-3d` and `mata-win`.

## 2026-10-16 source fix and enemy art update

`environment.rs` used `crate` as an identifier and both `environment.rs` and
`render.rs` contained invalid shorthand Rust float literals. These have been
changed to legal identifier `storage_crate` and explicit forms such as `0.055`.

Five new close/medium-distance enemy OBJ + MTL pairs and five tiny `_far`
variants are now embedded in `Renderer::new()`. The original LANCER-7 sniper,
optic transition, campaign missions and controls are unchanged.

Static asset validation (no Rust toolchain required):

```bash
python tools/validate_models.py
```

Full engine validation in the existing Mata workspace:

```bash
cargo check -p mata-neon-run --no-default-features
cargo test -p mata-neon-run --no-default-features
cargo run -p mata-neon-run --release
```

Check especially the Runner, Striker, Tank, Marksman and Gunner at three
ranges, including weapon hits and scope aiming. Full Rust compilation wasn't
possible here: the sibling `mata-win` and `mata-3d` source crates and Rust
compiler aren't present in this sandbox.

## High-quality articulated locomotion

The original bob-only enemy animation has been replaced by cached twelve-piece
full-material articulated robot rigs. All five bots retain the original close
and medium source polygons, grouped into the same materials; see the exact
triangle conservation checks in `python tools/validate_models.py`.

1. Watch a **stationary** enemy: feet stay still, the chassis exhibits only
   a subtle idle motion, and the legs do **not** march in place.
2. Draw a Runner into a clear corridor. Verify alternating planted and
   airborne feet, bending knees, counter-swinging arms, and torso weight-shift.
3. Draw a Tank into motion. Its heavy step and limb motion should be more
   restrained than a Runner's. Check all five enemy types independently.
4. Lead a Striker into a wall: if its collision-resolved ground position is
   unchanged, its stepping cycle should stop advancing.
5. Circle an enemy: it turns smoothly toward you while its walking/strafe
   motion follows distance actually travelled, not global elapsed time.
6. Switch quality (F2) and compare enemies at close and medium distance.
   The full-resolution OBJ still retains every source polygon, and the
   medium OBJ keeps every original medium-detail triangle.
7. Verify hit flashes and projectile origins are unchanged; existing enemy
   hitboxes still use the root collision capsule, not the swinging feet.
8. Pausing, reading evidence and losing focus must not advance movement or
   foot cycles. Multiple scanline worker threads should display identical
   body and limb poses (pose evaluated once in World::update).

Build and run in the original workspace:

```bash
cargo test -p mata-neon-run --no-default-features
cargo check -p mata-neon-run
cargo run -p mata-neon-run --release
python mata-neon-run/tools/validate_models.py
```

**Limitations:** Full Rust compilation and real gameplay testing are still
required on a machine with the sibling Mata crates and Rust installed.
Static source, OBJ geometry, part material, and exact face-count preservation
checks are performed by the included validator.
