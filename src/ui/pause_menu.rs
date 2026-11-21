//! Pause overlay with comfort settings. Esc toggles it; losing window focus
//! opens it.

use bevy::prelude::*;

use super::theme::{self, font, palette, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::game::{AppState, PlayState, Settings};
use crate::input::Bindings;

pub struct PauseMenuPlugin;

impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(PlayState::Paused), spawn_overlay)
            .add_systems(
                Update,
                (
                    toggle_with_key.run_if(in_state(AppState::InGame)),
                    (handle_buttons, refresh_values)
                        .chain()
                        .run_if(in_state(PlayState::Paused)),
                ),
            );
    }
}

#[derive(Component, Debug, Clone, Copy)]
enum PauseAction {
    Resume,
    MainMenu,
    Quit,
    Setting(SettingKind, f32),
}

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq)]
enum SettingKind {
    Sensitivity,
    Fov,
    HeadBob,
    InvertY,
}

impl SettingKind {
    fn label(self) -> &'static str {
        match self {
            SettingKind::Sensitivity => "Mouse sensitivity",
            SettingKind::Fov => "Field of view",
            SettingKind::HeadBob => "Head movement",
            SettingKind::InvertY => "Invert look",
        }
    }

    fn value(self, s: &Settings) -> String {
        let on_off = |b: bool| {
            if b {
                "On".to_string()
            } else {
                "Off".to_string()
            }
        };
        match self {
            SettingKind::Sensitivity => format!("{:.1}", s.mouse_sensitivity),
            SettingKind::Fov => format!("{:.0}", s.fov_degrees),
            SettingKind::HeadBob => on_off(s.head_bob),
            SettingKind::InvertY => on_off(s.invert_y),
        }
    }

    fn apply(self, s: &mut Settings, steps: f32) {
        match self {
            SettingKind::Sensitivity => s.adjust_sensitivity(steps),
            SettingKind::Fov => s.adjust_fov(steps),
            SettingKind::HeadBob => s.head_bob = !s.head_bob,
            SettingKind::InvertY => s.invert_y = !s.invert_y,
        }
    }

    fn is_toggle(self) -> bool {
        matches!(self, SettingKind::HeadBob | SettingKind::InvertY)
    }
}

/// The text node showing a setting's current value.
#[derive(Component)]
struct SettingValue(SettingKind);

fn spawn_overlay(mut commands: Commands, settings: Res<Settings>) {
    commands
        .spawn((
            StateScoped(PlayState::Paused),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::SCRIM),
            GlobalZIndex(10),
        ))
        .with_children(|root| {
            root.spawn(theme::card(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(space::XXL), Val::Px(space::XL)),
                row_gap: Val::Px(space::MD),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }))
            .with_children(|card| {
                card.spawn(theme::text("Paused", font::HEADING, palette::INK));
                card.spawn(theme::text(
                    "The meadow will wait for you.",
                    font::BODY,
                    palette::INK_SOFT,
                ));
                card.spawn(Node {
                    height: Val::Px(space::SM),
                    ..default()
                });
                spawn_button(card, "Resume", ButtonVariant::Primary, PauseAction::Resume);

                card.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(space::SM),
                    margin: UiRect::vertical(Val::Px(space::MD)),
                    ..default()
                })
                .with_children(|list| {
                    for kind in [
                        SettingKind::Sensitivity,
                        SettingKind::Fov,
                        SettingKind::HeadBob,
                        SettingKind::InvertY,
                    ] {
                        spawn_setting_row(list, kind, &settings);
                    }
                });

                spawn_button(
                    card,
                    "Main Menu",
                    ButtonVariant::Secondary,
                    PauseAction::MainMenu,
                );
                spawn_button(
                    card,
                    "Quit Game",
                    ButtonVariant::Secondary,
                    PauseAction::Quit,
                );
            });
        });
}

fn spawn_setting_row(parent: &mut ChildSpawnerCommands, kind: SettingKind, settings: &Settings) {
    parent
        .spawn(Node {
            width: Val::Px(400.0),
            justify_content: JustifyContent::SpaceBetween,
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::MD),
            ..default()
        })
        .with_children(|row| {
            row.spawn(theme::text(kind.label(), font::BODY, palette::INK));
            row.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::SM),
                ..default()
            })
            .with_children(|control| {
                if kind.is_toggle() {
                    spawn_button(
                        control,
                        "Toggle",
                        ButtonVariant::Compact,
                        PauseAction::Setting(kind, 1.0),
                    );
                } else {
                    spawn_button(
                        control,
                        "-",
                        ButtonVariant::Compact,
                        PauseAction::Setting(kind, -1.0),
                    );
                }
                control
                    .spawn(Node {
                        width: Val::Px(46.0),
                        justify_content: JustifyContent::Center,
                        ..default()
                    })
                    .with_children(|v| {
                        v.spawn((
                            SettingValue(kind),
                            theme::text(kind.value(settings), font::BODY, palette::INK),
                        ));
                    });
                if !kind.is_toggle() {
                    spawn_button(
                        control,
                        "+",
                        ButtonVariant::Compact,
                        PauseAction::Setting(kind, 1.0),
                    );
                }
            });
        });
}

fn toggle_with_key(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    mut next_state: ResMut<NextState<PlayState>>,
) {
    if Bindings::just_pressed(&keys, &bindings.pause) {
        next_state.set(match state.get() {
            PlayState::Playing => PlayState::Paused,
            PlayState::Paused => PlayState::Playing,
        });
    }
}

fn handle_buttons(
    buttons: Query<(&Interaction, &PauseAction), Changed<Interaction>>,
    mut settings: ResMut<Settings>,
    mut play_state: ResMut<NextState<PlayState>>,
    mut app_state: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            PauseAction::Resume => play_state.set(PlayState::Playing),
            PauseAction::MainMenu => app_state.set(AppState::MainMenu),
            PauseAction::Quit => {
                exit.write(AppExit::Success);
            }
            PauseAction::Setting(kind, steps) => kind.apply(&mut settings, steps),
        }
    }
}

fn refresh_values(settings: Res<Settings>, mut values: Query<(&SettingValue, &mut Text)>) {
    if !settings.is_changed() {
        return;
    }
    for (SettingValue(kind), mut text) in &mut values {
        text.0 = kind.value(&settings);
    }
}
