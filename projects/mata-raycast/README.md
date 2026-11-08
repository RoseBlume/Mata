# MATA // OUTPOST ZERO — The Echo Directive

**Five-level original 2.5D FPS campaign**, built in Rust with five direct Mata crate dependencies: `mata-win`, `mata-ui`, `mata-image`, `mata-audio`, and `mata-fonts`. `mata-win` provides native windows/input/framebuffer; `mata-ui` supplies the font renderer. The game uses a custom software raycaster, pixel textures, billboard characters, hitscan combat, original audio cues, a 60 FPS-target game loop, and native MataWin input/framebuffer output. No general-purpose game engine, SDL, wgpu, or Bevy is used.

## Campaign (minor story spoilers)

| Act | Location | Gameplay | Narrative thread |
|---|---|---|---|
| 1 — **Outpost Zero** | The Breach | Recover a key and extract | Who sent your own distress code? |
| 2 — **Blackout Refinery** | The Dark Grid | Reactivate two relays | Where is the power really going? |
| 3 — **The Iron Vault** | Containment Sector | Recover a key, disable guards | What was Project LANTERN designed to preserve? |
| 4 — **Zero Hour** | Central Command | Three relays, a key, guards | Why did the Archivist summon ROOK-7? |
| 5 — **The Last Transmission** | The Archive Heart | Disable three shield resonators and defeat the Archivist | What should happen to the voices trapped in the Array? |

The full revised, 15,000+-word novel-length campaign manuscript is now embedded in the executable and accessible through a page-by-page story archive. This update adds a complete final level, a dedicated three-stage boss fight with a shield/health bar and aimed, telegraphed beam, phase-spawned guardians, and unique core art/effects. All five acts have expanded story context, post-mission reveals, and **three interactive optional lore consoles each** (15 records total). There is a campaign prologue, an epilogue, and a persistent evidence journal. The detailed plot, character backgrounds, chronology and act-by-act treatment are in `STORY_BIBLE.md`.

## Run

With the game source next to `mata-win`, `mata-ui`, `mata-image`, `mata-audio`, and `mata-fonts`, start from a sibling directory:

```bash
cargo run --manifest-path mata-raycast/Cargo.toml --release
```

Or extract the complete ZIP bundle, then run at its root:

```bash
cargo run -p mata-raycast --release
```

If integrating into an existing Mata workspace, install the split `mata-win` and `mata-ui` crates alongside the game and the required image/audio/font crates. **The game now directly depends on all five Mata crates listed above.**

## Integrated campaign manuscript

The full edited `Story.md` now ships both as a readable file and **inside the executable** with `include_str!`. The game's own software framebuffer draws the manuscript at native resolution in a resizable, automatically wrapped reader—no HTML renderer, additional dependency, or external file path is required.

- Starting a new campaign opens the actual **Prologue**. Read with Enter/Down, or press Esc to skip directly to the Act I briefing.
- Each mission briefing displays its opening prose from the manuscript (without revealing the end-of-act plot).
- On finishing each mission, its **complete story chapter** opens. Close with Esc/B to reach the debrief, or read all pages with Enter/Down. The final act flows directly into the full Epilogue when the last story page is turned.
- Press **B** from the title menu, pause screen, journal, during gameplay, or from a briefing/debrief to browse the in-game story library. Completed acts are available to reread; future chapters remain locked.
- Five historical scenes in **Before the Silence** become available after Act III; the final evidence index opens after completing the campaign.
- Story unlocks derive from the existing `MATA_CAMPAIGN_V2` checkpoint, so old save files remain valid and no new save format is necessary. The 15 collectible recordings and their own journal remain unchanged.
- Story text is formatted in a cache per chapter and viewport width, rather than allocating and rewrapping an entire chapter every frame.

## Controls

