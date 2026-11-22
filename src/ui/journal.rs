//! The journal (J): achievements with progress, lifetime statistics, and the
//! road to the ending (completion checklist plus the next milestones).
//!
//! Keys: 1/2/3 switch tabs, J or Esc closes. Game time is paused.

use bevy::prelude::*;
use bloom_core::data::AchievementCategory;
use bloom_core::game::GameEvent;
use bloom_core::ToolStatus;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant, Clicked};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, EventsForwarded, GameEventMsg, PlayState, Session};
use crate::input::Bindings;

pub struct JournalPlugin;

impl Plugin for JournalPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<JournalView>()
            .add_systems(OnEnter(PlayState::Journal), spawn_screen)
            .add_systems(
                Update,
                (
                    toggle.run_if(in_state(AppState::InGame)),
                    (keys, buttons, watch_events.after(EventsForwarded), rebuild)
                        .chain()
                        .run_if(in_state(PlayState::Journal)),
                ),
            );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Page {
    #[default]
    Achievements,
    Statistics,
    Progress,
}

#[derive(Resource, Default)]
struct JournalView {
    page: Page,
    stale: bool,
}

#[derive(Component, Clone, Copy)]
enum Act {
    Page(Page),
    /// Open the journey summary (after the ending).
    Summary,
    Close,
}

#[derive(Component)]
struct Body;

#[derive(Component)]
struct Summary;

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if Bindings::just_pressed(&keys, &bindings.journal) {
        match state.get() {
            PlayState::Playing => next.set(PlayState::Journal),
            PlayState::Journal => next.set(PlayState::Playing),
            _ => {}
        }
    }
}

fn spawn_screen(mut commands: Commands, mut view: ResMut<JournalView>, bindings: Res<Bindings>) {
    view.stale = true;
    commands
        .spawn((
            DespawnOnExit(PlayState::Journal),
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
                    h.spawn(theme::text("Journal", font::HEADING, palette::INK));
                    h.spawn((Summary, theme::text("", font::BODY, palette::INK_SOFT)));
                });
                card.spawn(Node {
                    column_gap: Val::Px(space::SM),
                    ..default()
                })
                .with_children(|tabs| {
                    spawn_button(
                        tabs,
                        "1  Achievements",
                        ButtonVariant::Compact,
                        Act::Page(Page::Achievements),
                    );
                    spawn_button(
                        tabs,
                        "2  Statistics",
                        ButtonVariant::Compact,
                        Act::Page(Page::Statistics),
                    );
                    spawn_button(
                        tabs,
                        "3  Progress",
                        ButtonVariant::Compact,
                        Act::Page(Page::Progress),
                    );
                });
                card.spawn((
                    Body,
                    Node {
                        column_gap: Val::Px(space::LG),
                        min_height: Val::Px(420.0),
                        ..default()
                    },
                ));
                card.spawn(Node {
                    justify_content: JustifyContent::FlexEnd,
                    ..default()
                })
                .with_children(|f| {
                    spawn_button(
                        f,
                        &format!("Close ({})", bindings.label(crate::input::Action::Journal)),
                        ButtonVariant::Compact,
                        Act::Close,
                    );
                });
            });
        });
}

fn keys(keys: Res<ButtonInput<KeyCode>>, bindings: Res<Bindings>, mut view: ResMut<JournalView>) {
    let [one, two, three, _] = bindings.tool_slots;
    let page = if keys.just_pressed(one) {
        Page::Achievements
    } else if keys.just_pressed(two) {
        Page::Statistics
    } else if keys.just_pressed(three) {
        Page::Progress
    } else {
        return;
    };
    if view.page != page {
        view.page = page;
        view.stale = true;
    }
}

fn buttons(
    mut clicks: MessageReader<Clicked>,
    pressed: Query<&Act>,
    mut view: ResMut<JournalView>,
    mut next: ResMut<NextState<PlayState>>,
    mut finale: ResMut<super::finale::FinaleRequest>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    for act in clicks.read().filter_map(|c| pressed.get(c.0).ok()) {
        sfx.write(PlaySfx::new(Sfx::Click));
        match *act {
            Act::Page(p) => {
                view.page = p;
                view.stale = true;
            }
            Act::Summary => {
                finale.summary_only = true;
                next.set(PlayState::Finale);
            }
            Act::Close => next.set(PlayState::Playing),
        }
    }
}

fn watch_events(mut events: MessageReader<GameEventMsg>, mut view: ResMut<JournalView>) {
    for GameEventMsg(e) in events.read() {
        if matches!(e, GameEvent::AchievementUnlocked { .. }) {
            view.stale = true;
        }
    }
}

