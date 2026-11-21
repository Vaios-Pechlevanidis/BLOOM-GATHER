//! Application wiring: window configuration, engine plugins and our plugins.

use bevy::diagnostic::{EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin};
use bevy::prelude::*;
use bevy::window::{PresentMode, WindowResizeConstraints, WindowResolution};

use crate::{audio, flowers, game, input, interaction, player, rendering, ui};

pub const GAME_TITLE: &str = "Bloom & Gather";
pub const GAME_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn build() -> App {
    // The smoke test must never touch the player's real save or settings.
    #[cfg(debug_assertions)]
    crate::dev::isolate_save_dir();
    let mut app = App::new();
    app.add_plugins(DefaultPlugins.set(WindowPlugin {
        primary_window: Some(Window {
            title: GAME_TITLE.into(),
            resolution: WindowResolution::new(1280.0, 800.0),
            resize_constraints: WindowResizeConstraints {
                min_width: 960.0,
                min_height: 600.0,
                ..default()
            },
            present_mode: PresentMode::AutoVsync,
            ..default()
        }),
        ..default()
    }))
    .add_plugins((
        FrameTimeDiagnosticsPlugin::default(),
        EntityCountDiagnosticsPlugin,
    ))
    .add_plugins((
        game::GamePlugin,
        input::InputPlugin,
        interaction::InteractionPlugin,
        flowers::FlowersPlugin,
        audio::AudioPlugin,
        player::PlayerPlugin,
        rendering::RenderingPlugin,
        ui::UiPlugin,
    ));
    #[cfg(debug_assertions)]
    app.add_plugins(crate::dev::SmokeTestPlugin);
    app
}
