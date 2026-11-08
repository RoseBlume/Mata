# Full-quality character frustum and AI acceptance checks

This project requires the user's existing sibling `../mata-win`, `../mata-3d`, and optionally `../mata-audio` crates. The current artifact could not be compiled in the model environment, so the following steps must be performed on the user's machine.

```bash
cargo test -p mata-neon-run --no-default-features
cargo check -p mata-neon-run
cargo run -p mata-neon-run --release
```

1. Verify every enemy visible at far, medium and short distances retains identical high-detail armor, bolts, vents and all 12 animated rig pieces. Test both high- and low-resolution windows. No visible LOD switch should occur.
2. Face away from a close robot, and then pivot back. Its 4K rig must appear intact, with no missing arms at the edge of the view. Repeat while using the sniper scope and resizing.
3. Record 4K mesh transforms and triangle submissions for all bots facing away from the camera. Offscreen rigs must have **zero per-frame rig transforms/draw submissions**, even as they continue moving in the simulation.
4. Populate a room with 15+ visible bots. **All of them** must render with full 4K detail; no two-enemy budget or camera-distance cap.
5. Verify idle robot remains in place while out of range. Put Rook behind a solid wall: even nearby bots must not acquire Rook through it. Approach from the front along an unobstructed corridor: bots notice, pursue, and shoot only when line of fire is clear.
6. Approach from behind the robot's vision cone: it should not instantly detect Rook. Let it scan and turn toward the player, then confirm it can detect normally.
7. Damage an unaware bot while it is not looking toward Rook. It should investigate the shot origin; if it then sees Rook, it can pursue.
8. Break line of sight around a corner. The enemy navigates toward the **last observed position**, searches briefly, then stops. Relocating behind a wall should not immediately redirect it.
9. Verify the five enemy kinds have distinct detection distances/cones and retain their weapons, projectile speeds, ammo drops, and articulated walking.
10. Benchmark 1080p, 1440p and 4K with 1, 5, 15 and 30 visible enemies. Record worst-case render time; full geometry is intentionally never dropped to raise FPS.

`todo.md` includes a detailed prioritized performance and visual-fidelity roadmap. No Cargo dependencies have been introduced.
