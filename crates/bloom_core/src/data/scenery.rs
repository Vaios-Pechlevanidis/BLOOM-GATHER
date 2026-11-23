//! How a region looks and feels: its light, the gate you enter it through
//! and its special features (flower rows, a stream, a pond, lanterns,
//! mushrooms, glowing plants, light shafts, falling petals, fireflies).
//!
//! Every field defaults to "nothing special" so a region only lists what
//! makes it different. Features that are solid or hold water (streams,
//! ponds, lanterns) are placed by `world::layout` and respected by flower
//! placement; the rest is decoration placed by the renderer.

use serde::{Deserialize, Serialize};

use super::Rgb;

/// The gate in the fence between a region and the one before it. The gate
/// belongs to the region it leads *into*.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum EntranceKind {
    /// A wooden five-bar farm gate between two posts.
    #[default]
    FarmGate,
    /// A rounded stone arch with blossom on top.
    StoneArch,
    /// Two leaning trees whose branches meet overhead.
    ForestArch,
    /// A wrought-iron arch hung with glowing lanterns.
    LanternArch,
}

/// The background sound loop of a region.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
pub enum Ambience {
    /// Breeze and birdsong.
    #[default]
    Meadow,
    /// Birdsong over a babbling stream.
    Valley,
    /// Deep woodland: soft wind in tall trees, distant birds.
    Forest,
    /// Crickets and a hushed night breeze.
    Night,
}

/// Light and air.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Atmosphere {
    /// Colour of the sun (or moon) light.
    pub sun_color: Rgb,
    /// Lux; daylight regions use a few thousand, night a few hundred.
    pub sun_illuminance: f32,
    /// Height of the sun above the horizon in degrees.
    pub sun_elevation_deg: f32,
    pub ambient_color: Rgb,
    pub ambient_brightness: f32,
    /// Colour at the top of the sky; `None` derives a soft blue from the
    /// horizon (`palette.sky`).
    pub zenith: Option<Rgb>,
    /// Fog starts and ends at these fractions of the region's longest side.
    pub fog_start: f32,
    pub fog_end: f32,
    /// Night: moon and stars in the sky, glowing flowers shine brighter.
    pub night: bool,
}

impl Default for Atmosphere {
    fn default() -> Self {
        Self {
            sun_color: Rgb(255, 242, 214),
            sun_illuminance: 3_600.0,
            sun_elevation_deg: 46.0,
            ambient_color: Rgb(219, 230, 255),
            ambient_brightness: 420.0,
            zenith: None,
            fog_start: 0.3,
            fog_end: 1.05,
            night: false,
        }
    }
}

/// Flowers planted in long parallel rows (lavender fields).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FlowerRows {
    /// Distance between rows in metres.
    pub spacing: f32,
    /// Direction the rows run, in degrees from the X axis.
    pub angle_deg: f32,
}

/// A stream winding across the region from west to east, with a bridge
/// wherever the footpath crosses it. Shallow enough to wade through.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct StreamDef {
    /// Water width in metres.
    pub width: f32,
    /// Depth of the water at the middle, in metres.
    pub depth: f32,
}

/// A round pond away from the path. Shallow enough to wade through.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct PondDef {
    pub radius: f32,
    pub depth: f32,
}

/// A bulb farm: flowers planted in long, straight beds of one colour each,
/// separated by bare furrows, on both sides of a straight farm track (like
/// the tulip and hyacinth fields of Holland). The south end is a farmyard
/// with the spawn, the workshop and the well; beds fill the rest.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct BulbFields {
    /// Planted width of a bed in metres.
    pub bed_width: f32,
    /// Bare soil between two beds in metres.
    pub furrow_width: f32,
    /// Rows of flowers along each bed.
    pub lines: u32,
    /// Depth of the farmyard at the south end (no beds there).
    pub yard_depth: f32,
    /// Grassy margin inside the fence around the beds.
    pub verge: f32,
    /// Width of the grass strip that crosses the beds halfway.
    pub headland: f32,
    /// Species that grow wild on the verges and in the yard instead.
    pub verge_species: Vec<String>,
    /// Species that turn up as single strays among the beds.
    pub stray_species: Vec<String>,
}

impl Default for BulbFields {
    fn default() -> Self {
        Self {
            bed_width: 1.4,
            furrow_width: 0.8,
            lines: 2,
            yard_depth: 24.0,
            verge: 3.0,
            headland: 3.0,
            verge_species: Vec::new(),
            stray_species: Vec::new(),
        }
    }
}

/// The sun rises and sets: a full day and night every `day_length_secs`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct DayCycle {
    /// Real seconds for one whole day (24 hours).
    pub day_length_secs: f32,
    /// Hour of the day (0-24) a session starts at.
    pub start_hour: f32,
}

impl Default for DayCycle {
    fn default() -> Self {
        Self {
            day_length_secs: 960.0,
            start_hour: 8.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RegionFeatures {
    pub rows: Option<FlowerRows>,
    /// Straight bulb-field beds (see [`BulbFields`]).
    pub fields: Option<BulbFields>,
    pub stream: Option<StreamDef>,
    pub pond: Option<PondDef>,
    /// Lantern posts along the footpath (solid; lit at night).
    pub lanterns: u32,
    /// Decorative mushroom clusters.
    pub mushrooms: u32,
    /// Decorative softly glowing plants (ferns and fungi, never flowers).
    pub glow_plants: u32,
    /// Shafts of light falling through the canopy.
    pub light_shafts: u32,
    /// Blossom petals drifting down from the trees.
    pub petals: bool,
    pub fireflies: u32,
    /// Share of trees in blossom (canopies in the palette's accent colour).
    pub blossom_trees: f32,
    /// Tree height range in metres; `None` uses the standard 4.5–7.5 m.
    pub tree_height: Option<(f32, f32)>,
    /// Decorative grass density relative to the meadow (1.0).
    pub grass_density: Option<f32>,
}

/// Look and feel of a region (all optional in the content file).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenery {
    pub entrance: EntranceKind,
    pub ambience: Ambience,
    pub atmosphere: Atmosphere,
    pub features: RegionFeatures,
    /// Day and night follow each other; `atmosphere` is then the light at
    /// noon (and `night` is ignored).
    pub day_cycle: Option<DayCycle>,
}

impl Scenery {
    pub fn grass_density(&self) -> f32 {
        self.features.grass_density.unwrap_or(1.0)
    }
}