fn rebuild(
    mut commands: Commands,
    mut view: ResMut<JournalView>,
    session: Option<Res<Session>>,
    body: Query<(Entity, Option<&Children>), With<Body>>,
    mut summary: Query<&mut Text, With<Summary>>,
) {
    let (Some(session), Ok((body, children))) = (session, body.single()) else {
        return;
    };
    if !view.stale {
        return;
    }
    view.stale = false;
    if let Some(children) = children {
        for c in children.iter() {
            commands.entity(c).despawn();
        }
    }
    let data = session.data();
    let unlocked = session.state().achievements.len();
    let page = match view.page {
        Page::Achievements => "Achievements",
        Page::Statistics => "Statistics",
        Page::Progress => "Progress",
    };
    for mut t in &mut summary {
        t.0 = format!(
            "{page}   ({unlocked} of {} achievements unlocked)",
            data.achievements.len()
        );
    }
    commands.entity(body).with_children(|body| match view.page {
        Page::Achievements => achievements(body, &session),
        Page::Statistics => statistics(body, &session),
        Page::Progress => progress(body, &session),
    });
}

fn column(body: &mut ChildSpawnerCommands, f: impl FnOnce(&mut ChildSpawnerCommands)) {
    body.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(6.0),
        flex_grow: 1.0,
        flex_basis: Val::Px(0.0),
        ..default()
    })
    .with_children(f);
}

fn heading(col: &mut ChildSpawnerCommands, text: &str) {
    col.spawn((
        Node {
            margin: UiRect::top(Val::Px(4.0)),
            ..default()
        },
        theme::text(text, font::BODY, palette::INK),
    ));
}

/// A thin progress bar.
fn bar(parent: &mut ChildSpawnerCommands, fraction: f32, done: bool) {
    parent
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Px(5.0),
                border_radius: BorderRadius::all(Val::Px(3.0)),
                ..default()
            },
            BackgroundColor(palette::BEIGE),
        ))
        .with_children(|b| {
            b.spawn((
                Node {
                    width: Val::Percent(fraction.clamp(0.0, 1.0) * 100.0),
                    height: Val::Percent(100.0),
                    border_radius: BorderRadius::all(Val::Px(3.0)),
                    ..default()
                },
                BackgroundColor(if done {
                    palette::SAGE_DARK
                } else {
                    palette::SAGE
                }),
            ));
        });
}

fn achievements(body: &mut ChildSpawnerCommands, session: &Session) {
    use AchievementCategory as C;
    let data = session.data();
    let groups: [&[C]; 3] = [
        &[C::Harvesting, C::Crafting],
        &[C::Exploration, C::Collection],
        &[C::Completion],
    ];
    for cats in groups {
        column(body, |col| {
            for &cat in cats {
                heading(col, cat.label());
                for (i, a) in data.achievements.iter().enumerate() {
                    if a.category != cat {
                        continue;
                    }
                    let done = session.is_achievement_unlocked(i);
                    let (now, target) = session.achievement_progress(i);
                    col.spawn((
                        Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(2.0),
                            padding: UiRect::axes(Val::Px(space::SM), Val::Px(4.0)),
                            border: UiRect::all(Val::Px(1.5)),
                            border_radius: BorderRadius::all(Val::Px(radius::SM)),
                            ..default()
                        },
                        BackgroundColor(if done {
                            palette::PASTEL_GREEN
                        } else {
                            palette::PARCHMENT
                        }),
                        BorderColor::all(palette::BEIGE),
                    ))
                    .with_children(|row| {
                        row.spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            ..default()
                        })
                        .with_children(|top| {
                            top.spawn(theme::text(
                                if done {
                                    format!("{}  (done)", a.name)
                                } else {
                                    a.name.clone()
                                },
                                font::CAPTION,
                                palette::INK,
                            ));
                            let reward = match (a.reward.coins, a.reward.skill_points) {
                                (0, 0) => String::new(),
                                (c, 0) => format!("{c} coins"),
                                (0, p) => format!("{p} skill pt"),
                                (c, p) => format!("{c} coins, {p} skill pt"),
                            };
                            top.spawn(theme::text(reward, font::CAPTION, palette::SAGE_DEEP));
                        });
                        // Description and count share a line; a thin bar below.
                        let counting = !done && target > 1;
                        row.spawn(Node {
                            justify_content: JustifyContent::SpaceBetween,
                            column_gap: Val::Px(space::SM),
                            ..default()
                        })
                        .with_children(|mid| {
                            mid.spawn(theme::text(
                                a.description.clone(),
                                font::CAPTION,
                                palette::INK_SOFT,
                            ));
                            if counting {
                                mid.spawn((
                                    Node {
                                        flex_shrink: 0.0,
                                        ..default()
                                    },
                                    theme::text(
                                        format!("{now} / {target}"),
                                        font::CAPTION,
                                        palette::INK_SOFT,
                                    ),
                                ));
                            }
                        });
                        if counting {
                            bar(row, now as f32 / target as f32, false);
                        }
                    });
                }
            }
        });
    }
}

