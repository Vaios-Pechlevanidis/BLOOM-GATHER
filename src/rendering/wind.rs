//! The wind material: a standard PBR material whose vertex shader sways
//! vertices according to their vertex-colour alpha (see `wind.wgsl`).

use bevy::asset::embedded_asset;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::mesh::VertexAttributeValues;
use bevy::render::render_resource::{AsBindGroup, ShaderRef};

pub type WindMaterial = ExtendedMaterial<StandardMaterial, Wind>;

pub struct WindPlugin;

impl Plugin for WindPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "wind.wgsl");
        app.add_plugins(MaterialPlugin::<WindMaterial>::default());
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Wind {
    /// x = amplitude (m), y = speed, z = flutter, w = unused.
    #[uniform(100)]
    pub params: Vec4,
}

impl Wind {
    pub fn new(amplitude: f32, speed: f32, flutter: f32) -> Self {
        Self {
            params: Vec4::new(amplitude, speed, flutter, 0.0),
        }
    }
}

impl MaterialExtension for Wind {
    fn vertex_shader() -> ShaderRef {
        "embedded://bloom_and_gather/rendering/wind.wgsl".into()
    }
}

/// A matte, vertex-coloured wind material.
pub fn wind_material(base: StandardMaterial, wind: Wind) -> WindMaterial {
    ExtendedMaterial {
        base,
        extension: wind,
    }
}

/// Write sway weights into vertex-colour alpha from model-space height:
/// 0 below `from`, rising to `max` at `to`.
pub fn sway_by_height(mesh: &mut Mesh, from: f32, to: f32, max: f32) {
    let heights: Vec<f32> = match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(p)) => p.iter().map(|v| v[1]).collect(),
        _ => return,
    };
    if let Some(VertexAttributeValues::Float32x4(colors)) =
        mesh.attribute_mut(Mesh::ATTRIBUTE_COLOR)
    {
        for (c, y) in colors.iter_mut().zip(heights) {
            let t = ((y - from) / (to - from)).clamp(0.0, 1.0);
            c[3] = t * t * (3.0 - 2.0 * t) * max;
        }
    }
}
