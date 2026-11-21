use bevy::prelude::*;

/// Top-level screens. Entities tagged with `StateScoped(state)` are despawned
/// automatically when that state exits, so screens never leak entities.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[states(scoped_entities)]
pub enum AppState {
    /// Load and validate content; leaves immediately on success.
    #[default]
    Boot,
    MainMenu,
    InGame,
}

/// Exists only while in [`AppState::InGame`].
#[derive(SubStates, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[source(AppState = AppState::InGame)]
#[states(scoped_entities)]
pub enum PlayState {
    #[default]
    Playing,
    Paused,
    /// The inventory (basket) screen is open.
    Inventory,
}
