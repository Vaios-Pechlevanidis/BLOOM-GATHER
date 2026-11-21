# Bloom & Gather — Game Design

All concrete numbers below are the initial values in
`assets/data/game_data.ron`; that file is the source of truth. See
`economy-balancing.md` for the math behind them.

## 1. Pillars

1. **Cozy, not boring** — gentle visuals and sound, but every few minutes a new
   tool, recipe, species or region.
2. **Finite and completable** — 1,900 hand-placed-by-seed flowers across five
   regions. Each can be picked once. When they're all picked and the
   collection is complete, the game ends with a proper finale.
3. **Flowers are equal; arrangements are valuable** — every flower sells for
   the same 10 coins. Rarity matters for recipes and collection, not price.
4. **Never stuck** — region access depends only on progress that can't be
   spent (flowers harvested, level). Required tools are always affordable
   (see the progression guard in `economy-balancing.md`). Flowers needed for
   required bouquets are protected from being sold or used elsewhere.

## 2. Presentation: first-person 3D

The player explores each region on foot in first person: WASD to walk, Shift
to sprint, Space to jump, the mouse to look. Flowers are picked by looking at
them and pressing E; the workshop is a building you walk into. Menus (inventory,
map, skill tree, collection) open as in-game overlays and pause the world.

* Movement tuning (metres/seconds, in `game_data.ron`): walk 4.2 m/s, sprint
  6.8 m/s, jump 1.05 m, step-up 0.35 m, slopes up to 42°, interact range 3 m.
* Comfort: adjustable mouse sensitivity and field of view, optional head
  movement, invert look; no camera shake.
* Regions are compact and dense rather than huge and empty: Sunny Meadow is
  110×80 m, Moonlight Garden 230×160 m, fenced, with scenery beyond the fence.
* Solid props (trees, rocks, fences, buildings) block movement; low stones
  can be stepped over; decorative vegetation never counts as a flower.
* Every region has landmarks to navigate by: a bench beside the path, a
  stone well in an open spot, and a signpost where the path leads on to the
  next region. Grass, butterflies, bees, drifting clouds, a breeze and
  birdsong make the meadow feel alive without affecting gameplay.

## 2b. Core loop

Explore region → pick flowers (discovering species) → sell or keep →
buy tools/upgrades → pick faster and in bigger areas → craft bouquets in the
workshop → sell bouquets → unlock next region → … → harvest every flower,
craft every required bouquet → ending → optional free roam.

