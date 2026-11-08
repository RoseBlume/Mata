# Mineral Empire: The Deep Frontier

**Latest balance update:** Prestige progression has been eased after the previous
slowdown. Auto-Hire and Auto-Gear now share a 60-second in-game purchase timer
and choose between affordable upgrades using actual marginal earnings per
currency, rather than buying repeatedly on each short polling interval. The
controls display a countdown on desktop. Saves now write **V7**, while loading
existing V2–V6 slots. Details: [PROGRESSION_AND_AUTOMATION.md](PROGRESSION_AND_AUTOMATION.md).

The earlier balance and V6 notes below are historical context.

**New:** A full save-slot menu with Continue, Save, New Slot, Load, and a confirmed Reset Current Slot. The menu opens on launch and with the in-game MENU button. Slot 1 uses the original `save.dat`; other slots are independent. The economy has also been slowed significantly. See [MENU_AND_BALANCE.md](MENU_AND_BALANCE.md).

An **original**, playable mineral-mining incremental game inspired by the broad idea of layered prestige in incremental games. No source code, prose, artwork, UI, music, or balance tables were copied from the Antimatter Dimensions source archive.


## Tier 3+ prestige pacing rebalance

**Tiers 1–3 keep their existing unlock thresholds.** Starting with Tier 4,
run-earnings thresholds rise with an approximately doubling *gap between decimal
exponents* each tier. The previous-tier lifetime-currency requirement also
doubles with every higher tier: Tier 4 requires 25 Shares, Tier 5 requires 50
Relics, Tier 6 requires 100 Core Fragments, and so on.

Large prestige balances above 1,000 now use a diminishing-returns curve when
calculating production bonuses. Repeatable research has full effect through
rank 12, then adds roughly one effective rank each time the number of additional
ranks doubles. High-tier prestige rewards increase more slowly when a run
substantially overshoots its threshold. These changes reduce the snowball
from Auto-Research and Auto-Prestige while retaining the early-game economy.

**Existing saves remain compatible without migration.** New requirements apply
to existing progress, but ore counts, currency, upgrades, save slots, and
prestige history are not deleted. Players with unusually large balances earned
before this rebalance may be able to ascend once quickly; the normal run reset
then exposes the slower progression curve. Back up saves before updating.

## New: Bulk Research, Auto-Research and Stage Specializations

- **Research purchasing:** select **x1, x10, x50, or x100** at the top of the Research tab. The actual escalating cost is calculated across all requested ranks; if you cannot afford the full bundle, it buys as many as possible without overspending.
- **Auto-Research:** purchase the Auto-Research upgrade in any prestige stage, then toggle it in the Research tab. It purchases up to eight affordable research levels on each automation tick. Upgrades reset normally when a higher prestige reset clears their tier.
- **Expanded tier-specific upgrades:** each of 14 prestige stages now features four uniquely named specialist projects in addition to the eight original research branches and the Auto-Research unlock: signature output, ore-class mastery, engineering/discount improvements and lifetime legacy resonance. Effects vary by prestige stage.
- **Text fitting:** mining, equipment, prestige and research buttons now measure glyph widths at the active DPI scale and reduce or elide long scientific-notation prices rather than overflow the button.
- **Save V6:** `MNMPIRE6` stores the editable large-number Auto-Prestige minimum gain. V2–V5 saves load and migrate automatically. **Back up your save before updating.**

## Deep Frontier Expansion: 60 ores, 14 prestige tiers, Auto-Prestige

**New in this release:** 30 additional ore types (30 → 60), 10 new prestige tiers
(4 → 14), and an eighth research branch, **Auto-Prestige**, on every prestige tier.
The previous 30-ore economy and UI/scroll/resize fixes are preserved.

### Ten additional prestige tiers

| Tier | Prestige | Currency | Run-earnings threshold |
|---|---|---|---|
| 5 | Deep Core | Core Fragments | `1e150` |
| 6 | Continental Union | Union Seals | `1e330` |
| 7 | Oceanic Extraction | Abyss Pearls | `1e710` |
| 8 | Lunar Syndicate | Moon Crests | `1e1510` |
| 9 | Asteroid Cartel | Asteroid Bonds | `1e3180` |
| 10 | Nebula Foundry | Nebula Embers | `1e6700` |
| 11 | Galactic Combine | Galaxy Marks | `1e14100` |
| 12 | Quantum Rift | Rift Keys | `1e29600` |
| 13 | Dimensional Dominion | Realm Sigils | `1e62100` |
| 14 | Eternal Excavation | Eternity Shards | `1e130000` |