| Input | Action |
|---|---|
| **WASD** | Move and strafe |
| **Left/Right arrows** | Turn |
| **Right mouse + move** | Aim left or right |
| **Space / Left click** | Fire |
| **Shift** | Sprint |
| **E** | Open gates, activate relays, read purple archive consoles |
| **J** | Open/close recovered evidence journal |
| **B** | Open story library (from any screen); close story reader or library |
| **Up/Down** (story library) | Select a chapter |
| **Enter** (story library) | Read an unlocked chapter |
| **Enter / Down / Space** (story reader) | Next page, or continue at final page |
| **Up** (story reader) | Previous page |
| **Left / Right** (story reader) | Previous/next unlocked chapter |
| **Esc / B** (story reader) | Return to the game or previous menu |
| **Left/Right** (journal) | Previous/next recovered record |
| **Up/Down** (journal) | Previous/next page |
| **E / Esc / Enter** (journal) | Return to play |
| **Enter** | Advance prologue/briefing/debriefing, resume, start or retry |
| **N** (title) | Start a new campaign |
| **C** (title) | Resume latest campaign checkpoint |
| **R** (play/pause/game over) | Restart from checkpoint |
| **F1 / F3** | Toggle minimap / mute sound |
| **Esc / M** | Pause, navigate title, or quit as indicated |

### Final boss tactics

The Archivist initially takes **no damage**. Activate the **three shield resonators** (`T` tiles). The boss then has **24 hit points** and moves through three attack phases as its health drops. The beam aims at your location **when the warning begins**. Sidestep or break its line of sight using cover before the pulse. New guardians arrive during the final two phases. Finish the fight, then reach the exit/uplink tile. The final story and evidence transmission plays automatically.

## Saves and migration

- On Windows, campaign saves live under `%LOCALAPPDATA%\mata-raycast\campaign.save` (with `%APPDATA%` fallback).
- On Linux, saves live under `${XDG_STATE_HOME:-~/.local/state}/mata-raycast/campaign.save`.
- Checkpoints save after mission completion; discovered evidence flags save **immediately** after opening a console. The journal persists across retries and future acts. Active enemy and player positions still restart from the beginning of the saved mission.
- The new format is `MATA_CAMPAIGN_V2` and supports **five** acts and a **15-bit recovered-evidence mask**.
- Existing `MATA_CAMPAIGN_V1` saves from the four-act edition are read. If Act 4 had been completed, the old saved game starts you at the **new Act 5**, with survival supplies. Earlier checkpoints resume their original mission.
- After completing Act 5, **C** replays the final checkpoint; **N** starts over while keeping the best score.

## Files

- `src/levels.rs` — five original 24×16 maps, actor placement, objectives, act-specific narrative.
- `src/lore.rs` — fifteen primary-source station records, short prologue and epilogue.
- `src/world.rs` — gameplay, interactions, shooting, raycaster collision rules, boss beam and summoning AI.
- `src/campaign.rs` — screens, story unlocks/navigation, journal discovery, checkpoint persistence and V1 migration.
- `src/render.rs` — walls and sprite rendering, HUD, boss bar, journal and the full-text story library/reader.
- `src/main.rs` — MataWin event loop/input and cursor visibility, Mata UI font initialization, game updates and audio cues.
- `src/sound.rs` — MataAudio WAV playback.
- `assets/textures/` and `assets/sfx/` — original generated PNG and WAV assets.
- `src/story.rs` — compile-time manuscript embedding, chapter extraction, and dependency-free line layout.
- `Story.md` — the complete rewritten original manuscript now available inside the game.
- `STORY_BIBLE.md` — worldbuilding notes, setting, characters, and timeline.

## Verification limits

A static verifier checked every map's dimensions, perimeter, reachability, exit, exactly three accessible lore terminals and expected relay count, plus the final stage's boss placements and assets. Unit tests are included for map geometry, combat, phased-boss mechanics, save decoding/migration, journal persistence and checkpoint transitions. **A Rust compiler was not available here, so `cargo check`, unit tests and real gameplay were not executed.** The project should be compiled and tested on your machine before considering it validated.

