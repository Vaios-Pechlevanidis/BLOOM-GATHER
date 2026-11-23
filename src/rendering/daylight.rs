//! Time of day: the sun rises in the east, crosses the south and sets in the
//! north-west; then the moon and the stars take over until dawn.
//!
//! [`Daylight`] is worked out once per frame from the clock and the region's
//! noon light (`scenery.atmosphere`) and everything else reads it: the
//! sun/moon light, ambient light, fog and clear colour (here), the sky dome,
//! sun, moon, stars and clouds (`sky`), glowing flowers, lanterns,
//! fireflies, butterflies and the music. Regions without a day cycle keep
//! their fixed light (day, or night when `atmosphere.night`).

use std::f32::consts::TAU;

use bevy::pbr::DistanceFog;
use bevy::prelude::*;
use bloom_core::data::{Atmosphere, RegionDef};

use super::camera::MainCamera;
use super::color;
use super::meshes::{lin, mix};
use super::scene::SceneEntity;
use crate::game::{Content, RegionScene};

pub struct DaylightPlugin;

impl Plugin for DaylightPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Daylight>().add_systems(
            Update,
            (start_clock, advance_clock, apply_light)
                .chain()
                .run_if(resource_exists::<RegionScene>),
        );
    }
}

/// The lowest the sun sinks (degrees, at midnight).
const MIDNIGHT_ELEVATION: f32 = -24.0;
/// Light from the moon at its brightest (lux).
const MOON_LUX: f32 = 1_000.0;

#[derive(Resource, Debug, Clone)]
pub struct Daylight {
    /// Whether the region has a running day cycle.
    pub cycle: bool,
    /// Hour of the day, 0..24.
    pub hour: f32,
    /// Real seconds per in-game hour.
    pub secs_per_hour: f32,
    /// Height of the sun above the horizon (degrees; negative at night).
    pub elevation_deg: f32,
    /// Unit vectors from the ground towards the sun and the moon.
    pub sun_dir: Vec3,
    pub moon_dir: Vec3,
    /// 0 in daylight, 1 in full night.
    pub night: f32,
    /// Strength of sunrise/sunset colours, 0..1.
    pub dusk: f32,
    /// The directional light: where it comes from, its colour and lux.
    pub light_dir: Vec3,
    pub light_color: LinearRgba,
    pub light_lux: f32,
    pub ambient: LinearRgba,
    pub ambient_brightness: f32,
    /// Sky colours at the horizon and overhead.
    pub horizon: LinearRgba,
    pub zenith: LinearRgba,
    /// Colour of the sun disc and its glow in the sky.
    pub sun_color: LinearRgba,
    /// The region (and session generation) the clock was started for.
    started_for: Option<usize>,
}

impl Default for Daylight {
    fn default() -> Self {
        Self {
            cycle: false,
            hour: 12.0,
            secs_per_hour: 40.0,
            elevation_deg: 50.0,
            sun_dir: Vec3::Y,
            moon_dir: -Vec3::Y,
            night: 0.0,
            dusk: 0.0,
            light_dir: Vec3::Y,
            light_color: LinearRgba::WHITE,
            light_lux: 3_600.0,
            ambient: LinearRgba::WHITE,
            ambient_brightness: 400.0,
            horizon: LinearRgba::WHITE,
            zenith: LinearRgba::BLUE,
            sun_color: LinearRgba::WHITE,
            started_for: None,
        }
    }
}

impl Daylight {
    /// "07:30" style clock for the HUD.
    pub fn clock(&self) -> String {
        let minutes = (self.hour * 60.0) as u32 % (24 * 60);
        format!("{:02}:{:02}", minutes / 60, minutes % 60)
    }

    /// Glow of night flowers and lanterns: a hint by day, full at night.
    pub fn glow(&self) -> f32 {
        0.15 + 0.85 * self.night
    }

