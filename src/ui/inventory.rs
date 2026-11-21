//! The basket (inventory) screen: every species with its count, rarity and
//! collection status, sorting and filtering, and selling.
//!
//! Selling goes through the session, which enforces the keepsake reserve:
//! flowers still needed for a required bouquet are shown as "kept" and
//! can't be sold.

use bevy::prelude::*;
use bloom_core::data::Rarity;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, PlayState, Session};
use crate::input::Bindings;
use crate::rendering::color;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<View>()
            .add_systems(OnEnter(PlayState::Inventory), spawn_screen)
            .add_systems(
                Update,
                (
                    toggle.run_if(in_state(AppState::InGame)),
                    (handle_buttons, rebuild_grid)
                        .chain()
                        .run_if(in_state(PlayState::Inventory)),
                ),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Sort {
    #[default]
    Name,
    Count,
    Rarity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Filter {
    #[default]
    All,
    InBasket,
    Rarity(Rarity),
}

/// Remembered between openings.
#[derive(Resource, Default)]
struct View {
    sort: Sort,
    filter: Filter,
    /// The grid must be rebuilt (contents, sort or filter changed).
    stale: bool,
}

#[derive(Component, Clone, Copy)]
enum Action {
    Sort(Sort),
    Filter(Filter),
    SellOne(usize),
    SellAll(usize),
    SellEverything,
    Close,
}

#[derive(Component)]
struct Grid;

#[derive(Component)]
struct Summary;

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if Bindings::just_pressed(&keys, &bindings.inventory) {
        match state.get() {
            PlayState::Playing => next.set(PlayState::Inventory),
            PlayState::Inventory => next.set(PlayState::Playing),
            PlayState::Paused => {}
        }
    }
}

fn spawn_screen(mut commands: Commands, mut view: ResMut<View>) {
    view.stale = true;
    commands
        .spawn((
            StateScoped(PlayState::Inventory),
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
                width: Val::Px(860.0),
                max_width: Val::Percent(94.0),
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
                .with_children(|header| {
                    header.spawn(theme::text("Your Basket", font::HEADING, palette::INK));
                    header.spawn((Summary, theme::text("", font::BODY, palette::INK_SOFT)));
                });

                chip_row(
                    card,
                    "Show",
                    &[
                        ("All", Action::Filter(Filter::All)),
                        ("In basket", Action::Filter(Filter::InBasket)),
                        ("Common", Action::Filter(Filter::Rarity(Rarity::Common))),
                        ("Uncommon", Action::Filter(Filter::Rarity(Rarity::Uncommon))),
                        ("Rare", Action::Filter(Filter::Rarity(Rarity::Rare))),
                        (
                            "Very rare",
                            Action::Filter(Filter::Rarity(Rarity::VeryRare)),
                        ),
                    ],
                );
                chip_row(
                    card,
                    "Sort",
                    &[
                        ("Name", Action::Sort(Sort::Name)),
                        ("Count", Action::Sort(Sort::Count)),
                        ("Rarity", Action::Sort(Sort::Rarity)),
                    ],
                );

                card.spawn((
                    Grid,
                    Node {
                        flex_wrap: FlexWrap::Wrap,
                        column_gap: Val::Px(space::SM),
                        row_gap: Val::Px(space::SM),
                        min_height: Val::Px(120.0),
                        ..default()
                    },
                ));

                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Px(space::SM)),
                    ..default()
                })
                .with_children(|footer| {
                    footer.spawn(theme::text(
                        "Every flower sells for the same price, whatever its rarity.",
                        font::CAPTION,
                        palette::INK_SOFT,
                    ));
                    footer
                        .spawn(Node {
                            column_gap: Val::Px(space::SM),
                            ..default()
                        })
                        .with_children(|buttons| {
                            spawn_button(
                                buttons,
                                "Sell everything",
                                ButtonVariant::Compact,
                                Action::SellEverything,
                            );
                            spawn_button(
                                buttons,
                                "Close (Tab)",
                                ButtonVariant::Compact,
                                Action::Close,
                            );
                        });
                });
            });
        });
}

fn chip_row(parent: &mut ChildSpawnerCommands, label: &str, chips: &[(&str, Action)]) {
    parent
        .spawn(Node {
            align_items: AlignItems::Center,
            column_gap: Val::Px(space::SM),
            ..default()
        })
        .with_children(|row| {
            row.spawn((
                Node {
                    width: Val::Px(48.0),
                    ..default()
                },
                theme::text(label, font::CAPTION, palette::INK_SOFT),
            ));
            for (text, action) in chips {
                spawn_button(row, text, ButtonVariant::Small, *action);
            }
        });
}

