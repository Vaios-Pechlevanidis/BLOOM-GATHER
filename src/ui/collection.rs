//! The herbarium (C): every species as a card (its colours, rarity, how
//! many you picked, where it grows, its description once discovered), and
//! the bouquets you have bound. Undiscovered species only hint at where
//! they grow. Game time is paused while it is open.

use bevy::prelude::*;
use bloom_core::data::FlowerSpecies;
use bloom_core::Game;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, PlayState, Session};
use crate::input::Bindings;
use crate::rendering::color;

pub struct CollectionPlugin;

impl Plugin for CollectionPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(PlayState::Collection), spawn_screen)
            .add_systems(
                Update,
                (
                    toggle.run_if(in_state(AppState::InGame)),
                    close_button.run_if(in_state(PlayState::Collection)),
                ),
            );
    }
}

#[derive(Component)]
struct Close;

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if Bindings::just_pressed(&keys, &bindings.collection) {
        match state.get() {
            PlayState::Playing => next.set(PlayState::Collection),
            PlayState::Collection => next.set(PlayState::Playing),
            _ => {}
        }
    }
}

fn close_button(
    pressed: Query<&Interaction, (Changed<Interaction>, With<Close>)>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if pressed.iter().any(|i| *i == Interaction::Pressed) {
        sfx.write(PlaySfx::new(Sfx::Click));
        next.set(PlayState::Playing);
    }
}

/// A flower drawn as a petal ring around its centre colour (greyed out and
/// marked "?" until discovered).
pub fn swatch(parent: &mut ChildSpawnerCommands, species: &FlowerSpecies, known: bool, size: f32) {
    let (petal, center) = if known {
        (color(species.petal_color), color(species.center_color))
    } else {
        (palette::BEIGE_DEEP, palette::BEIGE)
    };
    parent
        .spawn((
            Node {
                width: Val::Px(size),
                height: Val::Px(size),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(Val::Px(1.5)),
                border_radius: BorderRadius::all(Val::Px(size)),
                ..default()
            },
            BackgroundColor(petal),
            BorderColor::all(palette::INK_SOFT.with_alpha(0.35)),
        ))
        .with_children(|ring| {
            if known {
                ring.spawn((
                    Node {
                        width: Val::Px(size * 0.38),
                        height: Val::Px(size * 0.38),
                        border_radius: BorderRadius::all(Val::Px(size)),
                        ..default()
                    },
                    BackgroundColor(center),
                ));
            } else {
                ring.spawn(theme::text("?", font::BODY, palette::INK_SOFT));
            }
        });
}

/// Where a species grows: region names, or a hint for regions not open yet.
fn grows_in(game: &Game, species: &FlowerSpecies) -> String {
    let data = game.data();
    let mut open = Vec::new();
    let mut later = 0;
    for (r, region) in data.regions.iter().enumerate() {
        if region.species_count(&species.id) == 0 {
            continue;
        }
        if game.is_region_unlocked(r) {
            open.push(region.name.as_str());
        } else {
            later += 1;
        }
    }
    match (open.is_empty(), later) {
        (false, 0) => format!("Grows in {}", open.join(", ")),
        (false, _) => format!("Grows in {} and further along the valley", open.join(", ")),
        (true, _) => "Grows further along the valley".to_string(),
    }
}

fn spawn_screen(mut commands: Commands, session: Res<Session>, bindings: Res<Bindings>) {
    let game = &session.game;
    let data = game.data();
    let found = game.state().discovered.len();
    commands
        .spawn((
            DespawnOnExit(PlayState::Collection),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::SCRIM),
            GlobalZIndex(58),
        ))
        .with_children(|root| {
            root.spawn(theme::card(Node {
                width: Val::Px(1160.0),
                max_width: Val::Percent(96.0),
                flex_direction: FlexDirection::Column,
                padding: UiRect::all(Val::Px(space::XL)),
                row_gap: Val::Px(space::MD),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }))
            .with_children(|card| {
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|h| {
                    h.spawn(theme::text("Herbarium", font::HEADING, palette::INK));
                    h.spawn(theme::text(
                        format!("{found} of {} species discovered", data.species.len()),
                        font::BODY,
                        palette::INK_SOFT,
                    ));
                });
                card.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    column_gap: Val::Px(space::SM),
                    row_gap: Val::Px(space::SM),
                    ..default()
                })
                .with_children(|grid| {
                    for (i, sp) in data.species.iter().enumerate() {
                        species_card(grid, game, i, sp);
                    }
                });
                // Bouquets bound so far.
                let bound: Vec<String> = data
                    .recipes
                    .iter()
                    .filter_map(|r| {
                        let n = game.state().crafted.get(&r.id).copied().unwrap_or(0);
                        (n > 0).then(|| format!("{} x{n}", r.name))
                    })
                    .collect();
                card.spawn(theme::text(
                    if bound.is_empty() {
                        "Bouquets bound: none yet".to_string()
                    } else {
                        format!("Bouquets bound: {}", bound.join(",  "))
                    },
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
                card.spawn(Node {
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                })
                .with_children(|f| {
                    spawn_button(
                        f,
                        &format!(
                            "Close ({})",
                            bindings.label(crate::input::Action::Collection)
                        ),
                        ButtonVariant::Compact,
                        Close,
                    );
                });
            });
        });
}

fn species_card(grid: &mut ChildSpawnerCommands, game: &Game, idx: usize, sp: &FlowerSpecies) {
    let known = game.is_discovered(idx);
    let picked = game
        .state()
        .harvested_per_species
        .get(&sp.id)
        .copied()
        .unwrap_or(0);
    let total: u32 = game
        .data()
        .regions
        .iter()
        .map(|r| r.species_count(&sp.id))
        .sum();
    grid.spawn((
        Node {
            width: Val::Px(174.0),
            min_height: Val::Px(150.0),
            flex_direction: FlexDirection::Column,
            padding: UiRect::all(Val::Px(space::SM)),
            row_gap: Val::Px(3.0),
            border: UiRect::all(Val::Px(1.5)),
            border_radius: BorderRadius::all(Val::Px(radius::MD)),
            ..default()
        },
        BackgroundColor(if known {
            palette::CREAM
        } else {
            palette::PARCHMENT
        }),
        BorderColor::all(palette::BEIGE_DEEP),
    ))
    .with_children(|c| {
        c.spawn(Node {
            column_gap: Val::Px(space::SM),
            align_items: AlignItems::Center,
            ..default()
        })
        .with_children(|top| {
            swatch(top, sp, known, 28.0);
            top.spawn(Node {
                flex_direction: FlexDirection::Column,
                ..default()
            })
            .with_children(|names| {
                names.spawn(theme::text(
                    if known {
                        sp.name.clone()
                    } else {
                        "Undiscovered".into()
                    },
                    font::BODY,
                    if known {
                        palette::INK
                    } else {
                        palette::INK_SOFT
                    },
                ));
                names.spawn(theme::text(
                    sp.rarity.label(),
                    font::CAPTION,
                    palette::SAGE_DEEP,
                ));
            });
        });
        if known {
            c.spawn(theme::text(
                format!("Picked {picked} of {total}"),
                font::CAPTION,
                palette::INK,
            ));
            c.spawn(theme::text(
                sp.description.clone(),
                font::CAPTION,
                palette::INK_SOFT,
            ));
        } else {
            c.spawn(theme::text(
                grows_in(game, sp),
                font::CAPTION,
                palette::INK_SOFT,
            ));
        }
    });
}
