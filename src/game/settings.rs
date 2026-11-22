//! Player-adjustable settings (controls, comfort, graphics, sound, key
//! bindings), saved next to the save slot as `settings.ron` whenever they
//! change. Missing fields (older files) take their defaults.

use bevy::prelude::*;
use bloom_core::persistence::{write_atomic, SaveStore};
use serde::{Deserialize, Serialize};

use crate::input::KeyOverrides;

/// How much the renderer does. High is the full look; Low suits older
/// laptops.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Quality {
    Low,
    Medium,
    #[default]
    High,
}

impl Quality {
    pub const ALL: [Quality; 3] = [Quality::Low, Quality::Medium, Quality::High];

    pub fn label(self) -> &'static str {
        match self {
            Quality::Low => "Low",
            Quality::Medium => "Medium",
            Quality::High => "High",
        }
    }

    pub fn shadows(self) -> bool {
        self != Quality::Low
    }

    pub fn shadow_cascades(self) -> usize {
        match self {
            Quality::High => 3,
            _ => 2,
        }
    }

    /// Share of the decorative grass that is drawn.
    pub fn grass_density(self) -> f32 {
        match self {
            Quality::Low => 0.35,
            Quality::Medium => 0.7,
            Quality::High => 1.0,
        }
    }

    /// Grass fades out between these distances (metres).
    pub fn grass_distance(self) -> (f32, f32) {
        match self {
            Quality::Low => (24.0, 34.0),
            Quality::Medium => (32.0, 46.0),
            Quality::High => (38.0, 55.0),
        }
    }

    pub fn msaa(self) -> bool {
        self != Quality::Low
    }

    /// Soft glow around bright things (lanterns, glowing flowers).
    pub fn bloom(self) -> bool {
        self != Quality::Low
    }
}

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load())
            .add_systems(Last, persist_settings)
            .add_systems(PreUpdate, apply_settings);
    }
}

#[derive(Resource, Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Multiplier on the base mouse-look rate.
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    /// Vertical field of view in degrees.
    pub fov_degrees: f32,
    /// Gentle head movement while walking and a small dip on landing.
    pub head_bob: bool,
    /// Sound effect volume, 0–1.
    pub sound_volume: f32,
    /// Overall volume, 0–1 (multiplies effects, music and ambience).
    pub master_volume: f32,
    pub music_volume: f32,
    pub ambience_volume: f32,
    /// Pause automatically when the window loses focus.
    pub pause_on_focus_loss: bool,
    pub quality: Quality,
    pub fullscreen: bool,
    pub vsync: bool,
    /// Size of all menus and the HUD.
    pub ui_scale: f32,
    pub show_minimap: bool,
    /// Rebound primary keys (action id → key name).
    pub keys: KeyOverrides,
}

impl Settings {
    /// Radians of rotation per unit of raw mouse motion at sensitivity 1.
    pub const BASE_LOOK_RATE: f32 = 0.0022;
    pub const SENSITIVITY_RANGE: (f32, f32) = (0.2, 3.0);
    pub const SENSITIVITY_STEP: f32 = 0.1;
    pub const FOV_RANGE: (f32, f32) = (55.0, 100.0);
    pub const FOV_STEP: f32 = 5.0;
    pub const VOLUME_STEP: f32 = 0.1;
    pub const UI_SCALE_RANGE: (f32, f32) = (0.8, 1.4);

    fn path() -> std::path::PathBuf {
        SaveStore::default_dir().join("settings.ron")
    }

    /// Saved settings, or defaults if there are none or they can't be read.
    pub fn load() -> Self {
        match std::fs::read_to_string(Self::path()) {
            Ok(text) => ron::from_str::<Settings>(&text)
                .map(Settings::sanitized)
                .unwrap_or_else(|err| {
                    warn!("Ignoring unreadable settings file: {err}");
                    Settings::default()
                }),
            Err(_) => Settings::default(),
        }
    }

    /// Clamp values that may have been edited by hand.
    fn sanitized(mut self) -> Self {
        let (lo, hi) = Self::SENSITIVITY_RANGE;
        self.mouse_sensitivity = self.mouse_sensitivity.clamp(lo, hi);
        let (lo, hi) = Self::FOV_RANGE;
        self.fov_degrees = self.fov_degrees.clamp(lo, hi);
        for v in [
            &mut self.sound_volume,
            &mut self.master_volume,
            &mut self.music_volume,
            &mut self.ambience_volume,
        ] {
            *v = v.clamp(0.0, 1.0);
        }
        let (lo, hi) = Self::UI_SCALE_RANGE;
        self.ui_scale = self.ui_scale.clamp(lo, hi);
        self
    }

    /// Effective volume of a channel (master included).
    pub fn effects(&self) -> f32 {
        self.master_volume * self.sound_volume
    }

    pub fn music(&self) -> f32 {
        self.master_volume * self.music_volume
    }

    pub fn ambience(&self) -> f32 {
        self.master_volume * self.ambience_volume
    }

    /// One step of 10 % on a 0–1 volume, without drift.
    pub fn step_volume(v: &mut f32, steps: f32) {
        *v = (((*v + steps * Self::VOLUME_STEP) * 10.0).round() / 10.0).clamp(0.0, 1.0);
    }

    pub fn adjust_ui_scale(&mut self, steps: f32) {
        let (lo, hi) = Self::UI_SCALE_RANGE;
        self.ui_scale = (((self.ui_scale + steps * 0.1) * 10.0).round() / 10.0).clamp(lo, hi);
    }