Each higher prestige tier requires lifetime currency from the preceding tier,
builds its own balance and research, contributes its own production multiplier,
and resets all lower-tier balances/research on ascension. Lifetime progress
remains. New tiers continue to use `Big` arithmetic rather than converting
exponents into an `f64`.

### Thirty additional resources

Bauxite, Chromite, Molybdenite, Vanadinite, Zircon, Spodumene, Monazite,
Columbite, Tantalite, Scheelite, Peridot, Aquamarine, Opal, Jadeite,
Grandidierite, Jeremejevite, Musgravite, Taaffeite, Serendibite, Red Beryl,
Hibonite, Eosphorite, Starlight Ore, Solarite, Nebulium, Quasar Shard,
Chronite, Riftstone, Singularity Glass, and Eternium.

The final eight are fictional speculative ores. All 30 new tiers include embedded
PNG artwork, unlock thresholds, equipment growth and production scaling.

### Auto-Prestige (all lower tiers)

Purchase **Auto-Prestige** research in any prestige tier, then open **Prestige**.
Once you have reached at least tier 2, the toggle will automate **every prestige
level below your current highest attained tier**, but never the current tier.
For example, at tier 5 it considers tiers 1–4. It always chooses the **highest
eligible lower tier** first, respecting each tier's normal prerequisites.

The old TARGET selector has been removed. Instead, click the **MIN GAIN** text
field and enter a positive amount, such as `10`, `25000`, or `1e500000`.
Typing replaces the selected existing value; Backspace removes characters;
**Enter** commits and saves, **Escape** cancels. On narrow/touch screens an
in-game number keypad appears, including exponent, delete, cancel, and OK.
An invalid, zero, or negative amount is rejected without losing the previous setting. Click outside the
field to commit. The number uses the game's arbitrary-length decimal exponent
format rather than an `f64` threshold.

The first owned Auto-Prestige research rank gives a 15-second cooldown, and
higher ranks reduce it to a minimum of 3 seconds. An automatic reset saves
immediately. Research may need repurchasing if a higher prestige reset clears
its tier. Manual prestige still requires two clicks.

### Save format and compatibility

New saves use **V6** (`MNMPIRE6`) with CRC32 validation and store all 60 ores,
14 prestige tiers, thirteen research tracks, and a complete `Big` minimum gain.
V2, V3, V4 and V5 saves are loaded and migrated without resetting progress;
older five-step minimum-gain presets are converted to the matching value. **Back up your save before upgrading.**
Existing save locations remain unchanged.

### Platform requirements

The game now depends directly on `mata-win` for native windows and framebuffers,
`mata-ui` for its font renderer and UI geometry, and on `mata-image` and
`mata-audio` for image and sound support. No new third-party UI toolkit is needed. The original
MataWin Android NativeActivity entry point is preserved; no new Java is needed.

## Previous expansion: 30 ores and automated equipment

The game now contains **30 distinct ore tiers** (the original 10 and 20 additional ores in the previous release), with new
mineral PNGs embedded in `assets/ores`. The last three high-tier materials are fictional.
Each ore has different equipment cost growth and production scaling, rewarding a variety
of mine-development strategies.

Research contains **seven** tracks at each prestige layer:

| Track | Effect |
| --- | --- |
| Extraction | 1.45× global production per rank |
| Logistics | 1.20× global production per rank |
| Automation | Unlocks AUTO-HIRE |
| Auto-Gear | Unlocks AUTO-GEAR, which purchases affordable equipment automatically |
| Refining | 1.80× global production per rank |
| Prospecting | Reduces mining rig and equipment prices |
| Quantum Drill | 1.35× production per rank |

Auto-Hire and Auto-Gear can be toggled independently in Empire Controls or Ledger.
The automation chooses purchases by estimated incremental production per credit,
with bounded purchases per update to avoid freezing the UI.

### Numbers larger than `f64`