    /// Work everything out for `hour` in `region`.
    fn update(&mut self, region: &RegionDef) {
        let air = region.scenery.atmosphere;
        if !self.cycle {
            self.fixed(region, &air);
            return;
        }
        // The sun's path: noon at 13:00 (summer time) due south, rising in
        // the north-east and setting in the north-west.
        let theta = (self.hour - 13.0) / 24.0 * TAU;
        let noon = air.sun_elevation_deg;
        let e = MIDNIGHT_ELEVATION + (noon - MIDNIGHT_ELEVATION) * (theta.cos() + 1.0) / 2.0;
        let across = Vec2::new(-theta.sin(), theta.cos()).normalize();
        let er = e.to_radians();
        self.elevation_deg = e;
        self.sun_dir = Vec3::new(across.x * er.cos(), er.sin(), across.y * er.cos()).normalize();
        // The moon rides opposite the sun, high in the middle of the night.
        let me = (-e * 1.9).clamp(-10.0, 62.0).to_radians();
        self.moon_dir = Vec3::new(-across.x * me.cos(), me.sin(), -across.y * me.cos()).normalize();
        self.night = smooth(-2.0, -12.0, e);
        self.dusk = (1.0 - ((e - 1.5) / 11.0).abs()).clamp(0.0, 1.0);

        let day_horizon = lin(color(region.palette.sky));
        let day_zenith = air
            .zenith
            .map_or_else(|| super::sky::zenith_for(day_horizon), |z| lin(color(z)));
        let keys = [
            Key::new(-18.0, (30, 36, 62), (8, 10, 26), (104, 124, 206), 230.0),
            Key::new(-7.0, (118, 84, 132), (32, 34, 84), (150, 130, 200), 250.0),
            Key::new(-1.0, (252, 138, 92), (88, 80, 158), (232, 170, 170), 300.0),
            Key::new(
                6.0,
                (255, 194, 140),
                (104, 136, 204),
                (255, 218, 196),
                360.0,
            ),
            Key {
                e: 22.0,
                horizon: day_horizon,
                zenith: day_zenith,
                ambient: lin(color(air.ambient_color)),
                brightness: air.ambient_brightness,
            },
        ];
        let (a, b, t) = bracket(&keys, e);
        self.horizon = mix(a.horizon, b.horizon, t);
        self.zenith = mix(a.zenith, b.zenith, t);
        self.ambient = mix(a.ambient, b.ambient, t);
        self.ambient_brightness = a.brightness + (b.brightness - a.brightness) * t;

        // Sunlight warms and fades towards the horizon.
        let noon_sun = lin(color(air.sun_color));
        let sun_keys = [
            (-1.0, srgb(255, 112, 60)),
            (5.0, srgb(255, 170, 110)),
            (14.0, srgb(255, 222, 184)),
            (30.0, noon_sun),
        ];
        let i = sun_keys.iter().rposition(|k| k.0 <= e).unwrap_or(0);
        let j = (i + 1).min(sun_keys.len() - 1);
        let st = if i == j {
            0.0
        } else {
            ((e - sun_keys[i].0) / (sun_keys[j].0 - sun_keys[i].0)).clamp(0.0, 1.0)
        };
        self.sun_color = mix(sun_keys[i].1, sun_keys[j].1, st);
        if e > -3.0 {
            self.light_dir = self.sun_dir;
            self.light_color = self.sun_color;
            self.light_lux = air.sun_illuminance * (0.04 + 0.96 * smooth(-3.0, 26.0, e));
        } else {
            self.light_dir = self.moon_dir.with_y(self.moon_dir.y.max(0.25)).normalize();
            self.light_color = srgb(176, 196, 255);
            self.light_lux = MOON_LUX * smooth(-3.0, -12.0, e);
        }
    }

    /// A region without a day cycle: its fixed light, as authored.
    fn fixed(&mut self, region: &RegionDef, air: &Atmosphere) {
        let e = air.sun_elevation_deg;
        let er = e.to_radians();
        // From the south-west, so the player (facing north at the start)
        // sees lit flowers and shadows falling away.
        let across = Vec2::new(-30.0, 28.0).normalize() * er.cos();
        let dir = Vec3::new(across.x, er.sin(), across.y).normalize();
        self.elevation_deg = if air.night { -20.0 } else { e };
        self.sun_dir = dir;
        self.moon_dir = dir;
        self.night = if air.night { 1.0 } else { 0.0 };
        self.dusk = 0.0;
        self.light_dir = dir;
        self.light_color = lin(color(air.sun_color));
        self.light_lux = air.sun_illuminance;
        self.ambient = lin(color(air.ambient_color));
        self.ambient_brightness = air.ambient_brightness;
        self.horizon = lin(color(region.palette.sky));
        self.zenith = air
            .zenith
            .map_or_else(|| super::sky::zenith_for(self.horizon), |z| lin(color(z)));
        self.sun_color = self.light_color;
    }
}

struct Key {
    e: f32,
    horizon: LinearRgba,
    zenith: LinearRgba,
    ambient: LinearRgba,
    brightness: f32,
}

impl Key {
    fn new(
        e: f32,
        horizon: (u8, u8, u8),
        zenith: (u8, u8, u8),
        ambient: (u8, u8, u8),
        brightness: f32,
    ) -> Self {
        Self {
            e,
            horizon: srgb(horizon.0, horizon.1, horizon.2),
            zenith: srgb(zenith.0, zenith.1, zenith.2),
            ambient: srgb(ambient.0, ambient.1, ambient.2),
            brightness,
        }
    }
}

/// The two keys around `e` and how far between them it lies.
fn bracket(keys: &[Key], e: f32) -> (&Key, &Key, f32) {
    let last = keys.len() - 1;
    if e <= keys[0].e {
        return (&keys[0], &keys[0], 0.0);
    }
    if e >= keys[last].e {
        return (&keys[last], &keys[last], 0.0);
    }
    let i = keys.iter().rposition(|k| k.e <= e).unwrap_or(0);
    let (a, b) = (&keys[i], &keys[i + 1]);
    (a, b, smooth(a.e, b.e, e))
}

