//! Title screen: Continue / New Game (with confirmation before replacing a
//! saved game) / Quit, over the slowly orbiting meadow.

use bevy::prelude::*;

use super::settings::SettingsScreen;
use super::theme::{self, font, palette, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::app::{GAME_TITLE, GAME_VERSION};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{start_new_game, AppState, Content, Saves, Session, SessionSet};
use crate::input::Bindings;
use crate::rendering::color;

pub struct MainMenuPlugin;

impl Plugin for MainMenuPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::MainMenu), spawn_menu.after(SessionSet))
            .add_systems(
                Update,
                (handle_buttons, handle_keys).run_if(in_state(AppState::MainMenu)),
            );
    }
}

#[derive(Component, Debug, Clone, Copy)]
enum MenuAction {
    /// Continue the saved game (or begin a first one).
    Play,
    /// Ask to confirm replacing the saved game.
    AskNewGame,
    ConfirmNewGame,
    CancelNewGame,
    Settings,
    Quit,
}

/// The two panels of the card; one is shown at a time.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Panel {
    Main,
    Confirm,
}

fn spawn_menu(mut commands: Commands, content: Res<Content>, saves: Res<Saves>) {
    let petals: Vec<Color> = content
        .species
        .iter()
        .take(7)
        .map(|s| color(s.petal_color))
        .collect();
    let has_save = saves.has_save;

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
            root.spawn(theme::card(Node {
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::axes(Val::Px(64.0), Val::Px(space::XXL)),
                row_gap: Val::Px(space::MD),
                border: UiRect::all(Val::Px(2.0)),
                max_width: Val::Px(900.0),
                ..default()
            }))
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
                if let Some(notice) = &saves.notice {
                    card.spawn(theme::text(notice.clone(), font::CAPTION, palette::BROWN));
                }
                card.spawn(Node {
                    height: Val::Px(space::MD),
                    ..default()
                });

                card.spawn((
                    Panel::Main,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::MD),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    if has_save {
                        spawn_button(panel, "Continue", ButtonVariant::Primary, MenuAction::Play);
                        spawn_button(
                            panel,
                            "New Game",
                            ButtonVariant::Secondary,
                            MenuAction::AskNewGame,
                        );
                    } else {
                        spawn_button(panel, "Begin", ButtonVariant::Primary, MenuAction::Play);
                    }
                    spawn_button(
                        panel,
                        "Settings",
                        ButtonVariant::Secondary,
                        MenuAction::Settings,
                    );
                    spawn_button(panel, "Quit", ButtonVariant::Secondary, MenuAction::Quit);
                });

                card.spawn((
                    Panel::Confirm,
                    Node {
                        display: Display::None,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Center,
                        row_gap: Val::Px(space::MD),
                        ..default()
                    },
                ))
                .with_children(|panel| {
                    panel.spawn(theme::text("Start over?", font::TITLE, palette::INK));
                    panel.spawn(theme::text(
                        "Your saved meadow, basket and progress will be replaced.",
                        font::BODY,
                        palette::INK_SOFT,
                    ));
                    spawn_button(
                        panel,
                        "Start over",
                        ButtonVariant::Primary,
                        MenuAction::ConfirmNewGame,
                    );
                    spawn_button(
                        panel,
                        "Keep my game",
                        ButtonVariant::Secondary,
                        MenuAction::CancelNewGame,
                    );
                });
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

fn show_panel(panels: &mut Query<(&Panel, &mut Node)>, which: Panel) {
    for (panel, mut node) in panels.iter_mut() {
        node.display = if *panel == which {
            Display::Flex
        } else {
            Display::None
        };
    }
}

fn handle_buttons(
    mut commands: Commands,
    buttons: Query<(&Interaction, &MenuAction), Changed<Interaction>>,
    mut panels: Query<(&Panel, &mut Node)>,
    content: Res<Content>,
    session: Option<Res<Session>>,
    mut saves: ResMut<Saves>,
    mut next_state: ResMut<NextState<AppState>>,
    mut sfx: EventWriter<PlaySfx>,
    mut exit: EventWriter<AppExit>,
    mut screen: ResMut<SettingsScreen>,
) {
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed || screen.open {
            continue;
        }
        sfx.write(PlaySfx::new(Sfx::Click));
        match action {
            MenuAction::Play => next_state.set(AppState::InGame),
            MenuAction::AskNewGame => show_panel(&mut panels, Panel::Confirm),
            MenuAction::CancelNewGame => show_panel(&mut panels, Panel::Main),
            MenuAction::ConfirmNewGame => {
                if let Some(session) = session.as_deref() {
                    start_new_game(&mut commands, &content, session, &mut saves);
                }
                next_state.set(AppState::InGame);
            }
            MenuAction::Settings => screen.show(),
            MenuAction::Quit => {
                exit.write(AppExit::Success);
            }
        }
    }
}

/// Enter continues (or begins) — never starts over by accident.
fn handle_keys(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    screen: Res<SettingsScreen>,
    mut next_state: ResMut<NextState<AppState>>,
) {
    // Keys belong to the settings screen while it is open (or just closed).
    if screen.open || screen.is_changed() {
        return;
    }
    if Bindings::just_pressed(&keys, &bindings.confirm) {
        next_state.set(AppState::InGame);
    }
}