`src/big.rs` introduces `Big`, a pure-Rust signed scientific number:

- A floating-point mantissa stores approximately **15–16 significant decimal digits**.
- The decimal exponent is stored in an arbitrary-length base-1,000,000,000 integer, not `i32`,
  `i64`, or `f64`. Exponents can exceed `1e308` and even `u64::MAX`.
- Supports addition, subtraction, multiplication, division, whole-number powers,
  fractional powers used in prestige rewards, ordering, and lossless exponent serialization.
- Large positive numbers are not clamped to `1e290` or displayed as `MAX`.
- Precision is **not** arbitrary: insignificant terms are rounded away in addition/subtraction,
  and mantissas have the precision of an `f64`. Exponent length is limited in practice by memory
  and a conservative 16,384-character parser limit on loaded saves. No computer can store truly
  infinite digits, but the format has no fixed maximum exponent.

Saves are written in version 5 with a checksum. **Previous versions 2, 3 and 4 are loaded and migrated**:
original miners, equipment, balances, research, achievements and prestige remain intact, while
new ore slots and research branches start at zero. Back up your save before upgrading.

## Gameplay

Start with **$25**. Hire coal miners, earn credits per second, expand into sixty resources, upgrade rigs, unlock research and automation, then prestige repeatedly to grow your production. The original roster is Coal, Iron, Copper, Silver, Gold, Platinum, Diamond, Emerald, Uranium, and Mythril; twenty additional tiers extend the game through Void Crystal.

Your ore credits are an automatically sold commodity; production never requires manual clicks. Every mineral has separate mine counts, equipment levels, rates, and total extracted statistics. Prices increase geometrically; equipment increases production by ore-specific multipliers, and equipment costs vary by shaft. Buy one, ten, or as many as you can afford.

### Prestige layers

| Layer | Revenue to reset | Earn | What resets | Permanent benefit |
|---|---:|---|---|---|
| Land Claims | $1 million in one run | Claims | Mines, equipment, cash | More production and research |
| Industrial Forge | $10 trillion in one run, and six lifetime Claims | Forge Cores | Mine economy + lower Claims and research | Multiplier retained over Claims |
| Mining Conglomerate | $100 sextillion in one run, and four lifetime Forge Cores | Shares | Mine economy + lower prestige balances | Multiplier retained over Forge Cores |
| Star Excavation | $100 undecillion in one run, and three lifetime Shares | Relics | Mine economy + lower prestige balances | Highest-tier multiplier |

All fourteen prestige tiers use **two-step confirmation** to avoid accidental resets. Each reset earns at least one unit, with larger rewards for overshooting the target. Lifetime quantities and achievements survive resets. Research currency is spent from the corresponding current prestige balance.

All fourteen stages have thirteen research tracks, including extraction, refining, prospecting, quantum drills, and separate Auto-Hire and Auto-Gear unlocks. Automated purchases favor production gains per credit.

## Build and run

The archive has this layout:

```text
mineral-empire-package/
  Cargo.toml                    # workspace
  mineral-empire/              # game; four direct Mata dependencies
    Cargo.toml
    Mata.toml
    src/{main,lib,big,engine,save,sound,ui}.rs
    assets/ores/*.png
    assets/fonts/DejaVuSans.ttf
  mata-win/                      # native windows, events, framebuffer
  mata-ui/                      # widget, font and display APIs
  mata-image/                    # supplied MataImage source
  mata-audio/                   # supplied MataAudio source with Android MediaSession
  mata-fonts/                    # mata-ui font dependency
  mata-video/                   # optional mata-ui media dependency
  remata-macros/                # small required proc-macro helper missing from upload
```

Run from the **outer directory**:

```bash
cargo run -p mineral-empire --release
```

Or install your Mata Bundler and run from the **game's** `mineral-empire/` directory:

```bash
mata dev
mata build --release
```

Desktop builds require a compatible Rust toolchain plus MataWin's relevant window libraries / linkers. An X11/Wayland display is required on Linux.

## Android

The game has a true Rust `android_main(AndroidApp)` entry point in `src/lib.rs`, and the supplied `Mata.toml` is set to NativeActivity. No custom Java Activity is necessary. With your updated Mata Bundler and Android SDK/NDK already configured:

