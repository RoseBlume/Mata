# Save menu and pacing update

## Menu

The game now starts at the save menu. `MENU` in the upper-right corner of the game opens it again; Escape resumes the game (or dismisses the reset confirmation). Buttons support mouse and Android touch through the existing custom MataWin hit-testing path.

- **CONTINUE** returns to the active slot. Mining and automation pause while the menu is open.
- **SAVE** writes progress to the selected slot; gameplay auto-saves every 25 seconds.
- **NEW SLOT** creates a fresh independent save and immediately switches to it. Up to 64 slots are supported.
- **LOAD / PLAY** loads the chosen slot. The current slot is saved first when possible.
- **RESET CURRENT** opens a confirmation screen and makes a separate `.bak` backup before resetting only that slot.

Original saves remain compatible: the existing `save.dat` automatically maps to **Slot 1**, additional slots use `slot-02.dat` etc., and `active-slot.txt` remembers which slot was most recently selected. All slot files retain the existing `MNMPIRE6` CRC-protected save format and V2–V5 migration code. Files and backups live in the same application save directory as before. If a slot is corrupt, the game will not overwrite it by auto-save; load/create another slot or explicitly reset it.

## Slower progression

The pace was reduced without a save format change:

| Component | Previous | Now |
|---|---:|---:|
| Base mine production | 100% | 28% |
| Miners' price multiplier per new rig | 1.19–1.31 | 1.29–1.41 |
| Starting gear cost relative to ore base | 12× | 18× |
| Gear price growth | 3.1–4.0× | 3.7–4.6× |
| Gear output gain | 1.9–2.7× | 1.7–2.3× |
| Next-ore unlock fraction of base cost | 35% | 65% |
| Extraction research output multiplier | 1.45×/rank | 1.30×/rank |
| Logistics research | 1.20×/rank | 1.14×/rank |
| Refining research | 1.80×/rank | 1.52×/rank |
| Quantum research | 1.35×/rank | 1.25×/rank |
| Prestige reward exponent | 0.32 | 0.28 |
| Research price growth | 2.7 / 2.35–2.55 | 3.1 / 2.7–2.9 |

This applies to both new and existing saves, so previously accumulated research/currency remains intact; later tiers can still accelerate considerably.

## Verify

From the Mata workspace:

```sh
cargo check -p mineral-empire
cargo test -p mineral-empire
cargo run -p mineral-empire --release
```

Manual checks: create a second slot, invest in a mine, save, switch to Slot 1, switch back and verify the mine is preserved; reset Slot 2, verify Slot 1 is untouched and a `slot-02-before-reset-*.bak` file exists. Verify the menu can be opened on maximize/restore and that Escape dismisses confirmation without resetting.
