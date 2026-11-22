use bevy::prelude::*;

/// Top-level screens. Entities tagged with `DespawnOnExit(state)` are despawned
/// automatically when that state exits, so screens never leak entities.
#[derive(States, Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
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
pub enum PlayState {
    #[default]
    Playing,
    Paused,
    /// The inventory (basket) screen is open.
    Inventory,
    /// The workbench screen is open (crafting keeps running).
    Workshop,
    /// The skill tree is open.
    Skills,
    /// The journal (achievements, statistics, progress) is open.
    Journal,
    /// The world map is open.
    Map,
    /// The herbarium: every species and bouquet collected.
    Collection,
    /// The ending: celebration, a tour of the valley, the journey summary.
    Finale,
}
