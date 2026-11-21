//! Player-adjustable settings (camera feel, comfort, sound), saved next to
//! the save slot as `settings.ron` whenever they change.

use bevy::prelude::*;
use bloom_core::persistence::{write_atomic, SaveStore};
use serde::{Deserialize, Serialize};

pub struct SettingsPlugin;

impl Plugin for SettingsPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Settings::load())
            .add_systems(Last, persist_settings);
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
    /// Pause automatically when the window loses focus.
    pub pause_on_focus_loss: bool,
}

impl Settings {
    /// Radians of rotation per unit of raw mouse motion at sensitivity 1.
    pub const BASE_LOOK_RATE: f32 = 0.0022;
    pub const SENSITIVITY_RANGE: (f32, f32) = (0.2, 3.0);
    pub const SENSITIVITY_STEP: f32 = 0.1;
    pub const FOV_RANGE: (f32, f32) = (55.0, 100.0);
    pub const FOV_STEP: f32 = 5.0;
    pub const VOLUME_STEP: f32 = 0.1;

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
        self.sound_volume = self.sound_volume.clamp(0.0, 1.0);
        self
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

    pub fn adjust_volume(&mut self, steps: f32) {
        let v = self.sound_volume + steps * Self::VOLUME_STEP;
        self.sound_volume = ((v * 10.0).round() / 10.0).clamp(0.0, 1.0);
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
            pause_on_focus_loss: true,
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
            s.adjust_volume(1.0);
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
    }
}
