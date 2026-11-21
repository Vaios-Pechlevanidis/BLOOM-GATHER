//! Player-adjustable settings (camera feel and comfort).
//!
//! Changed from the pause menu. Persisting them to disk arrives with the save
//! system; until then they reset each launch.

use bevy::prelude::*;

#[derive(Resource, Debug, Clone, PartialEq)]
pub struct Settings {
    /// Multiplier on the base mouse-look rate.
    pub mouse_sensitivity: f32,
    pub invert_y: bool,
    /// Vertical field of view in degrees.
    pub fov_degrees: f32,
    /// Gentle head movement while walking and a small dip on landing.
    pub head_bob: bool,
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
            pause_on_focus_loss: true,
        }
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
    }
}
