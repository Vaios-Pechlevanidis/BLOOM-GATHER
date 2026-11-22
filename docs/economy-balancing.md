# Bloom & Gather — Economy & Balancing

Every number here comes from `assets/data/game_data.ron`. When a value changes
there, re-run `cargo test -p bloom_core` — the data-integrity tests enforce the
invariants in §2 and will fail if a change breaks them.

## 1. The finite money supply

The world holds **1,900 flowers** at a fixed **10 coins** each:

| Source | Coins |
|---|---|
| Selling every flower raw | 19,000 |
| Region completion rewards (100 / 200 / 350 / 500 / 750) | 1,900 |
| **Guaranteed minimum income** (worst case: never craft) | **20,900** |
| Expected income crafting sensibly (≈ ×2.2 on flowers) | ≈ 43,000 |

Because the supply is finite, balance is a budgeting problem rather than a
growth-curve problem: the total of everything you *must* buy has to fit inside
the worst-case income, and the total of everything you *can* buy should fit
inside the good-play income.

| Sink | Total |
|---|---|
| Required tools (all 9) | 15,300 (73 % of guaranteed income) |
| All upgrades to max | 19,633 (before discounts) |
| Everything | ≈ 34,900 (between raw and crafted income) |

So a player who never crafts can still buy every required tool, with about
5,600 coins to spare. A player who crafts can buy everything. Crafting is the
better strategy, but skipping it never soft-locks you.

## 2. Invariants (enforced by tests)

1. **Equal flower value.** Species have no price field; the only flower price is
   `economy.flower_base_value`. No upgrade or skill effect targets it.
2. **Bouquets beat raw sales.** For every recipe,
   `sell_price > ingredient_count × flower_base_value` (current minimum ×1.5).
   Margins rise with tier (×1.5 → ×3.0) so new tools always feel worth it.
3. **Required tools are affordable from guaranteed income.**
   `Σ required tool prices ≤ raw world value + region rewards`.
4. **Every required recipe is satisfiable from world supply.** For each recipe,
   the world total of every named species ≥ what it needs, and the world has
   at least `min_distinct_species` species.
5. **Regions are reachable without spending.** Region N's harvest threshold is
   ≤ the flowers in regions 1…N−1, and harvest XP alone from that many
   flowers reaches region N's level requirement.
6. **Tool level gates are reachable by harvesting alone.**
7. **Region flower counts are exact.** Per-species counts sum to the region's
   flower total; generation must place exactly that many.

## 3. Pacing per region (worst case = raw selling only)

XP: 3/flower harvested. Level L→L+1 costs 30 + 20(L−1).
Cumulative XP for level n = 30(n−1) + 10(n−1)(n−2).

| After clearing | Flowers | Raw coins (+rewards) | Harvest-only level | Tools gated ≤ that level | Their cumulative price |
|---|---|---|---|---|---|
| Sunny Meadow | 100 | 1,100 | 5 | Ribbon, Scissors, Basket | 1,000 |
| Lavender Fields | 300 | 3,300 | 9 | + Knife, Wrapping Paper | 3,500 |
| Blossom Valley | 650 | 7,150 | 14 | + Pro Gardening, Workbench, Advanced | 11,300 |
| Enchanted Forest | 1,150 | 12,650 | 18 | + Pro Florist Set | 15,300 |
| Moonlight Garden | 1,900 | 20,900 | 23 | — | 15,300 |

Reading: a raw-only player falls a little behind the tool curve from region 2
on and catches up by the end. A crafting player (×1.5–×3 on the flowers they
bind) stays ahead and also affords upgrades. That gap is the intended
incentive.

## 4. Recipe margins

| Recipe | Flowers | Raw value | Price | Margin | Coins gained per flower vs raw |
|---|---|---|---|---|---|
| Simple | 3 | 30 | 45 | ×1.50 | +5.0 |
| Colorful | 5 | 50 | 90 | ×1.80 | +8.0 |
| Romantic | 7 | 70 | 150 | ×2.14 | +11.4 |
| Wrapped Garden | 8 | 80 | 190 | ×2.38 | +13.8 |
| Woodland | 9 | 90 | 230 | ×2.56 | +15.6 |
| Premium | 10 | 100 | 270 | ×2.70 | +17.0 |
| Moonlit | 12 | 120 | 340 | ×2.83 | +18.3 |
| Royal | 15 | 150 | 450 | ×3.00 | +20.0 |

Species constraints stop "always craft the best recipe": Royal needs one each
of Hydrangea, Fairy Lily and Starpetal (only 40 Starpetals exist), Moonlit
needs Moonflowers, and so on. Choosing which recipe uses which flowers is the
central economic decision of the late game.

