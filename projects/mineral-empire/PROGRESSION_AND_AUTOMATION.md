# Progression and automation rebalance

## Progression

The unlock requirements and sequential prestige tiers remain intact, including the exponential step-ups in late prestige milestones. The previous progression rebalance throttled production too far below those requirements; it also applied severe diminishing returns to prestige rewards, upgrades, and technology at the same time. This made ordinary play feel stalled.

This update:

- Moves the Tier 2 production soft knee from 7 orders of magnitude before the next prestige requirement to 3 orders. Later tiers now use 4 orders rather than 7, 12, 17, etc.
- Reduces late-tier mining and equipment price inflation while keeping costs exponential.
- Makes earned prestige currency, owned prestige bonuses, and the first six research ranks more effective in later tiers. Returns still diminish for very high ranks.
- Retains prestige currency prerequisites, progressive ore access, very large high-tier thresholds, and the Tier 1 starting economy.

The goal is to unlock the next tier after meaningful investment rather than several hours spent entirely against the production softcap. Rates are calculated in the same `Big` notation as before and are not truncated to `f64`.

## Shared automatic hire / gear allocator

Auto-Hire and Auto-Gear are individually gated by their existing research unlocks. When either is enabled, they share one timer of **60 seconds of active play**. Enabling or disabling either feature restarts the timer; it pauses while the pause/save menu is open. Offline time awards ordinary mining income but does not trigger automatic purchases.

At the end of the minute, one optimizer examines every **unlocked, affordable** miner upgrade and equipment upgrade allowed by the enabled toggles. It estimates the **actual marginal production**, including equipment bonuses, currently active research, softcaps, and diversification benefits from opening a new shaft. It compares `income gained / price` using `Big` rather than rough revenue estimates. It buys the best option and recomputes the candidates before the next purchase; up to 128 purchases can be made in one window. If no purchase can repay its cost within 1 hour, it only allows investments costing at most 5% of the current treasury; otherwise it saves the cash and waits for the next minute. Unresearched or turned-off automations never purchase their category.

**This is a greedy marginal-ROI strategy**, not an exhaustive search over every combination of purchases. It is intended to prioritize genuine production gains without introducing expensive combinatorial optimization or risk of unbounded purchase loops.

## Existing saves

Save format **V7** adds one `f64` for the shared 60-second timer. V2–V6 saves still load; their clocks start at zero. Existing miners, equipment, prestige, research, and automation preferences are kept. Prestige resets preserve the shared purchasing timer so frequent automatic resets cannot postpone hiring forever; the optimizer evaluates the new mine state after each reset. Switching either auto-hire or auto-gear on/off restarts the minute. Re-saving a slot upgrades it to V7.

To verify in the complete Mata workspace:

```sh
cargo check -p mineral-empire
cargo test -p mineral-empire
```

Manual playtesting is useful for choosing the preferred target number of minutes between later tiers.
