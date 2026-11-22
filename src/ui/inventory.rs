//! The basket (inventory) screen: every species with its count, rarity and
//! collection status, sorting and filtering, and selling — plus the Tool Shop
//! and Upgrades tabs when opened at the workshop's tool shelf. Keys 1/2/3
//! switch tabs.
//!
//! Selling goes through the session, which enforces the keepsake reserve:
//! flowers still needed for a required bouquet are shown as "kept" and
//! can't be sold.

use bevy::prelude::*;
use bloom_core::data::Rarity;

use super::effects::change_lines;
use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, PlayState, Session};
use crate::input::Bindings;
use crate::interaction::OpenScreen;
use crate::rendering::color;
use bloom_core::game::{ToolLock, ToolStatus, UpgradeLock, UpgradeStatus};
use bloom_core::world::InteractableKind;

pub struct InventoryPlugin;

impl Plugin for InventoryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<View>()
            .add_systems(OnEnter(PlayState::Inventory), spawn_screen)
            .add_systems(
                Update,
                (
                    toggle.run_if(in_state(AppState::InGame)),
                    (tab_keys, handle_buttons, rebuild_grid)
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Tab {
    #[default]
    Basket,
    Shop,
    Upgrades,
}

/// Remembered between openings.
#[derive(Resource, Default)]
struct View {
    tab: Tab,
    /// Opened at the tool shelf: the Tool Shop tab is available.
    at_shelf: bool,
    sort: Sort,
    filter: Filter,
    /// The grid must be rebuilt (contents, sort or filter changed).
    stale: bool,
    /// Keyboard-selected tile in the shop or upgrades.
    selected: usize,
}

#[derive(Component, Clone, Copy)]
enum Action {
    Sort(Sort),
    Filter(Filter),
    SellOne(usize),
    SellAll(usize),
    SellEverything,
    Close,
    Tab(Tab),
    Buy(usize),
    BuyUpgrade(usize),
}

/// Parts of the screen only shown on the Basket tab.
#[derive(Component)]
struct BasketOnly;

#[derive(Component)]
struct Title;

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
            PlayState::Workshop
            | PlayState::Skills
            | PlayState::Journal
            | PlayState::Map
            | PlayState::Collection => next.set(PlayState::Playing),
            PlayState::Paused | PlayState::Finale => {}
        }
    }
}

