//! Application state machine, settings and the bridge to `bloom_core`.
//!
//! Loads and validates content, runs the `AppState` / `PlayState` machine,
//! pauses virtual time, holds player settings and owns the authoritative
//! game session (load, autosave, new game).

mod content;
mod session;
mod settings;
mod states;

pub use content::{ActiveRegion, Content, RegionScene, Touring};
pub use session::{start_new_game, EventsForwarded, GameEventMsg, Saves, Session, SessionSet};
pub use settings::Settings;
pub use states::{AppState, PlayState};

use bevy::prelude::*;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((settings::SettingsPlugin, session::SessionPlugin))
            .init_state::<AppState>()
            .add_sub_state::<PlayState>()
            .add_systems(OnEnter(AppState::Boot), content::load_content)
            .add_systems(OnEnter(PlayState::Paused), pause_time)
            .add_systems(OnExit(PlayState::Paused), resume_time)
            .add_systems(OnEnter(PlayState::Inventory), pause_time)
            .add_systems(OnExit(PlayState::Inventory), resume_time)
            .add_systems(OnEnter(PlayState::Skills), pause_time)
            .add_systems(OnExit(PlayState::Skills), resume_time)
            .add_systems(OnEnter(PlayState::Journal), pause_time)
            .add_systems(OnExit(PlayState::Journal), resume_time)
            .add_systems(OnEnter(PlayState::Map), pause_time)
            .add_systems(OnExit(PlayState::Map), resume_time)
            .add_systems(OnEnter(PlayState::Collection), pause_time)
            .add_systems(OnExit(PlayState::Collection), resume_time);
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