    pub fn cycle_quality(&mut self, steps: f32) {
        let i = Quality::ALL
            .iter()
            .position(|q| *q == self.quality)
            .unwrap_or(2) as i32;
        let j = (i + steps.signum() as i32).clamp(0, 2) as usize;
        self.quality = Quality::ALL[j];
    }

    pub fn adjust_sensitivity(&mut self, steps: f32) {
        let (lo, hi) = Self::SENSITIVITY_RANGE;
        let v = self.mouse_sensitivity + steps * Self::SENSITIVITY_STEP;
        // Round to one decimal so repeated steps don't drift.
        self.mouse_sensitivity = ((v * 10.0).round() / 10.0).clamp(lo, hi);
    }

    pub fn adjust_fov(&mut self, steps: f32) {
        let (lo, hi) = Self::FOV_RANGE;
        self.fov_degrees = (self.fov_degrees + steps * Self::FOV_STEP).clamp(lo, hi);
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            mouse_sensitivity: 1.0,
            invert_y: false,
            fov_degrees: 75.0,
            head_bob: true,
            sound_volume: 0.7,
            master_volume: 1.0,
            music_volume: 0.5,
            ambience_volume: 0.7,
            pause_on_focus_loss: true,
            quality: Quality::High,
            fullscreen: false,
            vsync: true,
            ui_scale: 1.0,
            show_minimap: true,
            keys: KeyOverrides::new(),
        }
    }
}

/// Key bindings, window mode, vsync and UI size follow the settings.
fn apply_settings(
    settings: Res<Settings>,
    mut bindings: ResMut<crate::input::Bindings>,
    mut ui_scale: ResMut<UiScale>,
    mut windows: Query<&mut Window, With<bevy::window::PrimaryWindow>>,
) {
    if !settings.is_changed() {
        return;
    }
    let wanted = crate::input::Bindings::from_overrides(&settings.keys);
    if *bindings != wanted {
        *bindings = wanted;
    }
    if (ui_scale.0 - settings.ui_scale).abs() > 1e-3 {
        ui_scale.0 = settings.ui_scale;
    }
    use bevy::window::{MonitorSelection, PresentMode, WindowMode};
    for mut w in &mut windows {
        let mode = if settings.fullscreen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        if w.mode != mode {
            w.mode = mode;
        }
        let present = if settings.vsync {
            PresentMode::AutoVsync
        } else {
            PresentMode::AutoNoVsync
        };
        if w.present_mode != present {
            w.present_mode = present;
        }
    }
}

fn persist_settings(settings: Res<Settings>) {
    if !settings.is_changed() || settings.is_added() {
        return;
    }
    let text = ron::ser::to_string_pretty(&*settings, ron::ser::PrettyConfig::default())
        .expect("settings are serialisable");
    if let Err(err) = write_atomic(&Settings::path(), text.as_bytes()) {
        warn!("Could not save settings: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjustments_clamp_and_do_not_drift() {
        let mut s = Settings::default();
        for _ in 0..7 {
            s.adjust_sensitivity(1.0);
        }
        assert_eq!(s.mouse_sensitivity, 1.7);
        for _ in 0..100 {
            s.adjust_sensitivity(-1.0);
        }
        assert_eq!(s.mouse_sensitivity, 0.2);
        for _ in 0..100 {
            s.adjust_fov(1.0);
        }
        assert_eq!(s.fov_degrees, 100.0);
        for _ in 0..3 {
            Settings::step_volume(&mut s.sound_volume, 1.0);
        }
        assert_eq!(s.sound_volume, 1.0);
    }

    #[test]
    fn settings_round_trip_and_sanitize() {
        let s = Settings {
            fov_degrees: 90.0,
            invert_y: true,
            ..Default::default()
        };
        let text = ron::to_string(&s).unwrap();
        assert_eq!(ron::from_str::<Settings>(&text).unwrap(), s);
        // Missing fields fall back to defaults; silly values are clamped.
        let edited: Settings = ron::from_str("(fov_degrees: 500.0)").unwrap();
        assert_eq!(edited.sanitized().fov_degrees, Settings::FOV_RANGE.1);
        // A file from before the full settings screen keeps its values.
        let old: Settings = ron::from_str("(sound_volume: 0.3, invert_y: true)").unwrap();
        assert_eq!(old.sound_volume, 0.3);
        assert_eq!(old.music_volume, Settings::default().music_volume);
        assert_eq!(old.quality, Quality::High);
    }

    #[test]
    fn channels_multiply_by_master_and_quality_steps_clamp() {
        let mut s = Settings {
            master_volume: 0.5,
            music_volume: 0.8,
            ..Default::default()
        };
        assert!((s.music() - 0.4).abs() < 1e-6);
        s.cycle_quality(1.0);
        assert_eq!(s.quality, Quality::High);
        s.cycle_quality(-1.0);
        s.cycle_quality(-1.0);
        s.cycle_quality(-1.0);
        assert_eq!(s.quality, Quality::Low);
        for _ in 0..10 {
            s.adjust_ui_scale(1.0);
        }
        assert_eq!(s.ui_scale, 1.4);
        let mut keyed = Settings::default();
        keyed.keys.insert("map".into(), "N".into());
        let text = ron::to_string(&keyed).unwrap();
        assert_eq!(ron::from_str::<Settings>(&text).unwrap().keys["map"], "N");
    }
}