fn srgb(r: u8, g: u8, b: u8) -> LinearRgba {
    lin(Color::srgb_u8(r, g, b))
}

fn smooth(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Reset the clock to the region's morning when a region is (re)built.
fn start_clock(scene: Res<RegionScene>, content: Res<Content>, mut daylight: ResMut<Daylight>) {
    if daylight.started_for == Some(scene.region) && !scene.is_changed() {
        return;
    }
    let region = &content.regions[scene.region];
    let cycle = region.scenery.day_cycle;
    daylight.cycle = cycle.is_some();
    // Arriving again in the same region keeps the time of day.
    if daylight.started_for != Some(scene.region) {
        if let Some(c) = cycle {
            daylight.hour = std::env::var("BLOOM_HOUR")
                .ok()
                .and_then(|h| h.parse::<f32>().ok())
                .unwrap_or(c.start_hour)
                .rem_euclid(24.0);
            daylight.secs_per_hour = c.day_length_secs / 24.0;
        }
    }
    daylight.started_for = Some(scene.region);
    daylight.update(region);
}

/// Time passes while the game runs (it stands still while paused).
fn advance_clock(
    time: Res<Time>,
    scene: Res<RegionScene>,
    content: Res<Content>,
    mut daylight: ResMut<Daylight>,
) {
    if daylight.cycle {
        let hours = time.delta_secs() / daylight.secs_per_hour.max(0.01);
        daylight.hour = (daylight.hour + hours).rem_euclid(24.0);
    }
    daylight.update(&content.regions[scene.region]);
}

/// Sun or moon, ambient light, clear colour and fog follow the time of day.
fn apply_light(
    daylight: Res<Daylight>,
    mut clear: ResMut<ClearColor>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut lights: Query<(&mut DirectionalLight, &mut Transform), With<SceneEntity>>,
    mut fogs: Query<&mut DistanceFog, With<MainCamera>>,
) {
    let d = &*daylight;
    for (mut light, mut t) in &mut lights {
        light.color = Color::LinearRgba(d.light_color);
        light.illuminance = d.light_lux;
        let at = d.light_dir * 60.0;
        let want = Transform::from_translation(at).looking_at(Vec3::ZERO, Vec3::Y);
        if t.rotation.angle_between(want.rotation) > 1e-4 {
            *t = want;
        }
    }
    clear.0 = Color::LinearRgba(d.horizon);
    ambient.color = Color::LinearRgba(d.ambient);
    ambient.brightness = d.ambient_brightness;
    for mut fog in &mut fogs {
        fog.color = Color::LinearRgba(mix(d.horizon, d.zenith, 0.12));
        let scatter = d.sun_color.with_alpha(0.25 + 0.4 * d.dusk);
        fog.directional_light_color = if d.night > 0.5 {
            Color::srgba(0.7, 0.78, 1.0, 0.15)
        } else {
            Color::LinearRgba(scatter)
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bloom_core::data::GameData;

    fn farm_at(hour: f32) -> Daylight {
        let data = GameData::builtin().unwrap();
        let mut d = Daylight {
            cycle: true,
            hour,
            ..default()
        };
        d.update(&data.regions[0]);
        d
    }

    #[test]
    fn the_sun_rises_crosses_the_south_and_sets() {
        let morning = farm_at(6.5);
        let noon = farm_at(13.0);
        let evening = farm_at(20.0);
        let midnight = farm_at(1.0);
        assert!(
            morning.elevation_deg > 0.0 && morning.sun_dir.x > 0.5,
            "east"
        );
        assert!(noon.elevation_deg > 50.0 && noon.sun_dir.z > 0.0, "south");
        assert!(
            evening.elevation_deg > -3.0 && evening.sun_dir.x < -0.5,
            "west"
        );
        assert!(midnight.elevation_deg < -15.0 && midnight.night > 0.99);
        assert!(midnight.moon_dir.y > 0.5, "the moon is up at night");
        assert_eq!(noon.night, 0.0);
    }

    #[test]
    fn sunset_glows_and_night_stays_playable() {
        let sunset = farm_at(20.6);
        assert!(sunset.dusk > 0.5, "dusk {}", sunset.dusk);
        assert!(
            sunset.horizon.red > sunset.horizon.blue,
            "an orange horizon"
        );
        let night = farm_at(2.0);
        assert!(night.light_lux > 500.0, "moonlight");
        assert!(night.ambient_brightness >= 200.0);
        assert!(farm_at(13.0).light_lux > night.light_lux * 3.0);
    }

    #[test]
    fn the_clock_reads_hours_and_minutes() {
        assert_eq!(farm_at(7.5).clock(), "07:30");
        assert_eq!(farm_at(23.99).clock(), "23:59");
    }
}
