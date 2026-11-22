//! The wind material: a standard PBR material whose vertex shader sways
//! vertices according to their vertex-colour alpha (see `wind.wesl`).

use bevy::asset::embedded_asset;
use bevy::mesh::VertexAttributeValues;
use bevy::pbr::{ExtendedMaterial, MaterialExtension};
use bevy::prelude::*;
use bevy::render::render_resource::AsBindGroup;
use bevy::shader::ShaderRef;

pub type WindMaterial = ExtendedMaterial<StandardMaterial, Wind>;

pub struct WindPlugin;

impl Plugin for WindPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "wind.wesl");
        app.add_plugins(MaterialPlugin::<WindMaterial>::default())
            .insert_resource(Weather { wind: 1.0 })
            .add_systems(Update, breeze);
    }
}

/// Gentle weather: the breeze slowly rises and falls over minutes, between
/// nearly still and a fresh wind. Grass, canopies, flowers, petals and
/// clouds all follow it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Weather {
    /// 1 = the usual breeze.
    pub wind: f32,
}

impl Weather {
    /// Breeze strength at `t` seconds: a smooth mix of slow waves.
    pub fn wind_at(t: f32) -> f32 {
        let slow =
            (t / 97.0).sin() * 0.6 + (t / 41.0 + 1.3).sin() * 0.3 + (t / 17.0 + 0.4).sin() * 0.1;
        0.9 + 0.4 * slow
    }
}

fn breeze(
    time: Res<Time>,
    mut weather: ResMut<Weather>,
    mut materials: ResMut<Assets<WindMaterial>>,
) {
    let wind = Weather::wind_at(time.elapsed_secs());
    if (wind - weather.wind).abs() < 0.01 {
        return;
    }
    weather.wind = wind;
    for (_, m) in materials.iter_mut() {
        m.extension.params.w = wind;
    }
}

#[derive(Asset, AsBindGroup, Reflect, Debug, Clone)]
pub struct Wind {
    /// x = amplitude (m), y = speed, z = flutter, w = breeze strength
    /// (weather; 0 = unset).
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
        "embedded://bloom_and_gather/rendering/wind.wesl".into()
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

#[cfg(test)]
mod tests {
    use super::Weather;

    #[test]
    fn the_breeze_stays_gentle_and_changes_slowly() {
        let mut prev = Weather::wind_at(0.0);
        let (mut lo, mut hi) = (f32::MAX, f32::MIN);
        for i in 1..20_000 {
            let w = Weather::wind_at(i as f32 * 0.1);
            assert!((w - prev).abs() < 0.01, "no sudden gusts");
            lo = lo.min(w);
            hi = hi.max(w);
            prev = w;
        }
        assert!(lo > 0.45 && hi < 1.35, "{lo}..{hi}");
        assert!(hi - lo > 0.5, "it does change");
    }
}
