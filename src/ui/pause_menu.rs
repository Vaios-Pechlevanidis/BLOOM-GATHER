//! Pause overlay: resume, settings, main menu, quit. Esc toggles it;
//! losing window focus opens it.

use bevy::prelude::*;

use super::settings::SettingsScreen;
use super::theme::{self, font, palette, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::game::{AppState, PlayState};
use crate::input::Bindings;

pub struct PauseMenuPlugin;

impl Plugin for PauseMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(PlayState::Paused), spawn_overlay)
            .add_systems(
                Update,
                (
                    toggle_with_key.run_if(in_state(AppState::InGame)),
                    handle_buttons.run_if(in_state(PlayState::Paused)),
                ),
            );
    }
}

#[derive(Component, Debug, Clone, Copy)]
enum PauseAction {
    Resume,
    Settings,
    MainMenu,
    Quit,
}

fn spawn_overlay(mut commands: Commands) {
    commands
        .spawn((
            DespawnOnExit(PlayState::Paused),
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
                spawn_button(
                    card,
                    "Settings (O)",
                    ButtonVariant::Secondary,
                    PauseAction::Settings,
                );
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

fn toggle_with_key(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    screen: Res<SettingsScreen>,
    mut next_state: ResMut<NextState<PlayState>>,
) {
    // Esc closes the settings screen first (it handles that itself).
    if screen.open || screen.is_changed() {
        return;
    }
    if Bindings::just_pressed(&keys, &bindings.pause) {
        next_state.set(match state.get() {
            PlayState::Playing => PlayState::Paused,
            // Esc closes the basket rather than pausing on top of it.
            PlayState::Paused
            | PlayState::Inventory
            | PlayState::Workshop
            | PlayState::Skills
            | PlayState::Journal
            | PlayState::Map
            | PlayState::Collection => PlayState::Playing,
            // The ending handles Esc itself (skip the tour).
            PlayState::Finale => return,
        });
    }
}

fn handle_buttons(
    buttons: Query<(&Interaction, &PauseAction), Changed<Interaction>>,
    keys: Res<ButtonInput<KeyCode>>,
    mut screen: ResMut<SettingsScreen>,
    mut play_state: ResMut<NextState<PlayState>>,
    mut app_state: ResMut<NextState<AppState>>,
    mut exit: MessageWriter<AppExit>,
) {
    if !screen.open && keys.just_pressed(KeyCode::KeyO) {
        screen.show();
    }
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed || screen.open {
            continue;
        }
        match *action {
            PauseAction::Resume => play_state.set(PlayState::Playing),
            PauseAction::Settings => screen.show(),
            PauseAction::MainMenu => app_state.set(AppState::MainMenu),
            PauseAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}
