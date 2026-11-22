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

| Slot | Tool | Mode | How it works (first person) |
|---|---|---|---|
| 1 | Hands | Single | look at a flower, E picks it; 0.35 s between picks |
| 1 | Garden Scissors | Single | same, 40 % faster (0.21 s); a snip animation |
| 2 | Harvesting Basket | Radius | aim at the ground; a ring shows a 1.6 m circle; E gathers up to 6 flowers, nearest first; 0.7 s pause |
| 3 | Professional Gardening Tools (garden rake) | Sweep | hold E and look across the flowers: every 0.25 s gathers up to 4 within 2.2 m |
| 4 | Advanced Harvesting Equipment (flower cart) | Area | hold E to set a corner, turn to drag a rectangle (up to 8 × 8 m), release to gather up to 40; 1.5 s pause |

Ground targets can be up to 4.5 m away (flower reach 3 m + 1.5 m). Every
flower a batch tool would take is highlighted before you commit. Keys 1–4 or
the mouse wheel switch tools; only owned tools appear on the tool bar.
Upgrades and skills will scale cooldowns, radius, caps and area size; they
never unlock a mode on their own. All numbers live in `game_data.ron`.

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

**At the workbench** (walk into the cottage and press E at the bench): the
recipe list shows every recipe; locked ones say what is still needed (tools,
species to find, level). The selected recipe shows its requirements, its
price next to what its flowers would sell for raw, and the exact flowers a
batch will use. The player chooses a quantity (-/+, Max, arrow keys) up to
the batch capacity and may mark species to *keep out* of the bouquet; the
rest is filled automatically from the most plentiful flowers. Binding takes
`craft_time_secs` per bouquet of play time (the job is saved); bouquets and
crafting XP arrive when it finishes. The Bouquets tab (or pressing E at the
display table) sells them. Finished bouquets stand in the display's vases.

**Reserve:** while a required recipe has never been crafted, the exact flowers
it needs (e.g. 2 Roses for Romantic) are *set aside*: they can't be sold or
used by other recipes, and the UI says so. The reserve is released after the
first craft. Combined with the world totals this guarantees every required
bouquet stays craftable.

## 7. Upgrades

