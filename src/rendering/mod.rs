//! World rendering: the 3D camera and the region scene.
//!
//! Rendering only *reads* game state. Placement and collision come from
//! `bloom_core`, so what you see and what you bump into always agree.

mod camera;
pub mod meshes;
mod scene;

pub use camera::MainCamera;
pub use scene::SceneSet;

use bevy::prelude::*;
use bloom_core::data::Rgb;

pub struct RenderingPlugin;

impl Plugin for RenderingPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((camera::CameraPlugin, scene::ScenePlugin));
    }
}

/// Convert a content colour into a Bevy colour.
pub fn color(c: Rgb) -> Color {
    Color::srgb_u8(c.0, c.1, c.2)
}
