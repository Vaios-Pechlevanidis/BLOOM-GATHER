//! Title screen.

use bevy::prelude::*;

use super::theme::{self, font, palette, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::app::{GAME_TITLE, GAME_VERSION};
use crate::game::{AppState, Content};
use crate::input::Bindings;
use crate::rendering::color;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_menu)
            .add_systems(
                Update,
                (handle_buttons, handle_keys).run_if(in_state(AppState::MainMenu)),
            );
    }
}

#[derive(Component, Debug, Clone, Copy)]
enum MenuAction {
    Begin,
    Quit,
}

fn spawn_menu(mut commands: Commands, content: Res<Content>) {
    // A row of petal colours under the title, taken from the first species.
    let petals: Vec<Color> = content
        .species
        .iter()
        .take(6)
        .map(|s| color(s.petal_color))
        .collect();

    commands
        .spawn((
            StateScoped(AppState::MainMenu),
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                theme::card(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(64.0), Val::Px(space::XXL)),
                    row_gap: Val::Px(space::MD),
                    border: UiRect::all(Val::Px(2.0)),
                    ..default()
                }),
                Interaction::default(), // blocks world clicks behind the card
            ))
            .with_children(|card| {
                card.spawn(theme::text(GAME_TITLE, font::HERO, palette::INK));
                card.spawn(Node {
                    column_gap: Val::Px(space::SM),
                    margin: UiRect::vertical(Val::Px(space::XS)),
                    ..default()
                })
                .with_children(|row| {
                    for petal in petals {
                        row.spawn((
                            Node {
                                width: Val::Px(12.0),
                                height: Val::Px(12.0),
                                ..default()
                            },
                            BackgroundColor(petal),
                            BorderRadius::all(Val::Px(6.0)),
                        ));
                    }
                });
                card.spawn(theme::text(
                    "Gather every flower. Craft every bouquet. Bloom where you're planted.",
                    font::BODY,
                    palette::INK_SOFT,
                ));
                card.spawn(Node {
                    height: Val::Px(space::LG),
                    ..default()
                });
                spawn_button(card, "Begin", ButtonVariant::Primary, MenuAction::Begin);
                spawn_button(card, "Quit", ButtonVariant::Secondary, MenuAction::Quit);
            });

            root.spawn((
                Node {
                    position_type: PositionType::Absolute,
                    right: Val::Px(space::LG),
                    bottom: Val::Px(space::MD),
                    ..default()
                },
                theme::text(format!("v{GAME_VERSION}"), font::CAPTION, palette::INK_SOFT),
            ));
        });
}

fn handle_buttons(
    buttons: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match action {
            MenuAction::Begin => next_state.set(AppState::InGame),
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    if Bindings::just_pressed(&keys, &bindings.confirm) {
        next_state.set(AppState::InGame);
    }
}