## 5. Anti-deadlock mechanisms

### 5.1 Region unlocks cost progress, not coins
Level and flowers harvested only go up, so a region can always be unlocked
eventually.

### 5.2 Progression guard on optional purchases (implemented in Phase U)
An optional purchase (upgrade) of price `p` is allowed only if

```
coins − p + G ≥ R
G = base_value × (unharvested flowers in world + flowers in inventory)
    + Σ sell value (with bonuses) of bouquets owned or being bound
    + unclaimed region completion coins
R = Σ current (discounted) price of required tools not yet owned
```

`G` counts only income the player can always obtain. At the start
`G = 20,900`, `R = 15,300`, so 5,600 coins of optional spending is always
allowed even for a non-crafter; crafting raises `coins` and so the headroom.
When the guard blocks a purchase, the shop explains why and what to buy first.
Required tools themselves are never blocked by the guard.

### 5.3 Keepsake reserve for required bouquets (Phase 4)
Flowers can be sold or crafted into other recipes, so the flowers needed for
required bouquets could be used up. Before any sale or craft, the game checks
that every *not-yet-crafted* required recipe can still be satisfied from
`inventory − (flowers about to be used) + unharvested world flowers`. The
check plans greedily: named species first, then distinct-species needs from
the most plentiful species, then "any" fill. Greedy planning can only be
stricter than a perfect solver, never looser, so the check is safe. If the
action would break feasibility, the excess is held back and the UI shows
"Set aside for required bouquets: …". Total flowers that can ever be held is
69 (the sum of all required recipe sizes).

### 5.4 Always-available selling
Individual flowers can always be sold (except the set-aside ones), so a player
with flowers is never without a path to coins.

## 6. Upgrade cost model

`cost(level) = round(base × growth^level)`, default `growth = 1.6`, where
`level` is the current level (so the first purchase costs `base`).
Sum over n levels = `base × (growth^n − 1) / (growth − 1)`:
5 levels = 15.81 × base, 4 levels = 9.26 × base, 3 levels = 5.16 × base.

| Upgrade | Base | Levels | Total |
|---|---|---|---|
| Nimble Hands | 80 | 5 | 1,265 |
| Wider Reach | 150 | 5 | 2,371 |
| Deep Basket | 150 | 5 | 2,371 |
| Wide Rakes | 400 | 3 | 2,064 |
| Quick Binding | 100 | 5 | 1,581 |
| Bulk Orders | 200 | 4 | 1,851 |
| Garden Boots | 250 | 4 | 2,314 |
| Market Stall | 150 | 5 | 2,371 |
| Haggler | 120 | 5 | 1,897 |
| Pretty Packaging | 300 | 3 | 1,548 |
| **Total** | | | **19,633** |

(Each level's price is rounded to whole coins, hence 2,371 rather than
15.81 × 150.)

Stacking rules, all in `bloom_core::game::growth`:

* Effects of the same kind add up across upgrades and skills.
* Price discounts (Haggler, Better Deals, Reduced Upgrade Costs) and the
  region threshold reduction are floored at 50 % of base; time reductions
  (pick cooldown, binding time) at 25 %. Validation rejects content whose
  maximum levels could pass a floor, so no level is ever wasted: pick
  cooldown tops out at −45 %, binding time at −60 %, tool prices at −23 %.
* Bouquet bonuses (Market Stall, Artistic Arrangements, Improved Profits,
  Pretty Packaging, Premium Packaging) add up and multiply the recipe's base
  price (up to +34 % for every bouquet, +60 % for wrapped ones). They never
  apply to raw flowers, so invariant 1 holds structurally: no effect kind
  targets the flower price.

## 7. XP & skill points

* Harvest-only XP: 1,900 × 3 = 5,700 → level 23.
* With discoveries (17 × 25 = 425), region completions (5 × 150 = 750),
  first crafts (8 × 50 = 400) and ≈ 600 bouquets (≈ 6,000 XP) a full
  playthrough reaches about level 30 (the cap).
* SP: 1 per level from 2 (up to 29), +5 region completions, +2 collection
  milestones (8 and 16 species), +up to 5 from Collection Bonuses ≈ 41 SP
  (achievements in Phase P may add a few), against a tree that costs 44 SP to
  max — the player chooses a focus until late game.

## 8. Tuning knobs

| Want to… | Change |
|---|---|
| Make early game faster | lower `xp_per_level_step`, Ribbon/Scissors price |
| Make crafting matter more | widen recipe margins, raise upgrade totals |
| Make raw selling more forgiving | raise region completion coins |
| Lengthen the game | raise region flower counts (and tests will flag invariant breaks) |