fn spawn_screen(
    mut commands: Commands,
    mut view: ResMut<View>,
    mut open: ResMut<OpenScreen>,
    bindings: Res<Bindings>,
) {
    let close = format!("Close ({})", bindings.label(crate::input::Action::Basket));
    view.stale = true;
    // The shop lives at the tool shelf in the workshop; Tab (or the storage
    // chest) opens the basket.
    view.at_shelf = open.0.take() == Some(InteractableKind::ToolShelf);
    view.tab = if view.at_shelf {
        Tab::Shop
    } else {
        Tab::Basket
    };
    let at_shelf = view.at_shelf;
    commands
        .spawn((
            DespawnOnExit(PlayState::Inventory),
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
                // Four 258 px tiles per row (keeps the upgrades on one screen).
                width: Val::Px(1160.0),
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
                    header.spawn((
                        Title,
                        theme::text("Your Basket", font::HEADING, palette::INK),
                    ));
                    header.spawn((Summary, theme::text("", font::BODY, palette::INK_SOFT)));
                });

                card.spawn(Node {
                    column_gap: Val::Px(space::SM),
                    ..default()
                })
                .with_children(|tabs| {
                    spawn_button(
                        tabs,
                        "1  Basket",
                        ButtonVariant::Compact,
                        Action::Tab(Tab::Basket),
                    );
                    if at_shelf {
                        spawn_button(
                            tabs,
                            "2  Tool shop",
                            ButtonVariant::Compact,
                            Action::Tab(Tab::Shop),
                        );
                        spawn_button(
                            tabs,
                            "3  Upgrades",
                            ButtonVariant::Compact,
                            Action::Tab(Tab::Upgrades),
                        );
                    }
                });

                card.spawn((
                    BasketOnly,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::MD),
                        ..default()
                    },
                ))
                .with_children(|card| {
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
                });

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
                    footer.spawn((
                        BasketOnly,
                        theme::text(
                            "Every flower sells for the same price, whatever its rarity.",
                            font::CAPTION,
                            palette::INK_SOFT,
                        ),
                    ));
                    footer
                        .spawn(Node {
                            column_gap: Val::Px(space::SM),
                            ..default()
                        })
                        .with_children(|buttons| {
                            buttons
                                .spawn((BasketOnly, Node::default()))
                                .with_children(|b| {
                                    spawn_button(
                                        b,
                                        "Sell everything",
                                        ButtonVariant::Compact,
                                        Action::SellEverything,
                                    );
                                });
                            spawn_button(buttons, &close, ButtonVariant::Compact, Action::Close);
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

/// Keys 1/2/3 switch tabs; in the shop and upgrades, arrows choose a tile
/// and Enter buys it.
fn tab_keys(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    session: Option<ResMut<Session>>,
    mut view: ResMut<View>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let [basket, shop, upgrades, _] = bindings.tool_slots;
    let tab = if keys.just_pressed(basket) {
        Some(Tab::Basket)
    } else if keys.just_pressed(shop) && view.at_shelf {
        Some(Tab::Shop)
    } else if keys.just_pressed(upgrades) && view.at_shelf {
        Some(Tab::Upgrades)
    } else {
        None
    };
    if let Some(tab) = tab.filter(|&t| t != view.tab) {
        view.tab = tab;
        view.selected = 0;
        view.stale = true;
    }
    let Some(mut session) = session else { return };
    let tiles = match view.tab {
        Tab::Basket => return,
        Tab::Shop => session.data().tools.len(),
        Tab::Upgrades => session.data().upgrades.len(),
    };
    let before = view.selected;
    let step = |d: isize, v: &mut View| {
        v.selected = (v.selected as isize + d).clamp(0, tiles as isize - 1) as usize;
    };
    if keys.just_pressed(KeyCode::ArrowRight) {
        step(1, &mut view);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) {
        step(-1, &mut view);
    }
    if keys.just_pressed(KeyCode::ArrowDown) {
        step(TILES_PER_ROW, &mut view);
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        step(-TILES_PER_ROW, &mut view);
    }
    if view.selected != before {
        view.stale = true;
    }
    if Bindings::just_pressed(&keys, &bindings.confirm) {
        let bought = match view.tab {
            Tab::Shop => session.buy_tool(view.selected).is_ok(),
            Tab::Upgrades => session.buy_upgrade(view.selected).is_ok(),
            Tab::Basket => false,
        };
        if !bought {
            sfx.write(PlaySfx::new(Sfx::Deny));
        }
        view.stale = true;
    }
}

/// Shop and upgrade tiles per row (for Up/Down).
const TILES_PER_ROW: isize = 4;

fn handle_buttons(
    buttons: Query<(&Interaction, &Action), Changed<Interaction>>,
    mut view: ResMut<View>,
    session: Option<ResMut<Session>>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: MessageWriter<PlaySfx>,
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
            Action::Tab(tab) => {
                if view.tab != tab {
                    view.selected = 0;
                }
                view.tab = tab;
                sfx.write(PlaySfx::new(Sfx::Click));
            }
            Action::Buy(tool) => {
                view.selected = tool;
                // Success sound and toast come from the ToolBought event.
                if session.buy_tool(tool).is_err() {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Action::BuyUpgrade(upgrade) => {
                view.selected = upgrade;
                if session.buy_upgrade(upgrade).is_err() {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
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
    mut summary: Query<&mut Text, (With<Summary>, Without<Title>)>,
    mut title: Query<&mut Text, (With<Title>, Without<Summary>)>,
    mut basket_only: Query<&mut Node, With<BasketOnly>>,
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
        t.0 = match view.tab {
            Tab::Upgrades => format!(
                "{} coins   {} free for extras",
                session.state().coins,
                session.optional_spending_limit()
            ),
            _ => format!("{held} flowers   {} coins", session.state().coins),
        };
    }
    for mut t in &mut title {
        t.0 = match view.tab {
            Tab::Basket => "Your Basket".into(),
            Tab::Shop => "Tool Shop".into(),
            Tab::Upgrades => "Upgrades".into(),
        };
    }
    for mut node in &mut basket_only {
        node.display = if view.tab == Tab::Basket {
            Display::Flex
        } else {
            Display::None
        };
    }
    match view.tab {
        Tab::Shop => return shop_tiles(&mut commands, grid, &session, view.selected),
        Tab::Upgrades => return upgrade_tiles(&mut commands, grid, &session, view.selected),
        Tab::Basket => {}
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
                    border_radius: BorderRadius::all(Val::Px(radius::MD)),
                    ..default()
                },
                BackgroundColor(if count > 0 {
                    palette::PARCHMENT
                } else {
                    palette::CREAM
                }),
                BorderColor::all(palette::BEIGE),
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
                        border_radius: BorderRadius::all(Val::Px(11.0)),
                        ..default()
                    },
                    BackgroundColor(swatch),
                    BorderColor::all(if known {
                        color(species.center_color)
                    } else {
                        palette::BEIGE_DEEP
                    }),
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

/// One tile per tool: what it does, its price and level, and Buy / Owned /
/// what is still missing.
fn shop_tiles(commands: &mut Commands, grid: Entity, session: &Session, selected: usize) {
    let data = session.data().clone();
    commands.entity(grid).with_children(|grid| {
        for (i, tool) in data.tools.iter().enumerate() {
            let status = session.tool_status(i);
            let owned = status == ToolStatus::Owned;
            grid.spawn((
                Node {
                    width: Val::Px(258.0),
                    min_height: Val::Px(128.0),
                    padding: UiRect::all(Val::Px(space::SM)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(if i == selected { 2.5 } else { 1.5 })),
                    border_radius: BorderRadius::all(Val::Px(radius::MD)),
                    ..default()
                },
                BackgroundColor(if owned {
                    palette::PASTEL_GREEN
                } else {
                    palette::PARCHMENT
                }),
                BorderColor::all(if i == selected {
                    palette::INK
                } else {
                    palette::BEIGE
                }),
            ))
            .with_children(|tile| {
                tile.spawn(Node {
                    column_gap: Val::Px(space::SM),
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn((
                        Node {
                            width: Val::Px(16.0),
                            height: Val::Px(16.0),
                            border_radius: BorderRadius::all(Val::Px(4.0)),
                            ..default()
                        },
                        BackgroundColor(color(tool.icon_color)),
                    ));
                    row.spawn(theme::text(tool.name.clone(), font::BODY, palette::INK));
                });
                tile.spawn(theme::text(
                    tool.description.clone(),
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
                tile.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(theme::text(
                        format!(
                            "Level {}   {} coins",
                            tool.unlock_level,
                            session.tool_price(i)
                        ),
                        font::CAPTION,
                        palette::INK,
                    ));
                    match &status {
                        ToolStatus::Owned => {
                            row.spawn(theme::text("Owned", font::CAPTION, palette::SAGE_DEEP));
                        }
                        ToolStatus::Buyable => {
                            spawn_button(row, "Buy", ButtonVariant::Small, Action::Buy(i))
                        }
                        ToolStatus::Locked(lock) => {
                            let why = match lock {
                                ToolLock::Level(l) => format!("Needs level {l}"),
                                ToolLock::NeedsTool(t) => {
                                    format!(
                                        "Needs {}",
                                        data.tool(t).map_or(t.as_str(), |d| d.name.as_str())
                                    )
                                }
                                ToolLock::Coins(_) => "Not enough coins".to_string(),
                            };
                            row.spawn(theme::text(why, font::CAPTION, palette::BROWN));
                        }
                    }
                });
            });
        }
        grid.spawn(Node {
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|note| {
            note.spawn(theme::text(
                "Arrows choose, Enter buys. Upgrades are on tab 3.",
                font::CAPTION,
                palette::INK_SOFT,
            ));
        });
    });
}

/// One tile per upgrade: its level, what the next level changes (before ->
/// after), the price, and Buy / Maxed / why not.
fn upgrade_tiles(commands: &mut Commands, grid: Entity, session: &Session, selected: usize) {
    let data = session.data().clone();
    commands.entity(grid).with_children(|grid| {
        for (i, u) in data.upgrades.iter().enumerate() {
            let status = session.upgrade_status(i);
            let level = session.upgrade_level(i);
            let maxed = status == UpgradeStatus::Maxed;
            let preview = (!maxed).then(|| session.preview_upgrade(i));
            grid.spawn((
                Node {
                    width: Val::Px(258.0),
                    min_height: Val::Px(120.0),
                    padding: UiRect::all(Val::Px(space::SM)),
                    flex_direction: FlexDirection::Column,
                    row_gap: Val::Px(4.0),
                    border: UiRect::all(Val::Px(if i == selected { 2.5 } else { 1.5 })),
                    border_radius: BorderRadius::all(Val::Px(radius::MD)),
                    ..default()
                },
                BackgroundColor(if maxed {
                    palette::PASTEL_GREEN
                } else {
                    palette::PARCHMENT
                }),
                BorderColor::all(if i == selected {
                    palette::INK
                } else {
                    palette::BEIGE
                }),
            ))
            .with_children(|tile| {
                tile.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    ..default()
                })
                .with_children(|row| {
                    row.spawn(theme::text(u.name.clone(), font::BODY, palette::INK));
                    row.spawn(theme::text(
                        format!("{level} / {}", u.max_level),
                        font::CAPTION,
                        palette::INK_SOFT,
                    ));
                });
                tile.spawn(theme::text(
                    u.description.clone(),
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
                for line in change_lines(session, preview.as_ref(), &u.effects) {
                    tile.spawn(theme::text(line, font::CAPTION, palette::SAGE_DEEP));
                }
                tile.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    margin: UiRect::top(Val::Auto),
                    ..default()
                })
                .with_children(|row| {
                    let price = session
                        .upgrade_price(i)
                        .map_or(String::new(), |p| format!("{p} coins"));
                    row.spawn(theme::text(price, font::CAPTION, palette::INK));
                    match &status {
                        UpgradeStatus::Maxed => {
                            row.spawn(theme::text("Maxed", font::CAPTION, palette::SAGE_DEEP));
                        }
                        UpgradeStatus::Buyable { .. } => spawn_button(
                            row,
                            "Buy",
                            ButtonVariant::Small,
                            Action::BuyUpgrade(i),
                        ),
                        UpgradeStatus::Locked(lock) => {
                            let why = match lock {
                                UpgradeLock::NeedsTool(t) => format!(
                                    "Needs {}",
                                    data.tool(t).map_or(t.as_str(), |d| d.name.as_str())
                                ),
                                UpgradeLock::Coins { .. } => "Not enough coins".into(),
                                UpgradeLock::SavingForTools { .. } => "Save for tools first".into(),
                            };
                            row.spawn(theme::text(why, font::CAPTION, palette::BROWN));
                        }
                    }
                });
            });
        }
        grid.spawn(Node {
            width: Val::Percent(100.0),
            ..default()
        })
        .with_children(|note| {
            note.spawn(theme::text(
                "Coins you'll need for the tools you don't have yet are kept safe: extras can only use the rest. Arrows choose, Enter buys.",
                font::CAPTION,
                palette::INK_SOFT,
            ));
        });
    });
}
