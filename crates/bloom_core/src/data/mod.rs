//! Data-driven content: flower species, tools, bouquet recipes and regions,
//! plus the global economy/progression/harvesting/world configuration.
//!
//! Content is authored in `assets/data/game_data.ron`, deserialized into the
//! types below, indexed for fast lookup and validated by
//! [`GameData::validate`] (see `validation.rs`).

mod achievements;
mod growth;
mod scenery;
mod validation;

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::economy::{Coins, EconomyConfig};
use crate::progression::ProgressionConfig;

pub use achievements::{AchievementCategory, AchievementDef, AchievementReward, Condition};
pub use growth::{
    Effect, SkillBranch, SkillDef, UpgradeCategory, UpgradeDef, MIN_PRICE_FACTOR, MIN_TIME_FACTOR,
};
pub use scenery::{
    Ambience, Atmosphere, EntranceKind, FlowerRows, PondDef, RegionFeatures, Scenery, StreamDef,
};
pub use validation::ValidationError;

/// The content file compiled into the game binary.
pub const BUILTIN_GAME_DATA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../assets/data/game_data.ron"
));

/// sRGB colour, 0–255 per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rgb(pub u8, pub u8, pub u8);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rarity {
    Common,
    Uncommon,
    Rare,
    VeryRare,
}

impl Rarity {
    pub const ALL: [Rarity; 4] = [
        Rarity::Common,
        Rarity::Uncommon,
        Rarity::Rare,
        Rarity::VeryRare,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Rarity::Common => "Common",
            Rarity::Uncommon => "Uncommon",
            Rarity::Rare => "Rare",
            Rarity::VeryRare => "Very Rare",
        }
    }
}

/// A flower species. Note the absence of a price: every flower sells for
/// [`EconomyConfig::flower_base_value`], so rarity can never affect price.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FlowerSpecies {
    pub id: String,
    pub name: String,
    pub description: String,
    pub rarity: Rarity,
    /// 3D silhouette family.
    pub shape: FlowerShape,
    /// Height of the bloom above the ground in metres (model size and
    /// look-at picking).
    pub height: f32,
    pub petal_color: Rgb,
    pub center_color: Rgb,
    /// Emissive strength 0–1 for flowers that glow at night.
    #[serde(default)]
    pub glow: f32,
    /// Optional painted glTF model replacing the procedural one.
    #[serde(default)]
    pub model: Option<String>,
}

/// The 3D silhouette families flowers are built from. Several species may
/// share a family (with different colours and sizes); each family looks
/// distinctly different.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FlowerShape {
    /// Flat ray of narrow petals around a domed centre.
    Daisy,
    /// Six petals behind a forward-facing trumpet.
    Trumpet,
    /// Four broad cupped petals around a dark centre.
    Cup,
    /// Arching stem hung with small bells.
    Bells,
    /// Low round pompom of tiny florets over trefoil leaves.
    Pompom,
    /// Several upright spikes of stacked buds.
    Spike,
    /// Closed upright cup of six petals.
    Tulip,
    /// Tall stem with a large sideways-facing disc.
    Sunflower,
    /// Tight layered spiral of petals.
    Rose,
    /// Big ruffled ball of petals.
    Ruffle,
    /// Round cluster of many four-petal florets.
    Mophead,
    /// Tall spike with tubular bells down one side.
    Foxglove,
    /// Arching stem with asymmetric blooms.
    Orchid,
    /// Six recurved petals forming a star-shaped trumpet.
    Lily,
    /// Small shrub dotted with starry flowers.
    StarCluster,
    /// Wide upward-facing pale trumpet.
    Moonbloom,
    /// Five-pointed star bloom.
    Star,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HarvestMode {
    /// One flower per click (always available).
    Single,
    /// Click to gather every flower within a radius.
    Radius,
    /// Hold and drag to keep gathering around the cursor.
    Sweep,
    /// Drag out a rectangle and gather everything inside it.
    Area,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ToolCategory {
    Harvesting,
    Crafting,
    Both,
}

/// What owning a tool *does*. Recipes additionally reference tools through
/// [`RecipeDef::required_tools`]; that is how crafting tools unlock recipes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToolEffect {
    /// Multiplies the single-pick cooldown (e.g. 0.6 = 40 % faster).
    HarvestCooldownMultiplier(f32),
    /// Unlocks [`HarvestMode::Radius`].
    RadiusMode { radius: f32, max_flowers: u32 },
    /// Unlocks [`HarvestMode::Sweep`].
    SweepMode {
        radius: f32,
        max_flowers_per_tick: u32,
    },
    /// Unlocks [`HarvestMode::Area`].
    AreaMode {
        max_width: f32,
        max_height: f32,
        max_flowers: u32,
    },
    /// Raises the workshop batch capacity to at least this value.
    CraftBatchCapacity(u32),
}