The HUD always shows one **next objective** (e.g. "Reach level 3 and harvest
60 flowers to open Lavender Fields", "Buy the Flower Binding Ribbon to start
crafting").

## 3. Flowers

17 species in four rarity tiers by world abundance. A flower's selling value is the
single global `economy.flower_base_value` (10 coins) — species definitions
have no price field, so a rare flower *cannot* be configured to sell for more.

| Species | Rarity | Shape | First region | World total |
|---|---|---|---|---|
| Daisy | Common | ray of petals, domed centre | Sunny Meadow | 45 |
| Daffodil | Common | nodding trumpet | Sunny Meadow | 80 |
| Poppy | Uncommon | four cupped petals | Sunny Meadow | 35 |
| Bluebell | Uncommon | arching stem of bells | Sunny Meadow | 55 |
| Meadow Clover (wildflower) | Uncommon | low pompom, trefoil leaves | Sunny Meadow | 10 |
| Lavender | Common | upright spikes | Lavender Fields | 160 |
| Tulip | Common | closed cup | Lavender Fields | 105 |
| Sunflower | Uncommon | tall, sideways disc | Lavender Fields | 70 |
| Rose | Uncommon | layered spiral | Blossom Valley | 250 |
| Peony | Uncommon | ruffled ball | Blossom Valley | 155 |
| Hydrangea | Rare | mophead of florets | Blossom Valley | 70 |
| Foxglove | Common | tall spike of bells | Enchanted Forest | 230 |
| Wild Orchid | Rare | arching asymmetric blooms | Enchanted Forest | 140 |
| Fairy Lily | Very Rare | recurved star trumpet, glows | Enchanted Forest | 55 |
| Night Jasmine | Common | starry shrub, faint glow | Moonlight Garden | 220 |
| Moonflower | Rare | wide pale trumpet, glows | Moonlight Garden | 180 |
| Starpetal | Very Rare | five-pointed star, glows | Moonlight Garden | 40 |

Each species is a distinct 3D model built from its `shape` family, colours
and botanical `height`. All flowers are displayed (and picked) at
`world.flower_scale` = 2.2× life size so they read well from eye height.
Decorative vegetation never counts as a flower.

The first time a species is harvested it is **discovered** (XP + collection
entry). The collection is complete when all 17 are discovered.

## 4. Harvesting

| Stage | Unlocked by | Mode | Effect |
|---|---|---|---|
| 1 Hand picking | start | Single | 1 flower per click, 0.35 s cooldown |
| 2 Garden Scissors | tool | Single | cooldown ×0.6 |
| 3 Harvesting Basket | tool | Radius | click picks every flower within 45 units (max 6) |
| 4 Professional Gardening Tools | tool | Multi (sweep) | hold & drag to keep picking in a 70-unit radius (max 15 per tick) |
| 5 Advanced Harvesting Equipment | tool | Area | drag a rectangle up to 320×320 units (max 80 per action) |

The player can switch between any unlocked mode (HUD mode bar, keys 1–4).
Upgrades and skills scale cooldown, radius, cap and area size; they never
unlock a mode on their own.

## 5. Tools

All nine tools are permanent, required for completion, and each unlocks real
functionality:

| # | Tool | Price | Level | Unlocks |
|---|---|---|---|---|
| 1 | Flower Binding Ribbon | 100 | 2 | Simple Bouquet; crafting itself |
| 2 | Garden Scissors | 300 | 3 | faster picking; Colorful Bouquet |
| 3 | Harvesting Basket | 600 | 5 | Radius mode |
| 4 | Florist Knife | 1,000 | 7 | Romantic Bouquet; Woodland Arrangement (with Workbench) |
| 5 | Wrapping Paper Set | 1,500 | 9 | Wrapped Garden Bouquet; Moonlit Bouquet (with Pro Set) |
| 6 | Professional Gardening Tools | 2,000 | 11 | Multi (sweep) mode |
| 7 | Florist Workbench | 2,600 | 12 | Woodland Arrangement; batch capacity 5 → 25 |
| 8 | Advanced Harvesting Equipment | 3,200 | 14 | Area mode |
| 9 | Professional Florist Set | 4,000 | 16 | Premium, Moonlit, Royal; batch capacity → 50 |

## 6. Bouquets

Crafting happens only in the workshop, on explicit player choice, with a
quantity picker (up to the batch capacity). Ingredients are validated and
consumed atomically; nothing is consumed on failure.

| Recipe | Ingredients | Tools | Price | vs. raw |
|---|---|---|---|---|
| Simple Bouquet | 3 any | Ribbon | 45 | ×1.50 |
| Colorful Bouquet | 5, ≥3 species | Ribbon, Scissors | 90 | ×1.80 |
| Romantic Bouquet | 7, ≥2 Rose | Ribbon, Knife | 150 | ×2.14 |
| Wrapped Garden Bouquet | 8, ≥4 species | Ribbon, Wrapping Paper | 190 | ×2.38 |
| Woodland Arrangement | 9, ≥2 Foxglove, ≥1 Wild Orchid | Knife, Workbench | 230 | ×2.56 |
| Premium Bouquet | 10, ≥5 species | Pro Florist Set | 270 | ×2.70 |
| Moonlit Bouquet | 12, ≥3 Moonflower, ≥2 Night Jasmine | Wrapping Paper, Pro Florist Set | 340 | ×2.83 |
| Royal Bouquet | 15, ≥1 Hydrangea, ≥1 Fairy Lily, ≥1 Starpetal | Pro Florist Set | 450 | ×3.00 |

All eight are required for completion. A recipe is **discovered** when the
player owns all its tools and has discovered every species it names.

**Ingredient selection:** the player can let the workshop auto-fill (uses the
most plentiful non-reserved species first) or pick species manually.

**Reserve:** while a required recipe has never been crafted, the exact flowers
it needs (e.g. 2 Roses for Romantic) are *set aside*: they can't be sold or
used by other recipes, and the UI says so. The reserve is released after the
first craft. Combined with the world totals this guarantees every required
bouquet stays craftable.

## 7. Upgrades (Phase 5)

Leveled, coin-priced, `cost(level) = base × growth^level`. Effects are
multiplicative and shown as before → after in the shop.

* Harvesting: Nimble Hands (cooldown −6 %/lvl, 5), Wider Reach (radius
  +10 %/lvl, 5), Deep Basket (max per action +20 %/lvl, 5), Wide Sweep (area
  size +10 %/lvl, 3).
* Crafting: Quick Binding (craft time −10 %/lvl, 5), Bulk Orders (batch
  capacity +5/lvl, 4), Careful Hands (5 % chance/lvl a bouquet is
  "exquisite", selling +25 %, 4).
* Economy: Market Stall (bouquet price +4 %/lvl, 5), Haggler (tool price
  −3 %/lvl, 5), Pretty Packaging (wrapped/premium recipes +6 %/lvl, 3;
  requires Wrapping Paper Set).

No upgrade raises the price of individual flowers.

## 8. Skill tree (Phase 6)

Five branches, 21 skills, skill points (SP) from level-ups (1 per level from
level 2), region completions (+1 each), collection milestones (8 and 16
species: +1 each) and selected achievements.

* **Harvesting:** Quick Fingers (cooldown −5 %/lvl, 3) → Double Harvest (8 %
  chance/lvl a Single pick also picks the nearest neighbour, 3) → Improved
  Radius (+8 % radius/lvl, 3; requires Harvesting Basket) → Master Picker
  (+25 % max per action; requires Pro Gardening Tools).
* **Botany:** Botanical Knowledge (+15 % discovery XP) → Flower Recognition
  (highlights undiscovered species on the field) → Rare Discoveries
  (minimap markers for rare/very rare flowers) → Collection Bonuses (+1 SP per
  region whose species are all discovered). *No Botany skill touches flower
  price.*
* **Floristry:** Basic Florist (+10 % crafting XP) → Efficient Binding (craft
  time −10 %/lvl) → Artistic Arrangements (+5 % bouquet price; requires
  Garden Scissors) → Premium Packaging (+8 % on wrapped recipes; requires
  Wrapping Paper Set) → Master Florist (+10 batch capacity; requires
  Workbench).
* **Efficiency:** Better Deals (tool price −4 %) → Reduced Tool Costs
  (upgrade price −4 %/lvl) → Improved Profits (bouquet price +3 %/lvl) →
  Efficient Production (5 % chance a craft refunds one non-reserved flower
  — capped so it never creates flowers beyond the finite world total: refunds
  only return flowers consumed in that craft).
* **Exploration:** New Meadows (region unlock harvest threshold −10 %) →
  Advanced Exploration (camera pan speed/zoom range +) → Rare Flower Detection
  (remaining-rare counter per region) → Region Unlock Bonuses (+50 coins and
  +50 XP on each region unlock).

Skills never grant a tool or unlock a mode/recipe that requires a tool.

## 9. Levels & XP

XP: 3 per flower, 25 per species discovered, 4 + 2×tier per bouquet, 50 for a
recipe's first craft, 150 per completed region, plus achievement rewards.
XP to go from level L to L+1 = 30 + 20 × (L − 1). Max level 30.
Harvesting alone (5,700 XP) reaches level 23 — above every tool and region
gate — so level is never a blocker; crafting just gets you there faster.

## 10. Regions

| Region | Flowers | Unlock | Mood |
|---|---|---|---|
| Sunny Meadow | 100 | start | warm sunlight, wildflowers |
| Lavender Fields | 200 | level 3 + 60 harvested | purple rows, gentle wind |
| Blossom Valley | 350 | level 7 + 220 harvested | flowering trees, drifting petals |
| Enchanted Forest | 500 | level 11 + 500 harvested | mushrooms, filtered light |
| Moonlight Garden | 750 | level 15 + 950 harvested | night, glowing plants |

Each shows total / harvested / remaining / species discovered / completion %.
Completion grants coins, XP and 1 SP, and is permanent.

## 11. Achievements (Phase 7)

Harvesting (First Flower, 100, 1,000, Master Harvester = all 1,900),
Crafting (First Bouquet, 10, 100, Master Florist = 500),
Exploration (First Discovery, First Region Completed, All Regions Unlocked,
World Explorer = all regions complete), Collection (First Rare Flower, Half
Collection, Complete Collection), Completion (All Tools, All Recipes
Discovered, All Required Bouquets Crafted, Bloom & Gather Completed).
Optional achievements reward small coin/SP bonuses or cosmetics and are never
required for the ending.

## 12. Ending

Completion requires all of: every flower harvested, every region complete,
every required tool owned, every species discovered, every required recipe
discovered and crafted at least once. When the last condition flips:
completion animation → camera tour of the empty, blooming-in-memory world →
journey summary (flowers, bouquets, coins earned, playtime, tools, regions,
full collection). Then free-roam mode with no new requirements.

## 13. Assumptions (documented per spec)

* "Crafting time if applicable": recipes have a short craft time (0.5–3 s per
  batch) shown as a workshop progress bar; the player still starts every craft.
* The Garden Scissors are both a harvesting and crafting tool (both specs list
  them in both roles).
* "Every required tool" = all nine tools; "complete flower collection" = all
  17 species discovered.
* Region unlocks never cost coins (anti-deadlock); they cost progress.
