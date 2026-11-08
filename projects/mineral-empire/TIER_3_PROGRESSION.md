> Historical balance notes. For the current rebalance, see [PROGRESSION_AND_AUTOMATION.md](PROGRESSION_AND_AUTOMATION.md).
> Some progression settings below have been relaxed.

# Tier 3 onward: exponential prestige progression

This rebalance targets overly fast post-Tier-3 unlocks. It changes economy
constants and calculation curves only. Save structure stays V6, so no save
migration or new dependencies are required.

## Unlock thresholds

| Tier | Name | Run earnings | Lifetime currency required from previous tier |
| ---: | --- | ---: | ---: |
| 1 | Land Claims | `1e6` | — |
| 2 | Industrial Forge | `1e13` | 6 Claims |
| 3 | Mining Conglomerate | `1e23` | 4 Forge Cores |
| 4 | Star Excavation | `1e65` | 25 Shares |
| 5 | Deep Core | `1e150` | 50 Relics |
| 6 | Continental Union | `1e330` | 100 Core Fragments |
| 7 | Oceanic Extraction | `1e710` | 200 Union Seals |
| 8 | Lunar Syndicate | `1e1510` | 400 Abyss Pearls |
| 9 | Asteroid Cartel | `1e3180` | 800 Moon Crests |
| 10 | Nebula Foundry | `1e6700` | 1,600 Asteroid Bonds |
| 11 | Galactic Combine | `1e14100` | 3,200 Nebula Embers |
| 12 | Quantum Rift | `1e29600` | 6,400 Galaxy Marks |
| 13 | Dimensional Dominion | `1e62100` | 12,800 Rift Keys |
| 14 | Eternal Excavation | `1e130000` | 25,600 Realm Sigils |

The exponent gaps grow roughly twofold per prestige level, not just by a
fixed number of digits. `Big` can represent these enormous thresholds.

## Bonuses and research

- Prestige balances at most 1,000 retain their old multiplier formula.
  Beyond 1,000, only the excess benefits from fractional powers; the
  denominator becomes larger at higher prestige tiers.
- Repeatable research gains its original benefit through rank 12.
  Above that, each doubling of additional ranks adds approximately one
  effective rank, preventing x100 purchases from immediately creating
  unbounded production multipliers.
- Cost discounts, gear amplification, quantum drilling, ore mastery,
  signature boosts, and lifetime legacies also use the diminishing-return
  treatment. Automation unlocks are unchanged.
- Prestige reward growth is unchanged for Tiers 1–3 (ratio 7/25).
  For Tier 4 onward the reward exponent decreases by tier from 1/5
  at Tier 4 to 1/15 at Tier 14.

## Save and gameplay compatibility

No new save format or fields. Existing slot saves (V2–V6), ores, research
levels, prestige currencies, and automation preferences persist. The updated
formulae apply immediately. Previously accumulated gigantic cash or lifetime
currencies might allow an initial fast reset; later runs use the harder
thresholds and dampened bonuses. Consider testing on a separate save slot.

## Verification

```sh
cargo check -p mineral-empire
cargo test -p mineral-empire
cargo run -p mineral-empire --release
```

Tests cover preserved early thresholds, increasing late thresholds and prior-
prestige currency requirements, softcaps, reduced late reward growth, and
existing Auto-Prestige priority rules. Manual playtesting is still needed
to tune time-to-tier estimates and ensure the pace is enjoyable.