```bash
cd mineral-empire
mata android build
mata android dev
```

The compiled shared library is `libmineral_empire.so`. The NativeActivity manifest uses `android.app.NativeActivity` and `android.app.lib_name = mineral_empire`. `android_main` creates a MataWin window from the supplied AndroidApp, processes touch/resize events, saves to its private internal data directory, and exits cleanly on close.

MataWin's current NativeActivity backend is used deliberately rather than GameActivity: its input system maps primary touches to mouse events, which this game supports, including vertical swipe scrolling.

**Caution:** Android APK builds and physical-device testing were not performed in the creation environment. The Mata crates are bundled as user-supplied code plus the missing macro helper; future changes to your crates may require small API adjustments.

## Controls

- Click/tap **HIRE** on an unlocked mine. Select **x1/x10/MAX** to set the amount.
- Click/tap **GEAR** to multiply that mine's production by its specialty modifier.
- Switch between **OPERATIONS**, **PRESTIGE**, **RESEARCH**, and **LEDGER**.
- In PRESTIGE, click ASCEND, then **CONFIRM RESET**.
- Buy Automation / Auto-Gear research, then independently toggle **AUTO-HIRE** and **AUTO-GEAR** in the Operations sidebar (desktop) or Ledger (mobile).
- Mouse wheel or swipe vertically to scroll the mine/research pages.
- **SAVE**, **SOUND**, **AUTO-HIRE**, and **AUTO-GEAR** controls are included in the UI. Keyboard shortcut `S` also saves.

Saves are written every 25 seconds and at exit, and immediately after prestige. The save is versioned and CRC-checked. Offline earnings are calculated on launch, capped at **12 hours**, with no automated purchasing performed for the offline period.

On Windows, save is stored at `%APPDATA%/mineral-empire/save.dat`; on Linux, at `$XDG_DATA_HOME/mineral-empire/save.dat` or `~/.local/share/mineral-empire/save.dat`; on Android, in the app-specific internal files/data path. Sound effects are small WAV files synthesized locally on first launch and streamed using **mata-audio**, without shipping music/audio dependencies.

## Source and licenses

- Game mechanics, interface, shapes, synthesized sound formulas, and generated ore sprites are original.
- `mata-win`, `mata-image`, `mata-audio`, `mata-fonts`, and `mata-video` are derived from the Mata sources supplied in this conversation; their own licensing still applies.
- The bundled DejaVu Sans font has a separate permissive license in `assets/fonts/FONT_LICENSE.txt`.
- The `remata-macros` crate is a small independent compatibility shim implementing the numeric-tag conversion expected by the supplied `mata-image` source. It is an internal build dependency, not a direct dependency of the game.

## Tests

```bash
cargo test -p mineral-empire
cargo check -p mineral-empire --all-targets
```

Engine tests cover purchases, geometric-cost affordability, layered prestige, and invalid operations. Save tests cover round-trip serialization. No compiler or Android SDK is installed in the task environment, so these commands must be run on your machine to verify your specific Mata crate versions.

## PNG-only MataImage feature fix

This bundle includes a `mata-image` fix for projects that disable its default
features and select `features = ["png"]`. Its `AnimationDecoder<'a>` public
API remains present even when both GIF and WebP decoders are disabled. It
uses an unconstructible marker variant to preserve the borrowed lifetime, so
only optional animated formats incur decoder code. Additionally, the shared
RGBA8 decoder macro is compiled only when a format that uses it is enabled.

Verify on a machine with Rust installed:

```sh
cargo check -p mata-image --no-default-features --features png
cargo check -p mineral-empire
cargo test -p mata-image --no-default-features --features png
```


### Slower high-tier progression

From Tier 2 onward, mining production has tier-specific diminishing returns, deeper ores unlock with later prestiges, and research/gear bonuses become more expensive. Tier 1 behavior is unchanged. See `../PROGRESSION_REBALANCE.md` for detailed notes.

## MataWin / Mata UI split

The GUI code in `src/ui.rs` remains custom software-rendered but imports
`FontRenderer` and `Rect` from `mata_ui::widgets`. Native presentation,
events, and framebuffers are still imported from `mata-win`. Ensure your
workspace has `mata-ui/` as a member alongside `mata-win/`.