Bought at the workshop's **tool shelf** (Upgrades tab). Levelled and
coin-priced: `cost(level) = base × 1.6^level`, then discounts. Every tile
shows what the next level changes as concrete numbers ("Single pick every:
0.21 s -> 0.20 s"). Effects add up per kind across upgrades and skills.

| Upgrade | Per level | Levels | Needs |
|---|---|---|---|
| Nimble Hands | single-pick cooldown −6 % | 5 | — |
| Wider Reach | basket and rake circle +10 % | 5 | Harvesting Basket |
| Deep Basket | flowers per basket/rake/cart action +20 % | 5 | Harvesting Basket |
| Wide Rakes | cart area +10 % | 3 | Advanced Harvesting Equipment |
| Quick Binding | binding time −8 % | 5 | Binding Ribbon |
| Bulk Orders | batch capacity +5 | 4 | Binding Ribbon |
| Garden Boots | walking and running speed +4 % | 4 | — |
| Market Stall | every bouquet's price +4 % | 5 | Binding Ribbon |
| Haggler | tool prices −3 % | 5 | — |
| Pretty Packaging | wrapped bouquets' price +6 % | 3 | Wrapping Paper Set |

No upgrade raises the price of individual flowers. Garden Boots replaces the
earlier "Careful Hands" idea (a random "exquisite" bouquet), which added a
hidden second bouquet stock for little play value; walking speed matters in
first person.

**Progression guard:** an upgrade can't be bought if it would leave too
little for the required tools still missing (economy doc §5.2). The shop
shows how much is free for extras, and a blocked upgrade says "Save for tools
first". Tools themselves are never blocked.

## 8. Skill tree

Open with **K**. Five branches of chained skills (each needs the one above
it); 21 skills, 44 skill points to learn everything. Skill points (SP):
1 per level from level 2, +1 per completed region, +1 at 8 and 16 species
discovered, +1 per fully discovered region with Collection Bonuses. A full
playthrough earns about 41, so the player picks a focus. Skills are
permanent. They never grant a tool, a harvesting mode or a recipe; some
require a tool because they improve what it does.

* **Harvesting:** Quick Fingers (pick cooldown −5 %/lvl, 3) → Double
  Harvest (8 %/lvl chance a single pick also takes the nearest unpicked
  neighbour within 1.6 m, 3) → Improved Radius (basket/rake circle +8 %/lvl,
  3; needs Harvesting Basket) → Master Picker (+25 % flowers per action, 2 SP;
  needs Pro Gardening Tools).
* **Botany:** Botanical Knowledge (+15 %/lvl discovery XP, 2) → Flower
  Recognition (flowers of undiscovered species have a golden glint above
  them) → Rare Flower Detection (the region card counts rare and very rare
  flowers left) → Collection Bonuses (+1 SP per fully discovered region,
  2 SP). *No Botany skill touches flower price.*
* **Floristry:** Basic Florist (+10 %/lvl crafting XP, 2) → Efficient
  Binding (binding time −10 %/lvl, 2) → Artistic Arrangements (+5 % bouquet
  price; needs Garden Scissors) → Premium Packaging (+8 % on wrapped
  bouquets; needs Wrapping Paper Set) → Master Florist (+10 batch capacity,
  2 SP; needs Florist Workbench).
* **Efficiency:** Better Deals (tool price −4 %/lvl, 2) → Reduced Upgrade
  Costs (upgrade price −4 %/lvl, 3) → Improved Profits (bouquet price
  +3 %/lvl, 3) → Efficient Production (5 %/lvl chance per bouquet that one
  of its flowers is saved and returns to the basket, 2 SP per level). Only
  flowers that craft used come back, so the world stays finite.
* **Exploration:** New Meadows (regions open after 10 % fewer harvested
  flowers) → Trailblazer (walk/run +5 %/lvl, 2) → Long Reach (+0.3 m/lvl
  pick and aim reach, 2) → Region Unlock Bonuses (+50 coins and +50 XP
  whenever a region opens, 2 SP).

Chance-based skills use a deterministic RNG whose state is saved, so
reloading never re-rolls a result. The 2D-era ideas "minimap markers" and
"camera pan/zoom" became Flower Recognition's glints and Long Reach, which
fit a first-person game.

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

## 11. Achievements and the journal

Nineteen optional achievements in five categories, each with a small reward
(coins, or a skill point for the long ones). They are never required for the
ending.

| Category | Achievements |
|---|---|
| Harvesting | First Flower (1), Armful (100), Field Hand (1,000; +1 SP), Master Harvester (all 1,900) |
| Crafting | First Bouquet (1), Busy Bench (10), Florist (100), Grand Florist (500; +1 SP) |
| Exploration | First Discovery, Meadow Cleared (1 region), Open Valley (all regions open), World Explorer (all complete; +1 SP) |
| Collection | Rare Find (a rare or very rare flower), Half the Herbarium (9 species), Complete Collection (+1 SP) |
| Completion | Fully Equipped (all tools), Recipe Book (all recipes), Every Bouquet (every required bouquet once), Bloom & Gather (the game complete) |

"Master Florist" from the first draft became **Grand Florist**, because a
Floristry skill already has that name. 500 bouquets is reachable (the world
could make up to 633 Simple Bouquets) but asks for a deliberate choice.

The **journal (J)** has three pages:

* **Achievements:** progress bars and rewards.
* **Statistics:** time played, distance walked, flowers picked, sold and
  rare ones, bouquets, coins earned and spent, tools, upgrade and skill
  levels, regions, achievements.
* **Progress:** the six completion requirements as a checklist, and what's
  next (next level, the next region's requirements, the next tool, the next
  collection milestone, the closest achievement).

**Level-ups** announce what the level brings: a skill point, tools now
sold at the shelf, and regions whose level is reached but which still wait
for harvested flowers.

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
