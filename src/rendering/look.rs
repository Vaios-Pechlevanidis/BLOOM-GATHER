//! The final look of the picture: tonemapping and colour grading.
//!
//! Two 3D cameras draw into the same HDR picture: the world camera, then the
//! first-person hands camera on top. Each camera's tonemapping pass works on
//! the *whole* shared picture, so the world camera leaves it linear
//! (`Tonemapping::None`, bloom still applies) and only the hands camera, the
//! last one, tonemaps and grades. Everything is tonemapped exactly once.
//!
//! Khronos PBR Neutral keeps the flowers' authored colours and the sky blue
//! (TonyMcMapface washed both out). The grade follows the time of day
//! ([`Daylight`]): crisp at noon, warm with opened-up shadows at sunset,
//! slightly cool at night.

use bevy::core_pipeline::tonemapping::DebandDither;
use bevy::prelude::*;
use bevy::render::view::{ColorGrading, ColorGradingGlobal, ColorGradingSection, Tonemapping};

use super::camera::MainCamera;
use super::Daylight;

pub struct LookPlugin;

impl Plugin for LookPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (setup_cameras, grade).chain());
    }
}

/// The curve the final camera applies to the whole picture.
pub const TONEMAPPING: Tonemapping = Tonemapping::KhronosPbrNeutral;

/// Marks the camera that tonemaps and grades the finished picture.
#[derive(Component)]
pub struct FinalCamera;

fn setup_cameras(
    mut commands: Commands,
    cameras: Query<(Entity, Has<MainCamera>), Added<Camera3d>>,
) {
    for (e, main) in &cameras {
        if main {
            commands
                .entity(e)
                .insert((Tonemapping::None, DebandDither::Disabled));
        } else {
            commands.entity(e).insert((
                TONEMAPPING,
                DebandDither::Enabled,
                FinalCamera,
                ColorGrading::default(),
            ));
        }
    }
}

fn grade(daylight: Res<Daylight>, mut cameras: Query<&mut ColorGrading, With<FinalCamera>>) {
    let want = grading(&daylight);
    for mut g in &mut cameras {
        *g = want.clone();
    }
}

/// The grade for the current light.
pub fn grading(d: &Daylight) -> ColorGrading {
    // How much of each mood: golden hour/sunset, and night.
    let dusk = d.dusk * (1.0 - d.night);
    let night = d.night;
    let day = (1.0 - dusk - night).max(0.0);

    let global = ColorGradingGlobal {
        exposure: 0.12 * dusk + 0.1 * night,
        // Warmer at sunset, a touch bluer under the moon.
        temperature: 0.025 * dusk - 0.015 * night,
        // A little more colour by day; sunset skies are already intense.
        post_saturation: 1.0 + 0.02 * day - 0.06 * dusk,
        ..default()
    };
    let shadows = ColorGradingSection {
        // Open up the backlit foreground at sunset and dusk.
        lift: 0.03 * dusk + 0.01 * night,
        saturation: 1.0 + 0.04 * day,
        ..default()
    };
    let midtones = ColorGradingSection {
        contrast: 1.0 + 0.05 * day,
        ..default()
    };
    ColorGrading {
        global,
        shadows,
        midtones,
        highlights: ColorGradingSection::default(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noon_is_neutral_sunset_warm_night_cool() {
        let noon = grading(&Daylight::default());
        assert_eq!(noon.global.temperature, 0.0);
        assert!(noon.global.post_saturation > 1.0);

        let mut sunset = Daylight::default();
        sunset.dusk = 1.0;
        let sunset = grading(&sunset);
        assert!(sunset.global.temperature > 0.0);
        assert!(sunset.shadows.lift > 0.0);

        let mut night = Daylight::default();
        night.night = 1.0;
        night.dusk = 1.0;
        let night = grading(&night);
        assert!(night.global.temperature < 0.0);
    }
}
