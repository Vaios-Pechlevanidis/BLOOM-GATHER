//! Input bindings.
//!
//! Gameplay systems never hard-code keys; they read [`Bindings`], which the
//! settings screen will make rebindable. Defaults follow the first-person
//! spec: WASD, Shift sprint, Space jump, E pick/interact, Tab basket, Esc
//! pause. M (map) is added together with the map, so no key is ever bound to
//! nothing.

use bevy::prelude::*;

pub struct InputPlugin;

impl Plugin for InputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bindings>();
    }
}

/// Action → key bindings. Several keys may map to one action.
#[derive(Resource, Debug, Clone)]
pub struct Bindings {
    pub forward: Vec<KeyCode>,
    pub back: Vec<KeyCode>,
    pub left: Vec<KeyCode>,
    pub right: Vec<KeyCode>,
    pub sprint: Vec<KeyCode>,
    pub jump: Vec<KeyCode>,
    pub interact: Vec<KeyCode>,
    pub inventory: Vec<KeyCode>,
    pub pause: Vec<KeyCode>,
    pub confirm: Vec<KeyCode>,
    pub toggle_debug: Vec<KeyCode>,
}

impl Default for Bindings {
    fn default() -> Self {
        use KeyCode::*;
        Self {
            forward: vec![KeyW, ArrowUp],
            back: vec![KeyS, ArrowDown],
            left: vec![KeyA, ArrowLeft],
            right: vec![KeyD, ArrowRight],
            sprint: vec![ShiftLeft, ShiftRight],
            jump: vec![Space],
            interact: vec![KeyE],
            inventory: vec![Tab],
            pause: vec![Escape],
            confirm: vec![Enter, NumpadEnter],
            toggle_debug: vec![F3],
        }
    }
}

impl Bindings {
    pub fn pressed(keys: &ButtonInput<KeyCode>, codes: &[KeyCode]) -> bool {
        keys.any_pressed(codes.iter().copied())
    }

    pub fn just_pressed(keys: &ButtonInput<KeyCode>, codes: &[KeyCode]) -> bool {
        keys.any_just_pressed(codes.iter().copied())
    }

    /// Movement keys as a local vector: x = right, y = forward.
    pub fn move_axis(&self, keys: &ButtonInput<KeyCode>) -> Vec2 {
        let axis = |pos: &[KeyCode], neg: &[KeyCode]| {
            Self::pressed(keys, pos) as i8 as f32 - Self::pressed(keys, neg) as i8 as f32
        };
        Vec2::new(
            axis(&self.right, &self.left),
            axis(&self.forward, &self.back),
        )
    }
}
