//! Native Bevy UI: design tokens, widgets and screens.
//!
//! UI reads game state and sends intents; it never mutates gameplay state
//! directly (from Phase 2 every change goes through `bloom_core` commands).

mod collection;
mod debug_overlay;
mod effects;
mod finale;
mod hud;
mod inventory;
mod journal;
mod main_menu;
mod map;
mod pause_menu;
mod skills;
pub mod theme;
mod toasts;
mod widgets;
mod workshop;

pub use toasts::Toast;

/// Top-level HUD elements (stats, prompt, tool bar, hints, minimap, gate
/// banner): hidden while the ending plays.
#[derive(Component)]
pub struct HudRoot;

use bevy::prelude::*;

pub struct UiPlugin;

impl Plugin for UiPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            widgets::WidgetsPlugin,
            main_menu::MainMenuPlugin,
            hud::HudPlugin,
            pause_menu::PauseMenuPlugin,
            debug_overlay::DebugOverlayPlugin,
            inventory::InventoryPlugin,
            toasts::ToastsPlugin,
            workshop::WorkshopPlugin,
            skills::SkillsPlugin,
            journal::JournalPlugin,
            map::MapPlugin,
            collection::CollectionPlugin,
            finale::FinalePlugin,
        ));
    }
}