fn stat_row(col: &mut ChildSpawnerCommands, label: &str, value: String) {
    col.spawn(Node {
        justify_content: JustifyContent::SpaceBetween,
        padding: UiRect::axes(Val::Px(space::SM), Val::Px(4.0)),
        border: UiRect::bottom(Val::Px(1.0)),
        ..default()
    })
    .insert(BorderColor::all(palette::BEIGE))
    .with_children(|r| {
        r.spawn(theme::text(label, font::BODY, palette::INK_SOFT));
        r.spawn(theme::text(value, font::BODY, palette::INK));
    });
}

fn statistics(body: &mut ChildSpawnerCommands, session: &Session) {
    let data = session.data();
    let s = session.state();
    let st = &s.stats;
    let secs = st.playtime_secs as u64;
    let played = format!("{} h {:02} min", secs / 3600, secs / 60 % 60);
    let walked = if st.distance_walked_m >= 1000.0 {
        format!("{:.2} km", st.distance_walked_m / 1000.0)
    } else {
        format!("{:.0} m", st.distance_walked_m)
    };
    let upgrade_levels: u32 = s.upgrades.values().sum();
    let upgrade_max: u32 = data.upgrades.iter().map(|u| u.max_level).sum();
    let skill_levels: u32 = s.skills.values().sum();
    let skill_max: u32 = data.skills.iter().map(|k| k.max_level).sum();
    let opened = (0..data.regions.len())
        .filter(|&r| session.is_region_unlocked(r))
        .count();
    column(body, |col| {
        heading(col, "The journey");
        stat_row(col, "Time played", played);
        stat_row(col, "Distance walked", walked);
        stat_row(
            col,
            "Level",
            format!("{}  ({} XP in all)", session.level(), s.xp),
        );
        stat_row(
            col,
            "Regions open / complete",
            format!(
                "{opened} / {}   {} / {}",
                data.regions.len(),
                s.completed_regions.len(),
                data.regions.len()
            ),
        );
        stat_row(
            col,
            "Regions visited",
            format!(
                "{} / {}",
                (0..data.regions.len())
                    .filter(|&r| session.has_visited(r))
                    .count(),
                data.regions.len()
            ),
        );
        stat_row(
            col,
            "Achievements",
            format!("{} / {}", s.achievements.len(), data.achievements.len()),
        );
        heading(col, "Flowers");
        stat_row(col, "Picked", st.flowers_harvested.to_string());
        stat_row(
            col,
            "Rare and very rare picked",
            session.rare_flowers_harvested().to_string(),
        );
        stat_row(col, "Sold one by one", st.flowers_sold.to_string());
        stat_row(
            col,
            "Species discovered",
            format!("{} / {}", s.discovered.len(), data.species.len()),
        );
    });
    column(body, |col| {
        heading(col, "Bouquets");
        stat_row(col, "Bound", st.bouquets_crafted.to_string());
        stat_row(col, "Sold", st.bouquets_sold.to_string());
        stat_row(
            col,
            "Recipes discovered",
            format!("{} / {}", s.discovered_recipes.len(), data.recipes.len()),
        );
        heading(col, "Coins");
        stat_row(col, "Earned", st.coins_earned.to_string());
        stat_row(
            col,
            "Spent on tools and upgrades",
            st.coins_spent.to_string(),
        );
        stat_row(col, "In your purse", s.coins.to_string());
        heading(col, "Growth");
        stat_row(
            col,
            "Tools owned",
            format!("{} / {}", s.tools.len(), data.tools.len()),
        );
        stat_row(
            col,
            "Upgrade levels",
            format!("{upgrade_levels} / {upgrade_max}"),
        );
        stat_row(
            col,
            "Skill levels",
            format!(
                "{skill_levels} / {skill_max}   ({} points earned)",
                session.skill_points_earned()
            ),
        );
    });
}