impl ToolEffect {
    /// The harvesting mode this effect unlocks, if any.
    pub fn unlocked_mode(&self) -> Option<HarvestMode> {
        match self {
            ToolEffect::RadiusMode { .. } => Some(HarvestMode::Radius),
            ToolEffect::SweepMode { .. } => Some(HarvestMode::Sweep),
            ToolEffect::AreaMode { .. } => Some(HarvestMode::Area),
            ToolEffect::HarvestCooldownMultiplier(_) | ToolEffect::CraftBatchCapacity(_) => None,
        }
    }
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolDef {
    pub id: String,
    pub name: String,
    pub description: String,
    pub category: ToolCategory,
    pub price: Coins,
    /// Minimum player level to buy the tool.
    pub unlock_level: u32,
    /// Tools that must already be owned.
    #[serde(default)]
    pub requires_tools: Vec<String>,
    pub effects: Vec<ToolEffect>,
    #[serde(default = "default_true")]
    pub required_for_completion: bool,
    pub icon_color: Rgb,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpeciesCount {
    pub species: String,
    pub count: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RecipeDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// 1-based difficulty tier; scales crafting XP.
    pub tier: u8,
    /// Total flowers consumed by one bouquet.
    pub flower_count: u32,
    /// Minimum number of different species among those flowers.
    #[serde(default)]
    pub min_distinct_species: u32,
    /// Specific species that must be included (counted towards `flower_count`).
    #[serde(default)]
    pub required_species: Vec<SpeciesCount>,
    pub required_tools: Vec<String>,
    /// Extra level gate on top of owning the tools (usually 0 = none).
    #[serde(default)]
    pub unlock_level: u32,
    pub craft_time_secs: f32,
    pub sell_price: Coins,
    #[serde(default = "default_true")]
    pub required_for_completion: bool,
    pub ribbon_color: Rgb,
}

impl RecipeDef {
    /// Flowers needed beyond the named species ("any species" slots).
    pub fn free_slots(&self) -> u32 {
        let named: u32 = self.required_species.iter().map(|r| r.count).sum();
        self.flower_count.saturating_sub(named)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionUnlock {
    pub level: u32,
    /// Lifetime flowers harvested across all regions.
    pub flowers_harvested: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionReward {
    pub coins: Coins,
    pub skill_points: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionPalette {
    pub sky: Rgb,
    pub ground: Rgb,
    pub ground_dark: Rgb,
    pub ground_light: Rgb,
    pub path: Rgb,
    pub accent: Rgb,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegionDef {
    pub id: String,
    pub name: String,
    pub description: String,
    /// Declared flower total; validation checks it equals the sum of `flowers`.
    pub flower_total: u32,
    /// Ground size in metres: `width` along X, `depth` along Z. The region is
    /// centred on the origin.
    pub width: f32,
    pub depth: f32,
    /// Seed for deterministic layout. Never change it after release: saves
    /// identify flowers by their generated index.
    pub seed: u64,
    pub flowers: Vec<SpeciesCount>,
    pub unlock: RegionUnlock,
    pub completion_reward: RegionReward,
    pub palette: RegionPalette,
    pub terrain: TerrainParams,
    pub props: RegionProps,
    /// The region has the florist's workshop (crafting, tool shop, storage).
    #[serde(default)]
    pub workshop: bool,
    /// Light, entrance gate, sound and special features.
    #[serde(default)]
    pub scenery: Scenery,
}

/// Shape of a region's rolling terrain (see `world::terrain`).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TerrainParams {
    /// Peak-to-centre height of the hills in metres.
    pub amplitude: f32,
    /// Typical hill width in metres (the noise wavelength).
    pub feature_size: f32,
    /// Noise octaves; more = more small bumps.
    pub octaves: u32,
    /// Extra height of the gentle berm along the region edge.
    pub border_rise: f32,
    /// How far in from the edge the berm reaches.
    pub border_width: f32,
}

/// Decorative but *solid* props. These never count as harvestable flowers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct RegionProps {
    pub trees: u32,
    pub rocks: u32,
}

impl RegionDef {
    pub fn counted_flowers(&self) -> u32 {
        self.flowers.iter().map(|f| f.count).sum()
    }

    pub fn species_count(&self, species: &str) -> u32 {
        self.flowers
            .iter()
            .filter(|f| f.species == species)
            .map(|f| f.count)
            .sum()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HarvestingConfig {
    /// Cooldown between single picks with bare hands.
    pub hand_cooldown_secs: f32,
    /// Pause after a basket (radius) gather.
    pub radius_cooldown_secs: f32,
    /// Time between gathers while sweeping.
    pub sweep_interval_secs: f32,
    /// Pause after an area gather.
    pub area_cooldown_secs: f32,
    /// Ground targets may be this much further away than flower reach.
    pub ground_reach_bonus: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorldConfig {
    /// Side length of a square world chunk in metres.
    pub chunk_size: f32,
    /// Minimum distance between two generated flowers, in metres.
    pub min_flower_spacing: f32,
    /// Terrain grid resolution in metres between height samples.
    pub terrain_cell: f32,
    /// Display scale applied to every flower (botanical sizes are tiny from
    /// eye height). Used by both rendering and look-at picking.
    pub flower_scale: f32,
    /// Walkable margin kept between the region edge and the invisible wall.
    pub boundary_margin: f32,
}

/// First-person movement tuning. All distances in metres, times in seconds.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlayerConfig {
    /// Collision radius of the player's body.
    pub radius: f32,
    /// Body height (feet to head).
    pub height: f32,
    /// Camera height above the feet.
    pub eye_height: f32,
    pub walk_speed: f32,
    pub sprint_speed: f32,
    /// Horizontal acceleration on the ground while there is input (m/s²).
    pub ground_accel: f32,
    /// Horizontal deceleration on the ground without input (m/s²).
    pub ground_decel: f32,
    /// Horizontal acceleration while airborne (m/s²).
    pub air_accel: f32,
    /// Downward acceleration (m/s²). Higher than 9.81 feels less floaty.
    pub gravity: f32,
    /// Apex height of a standing jump.
    pub jump_height: f32,
    /// Steepest terrain the player can walk up, in degrees.
    pub max_slope_deg: f32,
    /// Obstacles up to this height are stepped onto instead of blocking.
    pub step_height: f32,
    /// While grounded, stay glued to ground up to this far below the feet.
    pub ground_snap: f32,
    /// Grace period after walking off an edge in which jumping still works.
    pub coyote_secs: f32,
    /// A jump pressed this long before landing still triggers.
    pub jump_buffer_secs: f32,
    /// Maximum distance from the eye at which things can be interacted with.
    pub interact_range: f32,
}

impl PlayerConfig {
    /// Initial upward speed that reaches `jump_height`: v = √(2gh).
    pub fn jump_speed(&self) -> f32 {
        (2.0 * self.gravity * self.jump_height).sqrt()
    }
}

/// All game content and balancing, validated and indexed.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GameData {
    pub version: u32,
    pub economy: EconomyConfig,
    pub progression: ProgressionConfig,
    pub harvesting: HarvestingConfig,
    pub world: WorldConfig,
    pub player: PlayerConfig,
    pub species: Vec<FlowerSpecies>,
    pub tools: Vec<ToolDef>,
    pub recipes: Vec<RecipeDef>,
    pub regions: Vec<RegionDef>,
    #[serde(default)]
    pub upgrades: Vec<UpgradeDef>,
    #[serde(default)]
    pub skills: Vec<SkillDef>,
    #[serde(default)]
    pub achievements: Vec<AchievementDef>,
    #[serde(skip)]
    index: Index,
}

#[derive(Debug, Clone, Default)]
struct Index {
    species: HashMap<String, usize>,
    tools: HashMap<String, usize>,
    recipes: HashMap<String, usize>,
    regions: HashMap<String, usize>,
    upgrades: HashMap<String, usize>,
    skills: HashMap<String, usize>,
    achievements: HashMap<String, usize>,
}

#[derive(Debug)]
pub enum DataError {
    Parse(String),
    Invalid(Vec<ValidationError>),
}

impl fmt::Display for DataError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DataError::Parse(msg) => write!(f, "game data could not be parsed: {msg}"),
            DataError::Invalid(errors) => {
                writeln!(
                    f,
                    "game data failed validation ({} problem(s)):",
                    errors.len()
                )?;
                for e in errors {
                    writeln!(f, "  - {e}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for DataError {}

impl GameData {
    /// Parse, index and validate content from RON text.
    pub fn from_ron_str(text: &str) -> Result<Self, DataError> {
        let data: GameData = ron::from_str(text).map_err(|e| DataError::Parse(e.to_string()))?;
        data.reindexed_and_validated()
    }

    /// The content shipped with the game.
    pub fn builtin() -> Result<Self, DataError> {
        Self::from_ron_str(BUILTIN_GAME_DATA)
    }

    /// Rebuild lookup tables and validate. Call after editing content in code
    /// (tests, tools); [`GameData::from_ron_str`] does this automatically.
    pub fn reindexed_and_validated(mut self) -> Result<Self, DataError> {
        self.rebuild_index();
        self.validate().map_err(DataError::Invalid)?;
        Ok(self)
    }

    fn rebuild_index(&mut self) {
        fn build<T>(items: &[T], id: impl Fn(&T) -> &str) -> HashMap<String, usize> {
            items
                .iter()
                .enumerate()
                .map(|(i, t)| (id(t).to_owned(), i))
                .collect()
        }
        self.index = Index {
            species: build(&self.species, |s| &s.id),
            tools: build(&self.tools, |t| &t.id),
            recipes: build(&self.recipes, |r| &r.id),
            regions: build(&self.regions, |r| &r.id),
            upgrades: build(&self.upgrades, |u| &u.id),
            skills: build(&self.skills, |s| &s.id),
            achievements: build(&self.achievements, |a| &a.id),
        };
    }

    pub fn species(&self, id: &str) -> Option<&FlowerSpecies> {
        self.index.species.get(id).map(|&i| &self.species[i])
    }

    pub fn species_index(&self, id: &str) -> Option<usize> {
        self.index.species.get(id).copied()
    }

    pub fn tool(&self, id: &str) -> Option<&ToolDef> {
        self.index.tools.get(id).map(|&i| &self.tools[i])
    }

    pub fn recipe(&self, id: &str) -> Option<&RecipeDef> {
        self.index.recipes.get(id).map(|&i| &self.recipes[i])
    }

    pub fn recipe_index(&self, id: &str) -> Option<usize> {
        self.index.recipes.get(id).copied()
    }

    pub fn tool_index(&self, id: &str) -> Option<usize> {
        self.index.tools.get(id).copied()
    }

    pub fn upgrade(&self, id: &str) -> Option<&UpgradeDef> {
        self.index.upgrades.get(id).map(|&i| &self.upgrades[i])
    }

    pub fn upgrade_index(&self, id: &str) -> Option<usize> {
        self.index.upgrades.get(id).copied()
    }

    pub fn skill(&self, id: &str) -> Option<&SkillDef> {
        self.index.skills.get(id).map(|&i| &self.skills[i])
    }

    pub fn skill_index(&self, id: &str) -> Option<usize> {
        self.index.skills.get(id).copied()
    }

    pub fn achievement(&self, id: &str) -> Option<&AchievementDef> {
        self.index
            .achievements
            .get(id)
            .map(|&i| &self.achievements[i])
    }

    pub fn achievement_index(&self, id: &str) -> Option<usize> {
        self.index.achievements.get(id).copied()
    }

    pub fn region_index(&self, id: &str) -> Option<usize> {
        self.index.regions.get(id).copied()
    }

    pub fn region(&self, id: &str) -> Option<&RegionDef> {
        self.index.regions.get(id).map(|&i| &self.regions[i])
    }

    /// The selling value of any single flower.
    pub fn flower_value(&self) -> Coins {
        self.economy.flower_base_value
    }

    /// Total flowers in the whole finite world.
    pub fn world_flower_total(&self) -> u64 {
        self.regions
            .iter()
            .map(|r| r.counted_flowers() as u64)
            .sum()
    }

    /// How many flowers of a species exist across all regions.
    pub fn species_world_total(&self, species: &str) -> u64 {
        self.regions
            .iter()
            .map(|r| r.species_count(species) as u64)
            .sum()
    }

    /// Income guaranteed even if the player never crafts: every flower sold
    /// raw plus every region completion reward.
    pub fn guaranteed_income(&self) -> Coins {
        let raw = self.economy.raw_flower_value(self.world_flower_total());
        let rewards: Coins = self.regions.iter().map(|r| r.completion_reward.coins).sum();
        raw.saturating_add(rewards)
    }

    /// Sum of the base prices of every tool required for completion.
    pub fn required_tool_cost(&self) -> Coins {
        self.tools
            .iter()
            .filter(|t| t.required_for_completion)
            .map(|t| t.price)
            .sum()
    }
}