## 2026 elevation, input, and performance update

Each of the five missions contains a yellow lift on the minimap and a blue elevated walkway. Stand on the lift and press **E** to raise or lower it; its height changes continuously and carries you up and down. Raised surfaces have their own floor texture and side walls. The player's eye height changes with the platform, and gravity applies when walking back down. The lift is optional to complete the current objective; all original campaign and story content remains present.

Horizontal mouse movement now moves the on-screen crosshair directly; hitscan shots use that adjusted camera-plane angle. **Right mouse resets aim to centre.** No right-button dragging is needed. On desktop, MataWin's portable cursor API hides the pointer while actively playing and restores it on pause, focus loss, story screens, and exit. **This portable API changes cursor visibility only: it does not confine, warp, or lock the mouse.** Moving past the window edge ends tracking until the pointer returns. Windows, X11, and Wayland use the same game code.

The framebuffer still renders at window-native resolution, using quantized **precomputed 40-level texture shading** and incremental wall texture coordinates rather than floating-point pixel multiplication. Raised faces are stored on the stack. No renderer or physics crates were added.

**Integration requirement:** The game uses `Window::set_cursor_visible`, `Window::show_cursor`, and `Window::cursor_focused` from the **portable-cursor MataWin** version, not the older custom `capture_cursor`/`warp_cursor_to_center` extension. It uses `mata_ui::widgets::FontRenderer` from the split Mata UI crate.

Compile and run on your own machine to verify performance and native cursor behavior; Rust compilation and live gameplay could not be tested in the authoring environment.

## MataWin / Mata UI split migration

`mata-win` is intentionally limited to window creation, events, and framebuffers.
The game retains its own software raycaster and HUD, but TrueType drawing uses `mata-ui`'s `fonts` feature.

```toml
[dependencies]
mata-win = { path = "../mata-win", default-features = false, features = ["wayland", "x11"] }
mata-ui = { path = "../mata-ui", default-features = false, features = ["fonts"] }
```

The source-only ZIP assumes the existing workspace already has the split Mata crates.
Run `cargo check -p mata-win`, `cargo check -p mata-ui`, and `cargo check -p mata-raycast`.

## Native software 3D meshes (mata-3d)

Raycast now has a sixth direct crate dependency, **`mata-3d`**, separate from
`mata-win` and `mata-ui`. It is a reusable CPU triangle renderer—not another UI
framework or a replacement for the game's existing raycast walls.

- **Ammo:** low-poly energy cartridges parsed from `assets/models/energy_cartridge.obj`.
- **Health and security key:** spinning faceted 3D crystal meshes.
- **Moving lifts:** four small 3D lamp meshes follow the moving platform.
- Pickups use the same positions and active state as the original world simulation,
  so collecting a pickup removes its corresponding mesh instantly.
- Real 3D objects use the camera's 3D projection, per-triangle lighting, and a
  shared per-pixel depth buffer, including walls, raised floors and enemy billboards.
- **F2** toggles the 3D mesh pass. With it off, the original PNG pickup
  billboards return automatically so collectibles never become invisible.
  The HUD shows the number of rasterized 3D triangles.

### Setup

Copy `mata-3d/` alongside `mata-raycast/`, `mata-win/`, and `mata-ui/` in your
workspace. Add `"mata-3d"` to your `[workspace].members` if needed. The
updated `mata-raycast/Cargo.toml` contains:

```toml
mata-3d = { path = "../mata-3d" }
```

Run `cargo run -p mata-raycast --release`. Full 3D mesh development is
covered in `mata-3d/README.md`; test the standalone rotating demo with
`cargo run -p mata-3d --example rotating_mesh --release`.

Existing five-level campaign, lore, enemy AI, save format, and lift gameplay
remain unchanged. With a CPU renderer, FPS will depend on screen size and
number of triangles; F2 allows a local performance comparison. The build and
real FPS performance have not been verified in the authoring environment.