fn handle_buttons(
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut view: ResMut<View>,
    session: Option<ResMut<Session>>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let Some(mut session) = session else { return };
    for (interaction, action) in &buttons {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *action {
            Action::Sort(sort) => {
                view.sort = sort;
                sfx.write(PlaySfx::new(Sfx::Click));
            }
            Action::Filter(filter) => {
                view.filter = filter;
                sfx.write(PlaySfx::new(Sfx::Click));
            }
            Action::SellOne(species) => {
                if session.sell(species, 1).is_err() {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Action::SellAll(species) => {
                let n = session.sellable(species);
                if n == 0 || session.sell(species, n).is_err() {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Action::SellEverything => {
                if session.sell_all().0 == 0 {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Action::Close => {
                next.set(PlayState::Playing);
                continue;
            }
        }
        view.stale = true;
    }
}

fn rebuild_grid(
    mut commands: Commands,
    mut view: ResMut<View>,
    session: Option<Res<Session>>,
    grid: Query<(Entity, Option<&Children>), With<Grid>>,
    mut summary: Query<&mut Text, With<Summary>>,
) {
    let (Some(session), Ok((grid, children))) = (session, grid.single()) else {
        return;
    };
    if !view.stale {
        return;
    }
    view.stale = false;
    if let Some(children) = children {
        for child in children.iter() {
            commands.entity(child).despawn();
        }
    }

    let data = session.data().clone();
    let held: u32 = (0..data.species.len())
        .map(|s| session.inventory_count(s))
        .sum();
    for mut t in &mut summary {
        t.0 = format!("{held} flowers   {} coins", session.state().coins);
    }

    let mut order: Vec<usize> = (0..data.species.len())
        .filter(|&s| match view.filter {
            Filter::All => true,
            Filter::InBasket => session.inventory_count(s) > 0,
            Filter::Rarity(r) => data.species[s].rarity == r,
        })
        .collect();
    match view.sort {
        Sort::Name => order.sort_by(|&a, &b| data.species[a].name.cmp(&data.species[b].name)),
        Sort::Count => order.sort_by_key(|&s| std::cmp::Reverse(session.inventory_count(s))),
        Sort::Rarity => {
            order.sort_by_key(|&s| (data.species[s].rarity, data.species[s].name.clone()))
        }
    }

    commands.entity(grid).with_children(|grid| {
        if order.is_empty() {
            grid.spawn(theme::text(
                "Nothing here yet. Go and pick some flowers!",
                font::BODY,
                palette::INK_SOFT,
            ));
        }
        for s in order {
            let species = &data.species[s];
            let known = session.is_discovered(s);
            let count = session.inventory_count(s);
            let kept = if count > 0 { session.reserved(s) } else { 0 };
            grid.spawn((
                Node {
                    width: Val::Px(258.0),
                    padding: UiRect::all(Val::Px(space::SM)),
                    column_gap: Val::Px(space::SM),
                    align_items: AlignItems::Center,
                    border: UiRect::all(Val::Px(1.5)),
                    ..default()
                },
                BackgroundColor(if count > 0 {
                    palette::PARCHMENT
                } else {
                    palette::CREAM
                }),
                BorderColor(palette::BEIGE),
                BorderRadius::all(Val::Px(radius::MD)),
            ))
            .with_children(|tile| {
                let swatch = if known {
                    color(species.petal_color)
                } else {
                    palette::BEIGE
                };
                tile.spawn((
                    Node {
                        width: Val::Px(22.0),
                        height: Val::Px(22.0),
                        border: UiRect::all(Val::Px(3.0)),
                        ..default()
                    },
                    BackgroundColor(swatch),
                    BorderColor(if known {
                        color(species.center_color)
                    } else {
                        palette::BEIGE_DEEP
                    }),
                    BorderRadius::all(Val::Px(11.0)),
                ));
                tile.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    flex_grow: 1.0,
                    ..default()
                })
                .with_children(|col| {
                    let name = if known {
                        species.name.clone()
                    } else {
                        "Undiscovered".to_string()
                    };
                    col.spawn(theme::text(
                        name,
                        font::BODY,
                        if known {
                            palette::INK
                        } else {
                            palette::INK_SOFT
                        },
                    ));
                    let mut detail = format!("{}   x{count}", species.rarity.label());
                    if kept > 0 {
                        detail.push_str(&format!("   ({kept} kept)"));
                    }
                    col.spawn(theme::text(detail, font::CAPTION, palette::INK_SOFT));
                });
                if count > kept {
                    tile.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(2.0),
                        ..default()
                    })
                    .with_children(|buttons| {
                        spawn_button(buttons, "Sell 1", ButtonVariant::Small, Action::SellOne(s));
                        spawn_button(
                            buttons,
                            "Sell all",
                            ButtonVariant::Small,
                            Action::SellAll(s),
                        );
                    });
                }
            });
        }
    });
}
