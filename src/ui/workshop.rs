//! The workbench screen (opened at the workshop's workbench or bouquet
//! display).
//!
//! * Recipes tab: every recipe (locked ones show what is still missing); the
//!   selected one shows its requirements, its price against the flowers'
//!   raw value, and exactly which flowers a batch would use. Choose a
//!   quantity, optionally keep species out of the plan, and bind.
//! * Bouquets tab: finished bouquets and selling.
//! * A progress bar shows the batch being bound; the game keeps running
//!   while this screen is open, so the batch finishes here.
//!
//! Keyboard: ↑/↓ recipe, ←/→ quantity, Enter bind, 1/2 tabs, Esc/Tab close.

use std::collections::BTreeSet;

use bevy::prelude::*;
use bloom_core::game::{GameError, GameEvent, RecipeLock};
use bloom_core::world::InteractableKind;
use bloom_core::RecipeStatus;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant, Clicked};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{EventsForwarded, GameEventMsg, PlayState, Session};
use crate::input::Bindings;
use crate::interaction::OpenScreen;
use crate::ui::Toast;

pub struct WorkshopPlugin;

impl Plugin for WorkshopPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Bench>()
            .add_systems(OnEnter(PlayState::Workshop), spawn_screen)
            .add_systems(
                Update,
                (
                    keys,
                    buttons,
                    watch_events.after(EventsForwarded),
                    rebuild,
                    update_job,
                )
                    .chain()
                    .run_if(in_state(PlayState::Workshop)),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum BenchTab {
    #[default]
    Recipes,
    Bouquets,
}

#[derive(Resource, Default)]
struct Bench {
    tab: BenchTab,
    selected: usize,
    quantity: u32,
    /// Species the player wants to keep out of the plan.
    keep: BTreeSet<usize>,
    stale: bool,
}

#[derive(Component, Clone, Copy)]
enum Act {
    Tab(BenchTab),
    Select(usize),
    Less,
    More,
    Max,
    Bind,
    ToggleKeep(usize),
    SellOne(usize),
    SellAll(usize),
    SellEverything,
    Close,
}

#[derive(Component)]
struct Body;

#[derive(Component)]
struct Summary;

/// The heading names the open tab.
#[derive(Component)]
struct Title;

/// A header text, and whether it is the heading (else the summary).
type HeaderText = (&'static mut Text, Has<Title>);
type HeaderFilter = Or<(With<Summary>, With<Title>)>;

#[derive(Component)]
struct JobText;

#[derive(Component)]
struct JobFill;

fn spawn_screen(mut commands: Commands, mut bench: ResMut<Bench>, mut open: ResMut<OpenScreen>) {
    bench.tab = if open.0.take() == Some(InteractableKind::BouquetDisplay) {
        BenchTab::Bouquets
    } else {
        BenchTab::Recipes
    };
    bench.quantity = bench.quantity.max(1);
    bench.stale = true;
    commands
        .spawn((
            DespawnOnExit(PlayState::Workshop),
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
                width: Val::Px(960.0),
                max_width: Val::Percent(95.0),
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
                    h.spawn((Title, theme::text("Workbench", font::HEADING, palette::INK)));
                    h.spawn((Summary, theme::text("", font::BODY, palette::INK_SOFT)));
                });
                card.spawn(Node {
                    column_gap: Val::Px(space::SM),
                    ..default()
                })
                .with_children(|tabs| {
                    spawn_button(
                        tabs,
                        "1  Recipes",
                        ButtonVariant::Compact,
                        Act::Tab(BenchTab::Recipes),
                    );
                    spawn_button(
                        tabs,
                        "2  Bouquets",
                        ButtonVariant::Compact,
                        Act::Tab(BenchTab::Bouquets),
                    );
                });
                card.spawn((
                    Body,
                    Node {
                        column_gap: Val::Px(space::LG),
                        min_height: Val::Px(360.0),
                        ..default()
                    },
                ));
                // Batch progress and close.
                card.spawn(Node {
                    justify_content: JustifyContent::SpaceBetween,
                    align_items: AlignItems::Center,
                    column_gap: Val::Px(space::LG),
                    ..default()
                })
                .with_children(|f| {
                    f.spawn(Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(4.0),
                        flex_grow: 1.0,
                        ..default()
                    })
                    .with_children(|col| {
                        col.spawn((JobText, theme::text("", font::CAPTION, palette::INK_SOFT)));
                        col.spawn((
                            Node {
                                width: Val::Percent(100.0),
                                height: Val::Px(8.0),
                                border_radius: BorderRadius::all(Val::Px(4.0)),
                                ..default()
                            },
                            BackgroundColor(palette::BEIGE),
                        ))
                        .with_children(|bar| {
                            bar.spawn((
                                JobFill,
                                Node {
                                    width: Val::Percent(0.0),
                                    height: Val::Percent(100.0),
                                    border_radius: BorderRadius::all(Val::Px(4.0)),
                                    ..default()
                                },
                                BackgroundColor(palette::SAGE),
                            ));
                        });
                    });
                    spawn_button(f, "Close (Esc)", ButtonVariant::Compact, Act::Close);
                });
            });
        });
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    session: Option<Res<Session>>,
    mut bench: ResMut<Bench>,
    mut acts: Local<Vec<Act>>,
) {
    let Some(session) = session else { return };
    let n = session.data().recipes.len();
    let [one, two, ..] = bindings.tool_slots;
    let before = (bench.tab, bench.selected, bench.quantity);
    if keys.just_pressed(one) {
        bench.tab = BenchTab::Recipes;
    }
    if keys.just_pressed(two) {
        bench.tab = BenchTab::Bouquets;
    }
    if bench.tab == BenchTab::Recipes {
        if keys.just_pressed(KeyCode::ArrowDown) {
            bench.selected = (bench.selected + 1) % n;
            bench.quantity = 1;
        }
        if keys.just_pressed(KeyCode::ArrowUp) {
            bench.selected = (bench.selected + n - 1) % n;
            bench.quantity = 1;
        }
        let cap = session.batch_capacity();
        if keys.just_pressed(KeyCode::ArrowRight) {
            bench.quantity = (bench.quantity + 1).min(cap);
        }
        if keys.just_pressed(KeyCode::ArrowLeft) {
            bench.quantity = bench.quantity.saturating_sub(1).max(1);
        }
    }
    if (bench.tab, bench.selected, bench.quantity) != before {
        bench.stale = true;
    }
    acts.clear();
}

