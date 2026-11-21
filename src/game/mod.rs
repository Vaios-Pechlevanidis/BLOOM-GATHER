//! Application state machine, settings and the bridge to `bloom_core`.
//!
//! Loads and validates content, runs the `AppState` / `PlayState` machine,
//! pauses virtual time and holds player settings. Later phases add the
//! authoritative `bloom_core` game simulation resource here.

mod content;
mod settings;
mod states;

pub use content::{ActiveRegion, Content, RegionScene};
pub use settings::Settings;
pub use states::{AppState, PlayState};

use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Settings>()
            .init_state::<AppState>()
            .add_sub_state::<PlayState>()
            .add_systems(OnEnter(AppState::Boot), content::load_content)
            .add_systems(OnEnter(PlayState::Paused), pause_time)
            .add_systems(OnExit(PlayState::Paused), resume_time);
    }
}

/// Freeze gameplay animations (wind, effects) while the pause menu is open.
/// UI hover transitions use real time and keep working.
fn pause_time(mut time: ResMut<Time<Virtual>>) {
    time.pause();
}

fn resume_time(mut time: ResMut<Time<Virtual>>) {
    time.unpause();
}