fn check_row(col: &mut ChildSpawnerCommands, label: &str, done: u64, total: u64) {
    let complete = done >= total;
    col.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(3.0),
        padding: UiRect::axes(Val::Px(space::SM), Val::Px(5.0)),
        ..default()
    })
    .with_children(|row| {
        row.spawn(Node {
            justify_content: JustifyContent::SpaceBetween,
            ..default()
        })
        .with_children(|top| {
            top.spawn(theme::text(
                label,
                font::BODY,
                if complete {
                    palette::SAGE_DEEP
                } else {
                    palette::INK
                },
            ));
            top.spawn(theme::text(
                if complete {
                    "done".to_string()
                } else {
                    format!("{done} / {total}")
                },
                font::BODY,
                palette::INK_SOFT,
            ));
        });
        bar(row, done as f32 / total.max(1) as f32, complete);
    });
}

fn progress(body: &mut ChildSpawnerCommands, session: &Session) {
    let data = session.data();
    let c = session.completion();
    column(body, |col| {
        heading(col, "The road to the ending");
        col.spawn(theme::text(
            "Bloom & Gather is complete when all six are done. Achievements are never needed.",
            font::CAPTION,
            palette::INK_SOFT,
        ));
        check_row(col, "Every flower picked", c.flowers.0, c.flowers.1);
        check_row(
            col,
            "Every region complete",
            c.regions.0 as u64,
            c.regions.1 as u64,
        );
        check_row(col, "Every tool owned", c.tools.0 as u64, c.tools.1 as u64);
        check_row(
            col,
            "Every species discovered",
            c.species.0 as u64,
            c.species.1 as u64,
        );
        check_row(
            col,
            "Every recipe discovered",
            c.recipes_discovered.0 as u64,
            c.recipes_discovered.1 as u64,
        );
        check_row(
            col,
            "Every required bouquet bound once",
            c.required_bouquets.0 as u64,
            c.required_bouquets.1 as u64,
        );
    });
    column(body, |col| {
        if session.is_game_complete() {
            heading(col, "The journey is complete");
            col.spawn(theme::text(
                "Every flower in the valley has been gathered. Wander as long as you like: nothing new is asked of you.",
                font::BODY,
                palette::INK_SOFT,
            ));
            col.spawn(Node {
                margin: UiRect::top(Val::Px(space::SM)),
                ..default()
            })
            .with_children(|b| {
                spawn_button(b, "Journey summary", ButtonVariant::Compact, Act::Summary);
            });
            return;
        }
        heading(col, "Next up");
        let mut next: Vec<String> = Vec::new();
        match session.level_progress() {
            Some((into, needed)) => next.push(format!(
                "Level {}: {} more XP (+{} skill point)",
                session.level() + 1,
                needed - into,
                data.progression.skill_points_per_level
            )),
            None => next.push("Highest level reached.".into()),
        }
        if let Some(r) = (0..data.regions.len()).find(|&r| !session.is_region_unlocked(r)) {
            let def = &data.regions[r];
            next.push(format!(
                "{} opens at level {} with {} flowers picked (now level {}, {} picked)",
                def.name,
                def.unlock.level,
                session.region_harvest_threshold(r),
                session.level(),
                session.state().stats.flowers_harvested
            ));
        }
        let mut tools: Vec<usize> = (0..data.tools.len())
            .filter(|&t| session.tool_status(t) != ToolStatus::Owned)
            .collect();
        tools.sort_by_key(|&t| (data.tools[t].unlock_level, data.tools[t].price));
        if let Some(&t) = tools.first() {
            let def = &data.tools[t];
            next.push(format!(
                "{}: level {}, {} coins at the tool shelf",
                def.name,
                def.unlock_level,
                session.tool_price(t)
            ));
        }
        let found = session.state().discovered.len() as u32;
        if let Some(&m) = data
            .progression
            .collection_milestones
            .iter()
            .find(|&&m| found < m)
        {
            next.push(format!(
                "Discover {m} species for a skill point ({found} so far)"
            ));
        }
        let open: Vec<(usize, u64, u64)> = (0..data.achievements.len())
            .filter(|&a| !session.is_achievement_unlocked(a))
            .map(|a| {
                let (now, target) = session.achievement_progress(a);
                (a, now, target)
            })
            .collect();
        if let Some(&(a, now, target)) = open
            .iter()
            .max_by(|x, y| (x.1 as f64 / x.2 as f64).total_cmp(&(y.1 as f64 / y.2 as f64)))
        {
            next.push(format!(
                "Closest achievement: {} ({now} / {target})",
                data.achievements[a].name
            ));
        }
        for line in next {
            col.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(space::SM), Val::Px(6.0)),
                    border: UiRect::all(Val::Px(1.5)),
                    border_radius: BorderRadius::all(Val::Px(radius::SM)),
                    ..default()
                },
                BackgroundColor(palette::PARCHMENT),
                BorderColor::all(palette::BEIGE),
            ))
            .with_children(|b| {
                b.spawn(theme::text(line, font::BODY, palette::INK));
            });
        }
    });
}
