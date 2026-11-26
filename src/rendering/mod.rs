//! World rendering: the 3D camera, the region scene (terrain, props, lawn,
//! grass, landmarks), sky and clouds, and the wind material.
//!
//! Rendering only *reads* game state. Placement and collision come from
//! `bloom_core`, so what you see and what you bump into always agree.

mod beds;
mod bouquets;
mod camera;
mod critters;
pub mod daylight;
mod grass;
mod landmarks;
mod lawn;
pub mod meshes;
mod scene;
mod scenery;
mod sky;
mod wind;

pub use camera::MainCamera;
pub use daylight::Daylight;
pub use scene::SceneSet;
pub use wind::{sway_by_height, wind_material, Weather, Wind, WindMaterial};

use bevy::prelude::*;
use bloom_core::data::Rgb;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            wind::WindPlugin,
            camera::CameraPlugin,
            scene::ScenePlugin,
            grass::GrassPlugin,
            lawn::LawnPlugin,
            daylight::DaylightPlugin,
            sky::SkyPlugin,
            landmarks::LandmarksPlugin,
            critters::CrittersPlugin,
            bouquets::BouquetsPlugin,
            beds::BedsPlugin,
            scenery::SceneryPlugin,
        ));
    }
}

/// Convert a content colour into a Bevy colour.
pub fn color(c: Rgb) -> Color {
    Color::srgb_u8(c.0, c.1, c.2)
}