/// Binding via Enter or the button.
fn bind(
    session: &mut Session,
    bench: &Bench,
    sfx: &mut MessageWriter<PlaySfx>,
    toasts: &mut MessageWriter<Toast>,
) {
    match session.craft(bench.selected, bench.quantity, &bench.keep) {
        Ok(_) => {}
        Err(err) => {
            sfx.write(PlaySfx::new(Sfx::Deny));
            let msg = match err {
                GameError::RecipeLocked(_) => "You haven't discovered that recipe yet".to_string(),
                other => capitalise(&other.to_string()),
            };
            toasts.write(Toast::info(msg));
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

#[allow(clippy::too_many_arguments)]
fn buttons(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    mut clicks: MessageReader<Clicked>,
    pressed: Query<&Act>,
    session: Option<ResMut<Session>>,
    mut bench: ResMut<Bench>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut toasts: MessageWriter<Toast>,
) {
    let Some(mut session) = session else { return };
    if bench.tab == BenchTab::Recipes && Bindings::just_pressed(&keys, &bindings.confirm) {
        bind(&mut session, &bench, &mut sfx, &mut toasts);
        bench.stale = true;
    }
    for act in clicks.read().filter_map(|c| pressed.get(c.0).ok()) {
        match *act {
            Act::Tab(tab) => bench.tab = tab,
            Act::Select(r) => {
                if bench.selected != r {
                    bench.selected = r;
                    bench.quantity = 1;
                }
            }
            Act::Less => bench.quantity = bench.quantity.saturating_sub(1).max(1),
            Act::More => bench.quantity = (bench.quantity + 1).min(session.batch_capacity()),
            Act::Max => bench.quantity = max_makeable(&session, &bench).max(1),
            Act::Bind => bind(&mut session, &bench, &mut sfx, &mut toasts),
            Act::ToggleKeep(s) => {
                if !bench.keep.remove(&s) {
                    bench.keep.insert(s);
                }
            }
            Act::SellOne(r) => {
                if session.sell_bouquets(r, 1).is_err() {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Act::SellAll(r) => {
                let n = session.bouquet_count(r);
                if session.sell_bouquets(r, n).is_err() {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Act::SellEverything => {
                let mut sold = 0;
                for r in 0..session.data().recipes.len() {
                    let n = session.bouquet_count(r);
                    if n > 0 && session.sell_bouquets(r, n).is_ok() {
                        sold += n;
                    }
                }
                if sold == 0 {
                    sfx.write(PlaySfx::new(Sfx::Deny));
                }
            }
            Act::Close => {
                next.set(PlayState::Playing);
                continue;
            }
        }
        if !matches!(
            act,
            Act::Bind | Act::SellOne(_) | Act::SellAll(_) | Act::SellEverything
        ) {
            sfx.write(PlaySfx::new(Sfx::Click));
        }
        bench.stale = true;
    }
}

/// Rebuild when the workbench finishes, recipes appear, or anything sells.
fn watch_events(mut events: MessageReader<GameEventMsg>, mut bench: ResMut<Bench>) {
    for GameEventMsg(e) in events.read() {
        if matches!(
            e,
            GameEvent::BouquetsReady { .. }
                | GameEvent::RecipeDiscovered { .. }
                | GameEvent::CraftStarted { .. }
                | GameEvent::BouquetsSold { .. }
        ) {
            bench.stale = true;
        }
    }
}

/// The largest batch of the selected recipe that can be bound right now.
fn max_makeable(session: &Session, bench: &Bench) -> u32 {
    let cap = session.batch_capacity();
    (1..=cap)
        .rev()
        .find(|&n| session.plan_craft(bench.selected, n, &bench.keep).is_ok())
        .unwrap_or(0)
}

fn requirement_text(session: &Session, recipe: usize) -> String {
    let data = session.data();
    let r = &data.recipes[recipe];
    let mut parts = vec![format!("{} flowers", r.flower_count)];
    for req in &r.required_species {
        let name = data
            .species(&req.species)
            .map_or(req.species.as_str(), |s| s.name.as_str());
        parts.push(format!("at least {} {}", req.count, name));
    }
    if r.min_distinct_species > 0 {
        parts.push(format!("{} different species", r.min_distinct_species));
    }
    if parts.len() == 1 {
        parts.push("any kind".into());
    }
    parts.join(", ")
}

fn lock_text(session: &Session, locks: &[RecipeLock]) -> String {
    let data = session.data();
    let items: Vec<String> = locks
        .iter()
        .map(|l| match l {
            RecipeLock::Tool(t) => data.tool(t).map_or(t.clone(), |d| d.name.clone()),
            RecipeLock::Species(s) => format!(
                "find {}",
                data.species(s).map_or(s.as_str(), |d| d.name.as_str())
            ),
            RecipeLock::Level(l) => format!("level {l}"),
        })
        .collect();
    format!("Still needed: {}", items.join(", "))
}

fn rebuild(
    mut commands: Commands,
    mut bench: ResMut<Bench>,
    session: Option<Res<Session>>,
    body: Query<(Entity, Option<&Children>), With<Body>>,
    mut texts: Query<HeaderText, HeaderFilter>,
) {
    let (Some(session), Ok((body, children))) = (session, body.single()) else {
        return;
    };
    if !bench.stale {
        return;
    }
    bench.stale = false;
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).despawn();
        }
    }
    let data = session.data().clone();
    let held: u32 = session.state().inventory.values().sum();
    for (mut t, title) in &mut texts {
        t.0 = if title {
            match bench.tab {
                BenchTab::Recipes => "Workbench - Recipes".into(),
                BenchTab::Bouquets => "Workbench - Bouquets".into(),
            }
        } else {
            format!(
                "{held} flowers   {} coins   binds up to {}",
                session.state().coins,
                session.batch_capacity()
            )
        };
    }
    let selected = bench.selected.min(data.recipes.len() - 1);

    commands.entity(body).with_children(|body| match bench.tab {
        BenchTab::Recipes => {
            // Recipe list.
            body.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(6.0),
                width: Val::Px(300.0),
                ..default()
            })
            .with_children(|list| {
                for (i, r) in data.recipes.iter().enumerate() {
                    let known = session.is_recipe_discovered(i);
                    let active = i == selected;
                    list.spawn((
                        super::widgets::Button,
                        bevy::picking::hover::Hovered::default(),
                        Act::Select(i),
                        Node {
                            padding: UiRect::axes(Val::Px(space::MD), Val::Px(6.0)),
                            justify_content: JustifyContent::SpaceBetween,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(radius::SM)),
                            ..default()
                        },
                        BackgroundColor(if active {
                            palette::SAGE
                        } else {
                            palette::PARCHMENT
                        }),
                        BorderColor::all(if active {
                            palette::SAGE_DARK
                        } else {
                            palette::BEIGE
                        }),
                    ))
                    .with_children(|row| {
                        let ink = if active {
                            palette::CREAM
                        } else if known {
                            palette::INK
                        } else {
                            palette::INK_SOFT
                        };
                        let name = if known {
                            r.name.clone()
                        } else {
                            format!("{} (locked)", r.name)
                        };
                        row.spawn(theme::text(name, font::CAPTION, ink));
                        let have = session.bouquet_count(i);
                        if have > 0 {
                            row.spawn(theme::text(format!("x{have}"), font::CAPTION, ink));
                        }
                    });
                }
            });

            // Details.
            let r = &data.recipes[selected];
            body.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::SM),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|d| {
                d.spawn(theme::text(r.name.clone(), font::TITLE, palette::INK));
                d.spawn(theme::text(
                    r.description.clone(),
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
                d.spawn(theme::text(
                    format!("Needs {}", requirement_text(&session, selected)),
                    font::BODY,
                    palette::INK,
                ));
                let raw = data.economy.raw_flower_value(r.flower_count as u64);
                d.spawn(theme::text(
                    format!(
                        "Sells for {} coins (its flowers alone: {raw})",
                        session.bouquet_price(selected)
                    ),
                    font::BODY,
                    palette::SAGE_DEEP,
                ));
                match session.recipe_status(selected) {
                    RecipeStatus::Locked(locks) => {
                        d.spawn(theme::text(
                            lock_text(&session, &locks),
                            font::BODY,
                            palette::BROWN,
                        ));
                    }
                    RecipeStatus::Discovered => {
                        let cap = session.batch_capacity();
                        d.spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::SM),
                            margin: UiRect::top(Val::Px(space::SM)),
                            ..default()
                        })
                        .with_children(|q| {
                            q.spawn(theme::text("Bouquets", font::BODY, palette::INK));
                            spawn_button(q, "-", ButtonVariant::Small, Act::Less);
                            q.spawn(theme::text(
                                format!("{}", bench.quantity),
                                font::TITLE,
                                palette::INK,
                            ));
                            spawn_button(q, "+", ButtonVariant::Small, Act::More);
                            spawn_button(q, "Max", ButtonVariant::Small, Act::Max);
                            q.spawn(theme::text(
                                format!("up to {cap} at a time"),
                                font::CAPTION,
                                palette::INK_SOFT,
                            ));
                        });
                        match session.plan_craft(selected, bench.quantity, &bench.keep) {
                            Ok(plan) => {
                                let uses: Vec<String> = plan
                                    .ingredients
                                    .iter()
                                    .map(|(s, n)| format!("{n} {}", data.species[*s].name))
                                    .collect();
                                d.spawn(theme::text(
                                    format!("Uses {}", uses.join(", ")),
                                    font::BODY,
                                    palette::INK,
                                ));
                                d.spawn(theme::text(
                                    format!(
                                        "Takes {:.1} s, sells for {} coins in all",
                                        plan.secs,
                                        session.bouquet_price(selected) * bench.quantity as u64
                                    ),
                                    font::CAPTION,
                                    palette::INK_SOFT,
                                ));
                            }
                            Err(err) => {
                                d.spawn(theme::text(
                                    capitalise(&err.to_string()),
                                    font::BODY,
                                    palette::BROWN,
                                ));
                            }
                        }
                        // Species the player can keep out of the plan.
                        let in_basket: Vec<usize> = (0..data.species.len())
                            .filter(|&s| session.inventory_count(s) > 0)
                            .collect();
                        if !in_basket.is_empty() {
                            d.spawn(theme::text(
                                "Keep out of this bouquet:",
                                font::CAPTION,
                                palette::INK_SOFT,
                            ));
                            d.spawn(Node {
                                flex_wrap: FlexWrap::Wrap,
                                column_gap: Val::Px(6.0),
                                row_gap: Val::Px(6.0),
                                ..default()
                            })
                            .with_children(|chips| {
                                for s in in_basket {
                                    let kept = bench.keep.contains(&s);
                                    let label = format!(
                                        "{}{} x{}",
                                        if kept { "kept: " } else { "" },
                                        data.species[s].name,
                                        session.inventory_count(s)
                                    );
                                    spawn_button(
                                        chips,
                                        &label,
                                        ButtonVariant::Small,
                                        Act::ToggleKeep(s),
                                    );
                                }
                            });
                        }
                        d.spawn(Node {
                            margin: UiRect::top(Val::Px(space::MD)),
                            ..default()
                        })
                        .with_children(|b| {
                            let label = if session.craft_job().is_some() {
                                "Workbench busy"
                            } else {
                                "Bind (Enter)"
                            };
                            spawn_button(b, label, ButtonVariant::Primary, Act::Bind);
                        });
                    }
                }
            });
        }
        BenchTab::Bouquets => {
            body.spawn(Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(space::SM),
                flex_grow: 1.0,
                ..default()
            })
            .with_children(|list| {
                let mut total = 0;
                for (i, r) in data.recipes.iter().enumerate() {
                    let n = session.bouquet_count(i);
                    if n == 0 {
                        continue;
                    }
                    total += session.bouquet_price(i) * n as u64;
                    list.spawn((
                        Node {
                            padding: UiRect::all(Val::Px(space::SM)),
                            justify_content: JustifyContent::SpaceBetween,
                            align_items: AlignItems::Center,
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(radius::MD)),
                            ..default()
                        },
                        BackgroundColor(palette::PARCHMENT),
                        BorderColor::all(palette::BEIGE),
                    ))
                    .with_children(|row| {
                        row.spawn(theme::text(
                            format!("{}  x{n}", r.name),
                            font::BODY,
                            palette::INK,
                        ));
                        row.spawn(Node {
                            align_items: AlignItems::Center,
                            column_gap: Val::Px(space::SM),
                            ..default()
                        })
                        .with_children(|right| {
                            right.spawn(theme::text(
                                format!("{} coins each", session.bouquet_price(i)),
                                font::CAPTION,
                                palette::INK_SOFT,
                            ));
                            spawn_button(right, "Sell 1", ButtonVariant::Small, Act::SellOne(i));
                            spawn_button(right, "Sell all", ButtonVariant::Small, Act::SellAll(i));
                        });
                    });
                }
                if total == 0 {
                    list.spawn(theme::text(
                        "No bouquets yet. Bind some on the Recipes tab.",
                        font::BODY,
                        palette::INK_SOFT,
                    ));
                } else {
                    list.spawn(Node {
                        margin: UiRect::top(Val::Px(space::SM)),
                        ..default()
                    })
                    .with_children(|f| {
                        spawn_button(
                            f,
                            &format!("Sell every bouquet ({total} coins)"),
                            ButtonVariant::Compact,
                            Act::SellEverything,
                        );
                    });
                }
            });
        }
    });
}

fn update_job(
    session: Option<Res<Session>>,
    mut text: Query<&mut Text, With<JobText>>,
    mut fill: Query<&mut Node, With<JobFill>>,
) {
    let Some(session) = session else { return };
    let (label, pct) = match session.craft_job() {
        Some(job) => {
            let name = session
                .data()
                .recipe(&job.recipe)
                .map_or(job.recipe.as_str(), |r| r.name.as_str());
            (
                format!("Binding {} {}...", job.count, name),
                job.progress() * 100.0,
            )
        }
        None => ("The workbench is free.".to_string(), 0.0),
    };
    for mut t in &mut text {
        if t.0 != label {
            t.0 = label.clone();
        }
    }
    for mut n in &mut fill {
        n.width = Val::Percent(pct);
    }
}
