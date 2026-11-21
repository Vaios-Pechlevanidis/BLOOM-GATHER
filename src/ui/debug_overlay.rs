//! F3 performance overlay: FPS, frame time, entity count, player and state.
//! Later phases add visible/total flowers and harvest/craft/save timings.

use bevy::diagnostic::{
    DiagnosticPath, DiagnosticsStore, EntityCountDiagnosticsPlugin, FrameTimeDiagnosticsPlugin,
};
use bevy::prelude::*;

use super::theme::{font, palette, radius, space};
use crate::game::AppState;
use crate::input::Bindings;
use crate::player::Player;

pub struct DebugOverlayPlugin;

impl Plugin for DebugOverlayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RefreshTimer(Timer::from_seconds(
            0.25,
            TimerMode::Repeating,
        )))
        .add_systems(Startup, spawn_overlay)
        .add_systems(Update, (toggle, refresh).chain());
    }
}

/// The panel (owns visibility).
#[derive(Component)]
struct DebugOverlay;

/// The text inside the panel.
#[derive(Component)]
struct DebugText;

#[derive(Resource)]
struct RefreshTimer(Timer);

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            DebugOverlay,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(space::MD),
                top: Val::Px(space::MD),
                padding: UiRect::axes(Val::Px(space::MD), Val::Px(space::SM)),
                ..default()
            },
            BackgroundColor(palette::INK.with_alpha(0.78)),
            BorderRadius::all(Val::Px(radius::SM)),
            GlobalZIndex(100),
            Visibility::Hidden,
        ))
        .with_children(|panel| {
            panel.spawn((
                DebugText,
                Text::new(""),
                TextFont {
                    font_size: font::CAPTION,
                    ..default()
                },
                TextColor(palette::CREAM),
            ));
        });
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    mut overlay: Query<&mut Visibility, With<DebugOverlay>>,
    mut timer: ResMut<RefreshTimer>,
) {
    if !Bindings::just_pressed(&keys, &bindings.toggle_debug) {
        return;
    }
    for mut visibility in &mut overlay {
        *visibility = match *visibility {
            Visibility::Hidden => Visibility::Visible,
            _ => Visibility::Hidden,
        };
    }
    // Refresh immediately when shown.
    let duration = timer.0.duration();
    timer.0.set_elapsed(duration);
}

fn refresh(
    time: Res<Time<Real>>,
    mut timer: ResMut<RefreshTimer>,
    diagnostics: Res<DiagnosticsStore>,
    state: Res<State<AppState>>,
    player: Option<Res<Player>>,
    overlay: Query<&Visibility, With<DebugOverlay>>,
    mut texts: Query<&mut Text, With<DebugText>>,
) {
    if !timer.0.tick(time.delta()).just_finished() {
        return;
    }
    if overlay.single().map_or(true, |v| *v == Visibility::Hidden) {
        return;
    }
    let Ok(mut text) = texts.single_mut() else {
        return;
    };
    let smoothed = |path: &DiagnosticPath| {
        diagnostics
            .get(path)
            .and_then(|d| d.smoothed())
            .unwrap_or(0.0)
    };
    let fps = smoothed(&FrameTimeDiagnosticsPlugin::FPS);
    let frame_ms = smoothed(&FrameTimeDiagnosticsPlugin::FRAME_TIME);
    let entities = diagnostics
        .get(&EntityCountDiagnosticsPlugin::ENTITY_COUNT)
        .and_then(|d| d.value())
        .unwrap_or(0.0);
    let player_line = match player.as_deref() {
        Some(p) => {
            let b = &p.body;
            format!(
                "player ({:.1}, {:.1}, {:.1})  speed {:.1} m/s  {}\nlook yaw {:.0} deg  pitch {:.0} deg",
                b.position.x,
                b.position.y,
                b.position.z,
                b.horizontal_speed(),
                if b.grounded { "grounded" } else { "airborne" },
                p.yaw.to_degrees(),
                p.pitch.to_degrees()
            )
        }
        None => "player -".to_string(),
    };

    text.0 = format!(
        "FPS {fps:>5.1}   frame {frame_ms:>5.2} ms\nentities {entities:.0}\n{player_line}\nstate {:?}",
        state.get()
    );
}
