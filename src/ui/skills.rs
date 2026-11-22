//! The skill tree (K): five branches side by side, each a chain from top to
//! bottom. Select a skill to see what it does now and what the next level
//! changes, and learn it with skill points.
//!
//! Keyboard: arrows move between skills, Enter learns, K or Esc closes.

use bevy::prelude::*;
use bloom_core::data::SkillBranch;
use bloom_core::game::{GameEvent, SkillLock};
use bloom_core::SkillStatus;

use super::effects::change_lines;
use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{AppState, EventsForwarded, GameEventMsg, PlayState, Session};
use crate::input::Bindings;
use crate::ui::Toast;

pub struct SkillsPlugin;

impl Plugin for SkillsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<TreeView>()
            .add_systems(OnEnter(PlayState::Skills), spawn_screen)
            .add_systems(
                Update,
                (
                    toggle.run_if(in_state(AppState::InGame)),
                    (keys, buttons, watch_events.after(EventsForwarded), rebuild)
                        .chain()
                        .run_if(in_state(PlayState::Skills)),
                ),
            );
    }
}

#[derive(Resource, Default)]
struct TreeView {
    selected: usize,
    stale: bool,
}

#[derive(Component, Clone, Copy)]
enum Act {
    Select(usize),
    Learn,
    Close,
}

#[derive(Component)]
struct Body;

#[derive(Component)]
struct Points;

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
) {
    if Bindings::just_pressed(&keys, &bindings.skills) {
        match state.get() {
            PlayState::Playing => next.set(PlayState::Skills),
            PlayState::Skills => next.set(PlayState::Playing),
            _ => {}
        }
    }
}

fn spawn_screen(mut commands: Commands, mut view: ResMut<TreeView>, bindings: Res<Bindings>) {
    view.stale = true;
    commands
        .spawn((
            DespawnOnExit(PlayState::Skills),
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
                width: Val::Px(1180.0),
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
                    h.spawn(theme::text("Skill Tree", font::HEADING, palette::INK));
                    h.spawn((Points, theme::text("", font::BODY, palette::INK_SOFT)));
                });
                card.spawn((
                    Body,
                    Node {
                        flex_direction: FlexDirection::Column,
                        row_gap: Val::Px(space::MD),
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
                        &format!("Close ({})", bindings.label(crate::input::Action::Skills)),
                        ButtonVariant::Compact,
                        Act::Close,
                    );
                });
            });
        });
}

/// Skills of each branch, in tree order.
fn columns(session: &Session) -> Vec<(SkillBranch, Vec<usize>)> {
    let skills = &session.data().skills;
    SkillBranch::ALL
        .iter()
        .map(|&b| {
            (
                b,
                (0..skills.len())
                    .filter(|&i| skills[i].branch == b)
                    .collect(),
            )
        })
        .filter(|(_, list): &(SkillBranch, Vec<usize>)| !list.is_empty())
        .collect()
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    session: Option<ResMut<Session>>,
    mut view: ResMut<TreeView>,
    mut sfx: MessageWriter<PlaySfx>,
    mut toasts: MessageWriter<Toast>,
) {
    let Some(mut session) = session else { return };
    let cols = columns(&session);
    let Some((col, row)) = cols.iter().enumerate().find_map(|(c, (_, list))| {
        list.iter()
            .position(|&s| s == view.selected)
            .map(|r| (c, r))
    }) else {
        view.selected = cols.first().map_or(0, |(_, l)| l[0]);
        return;
    };
    let before = view.selected;
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::ArrowLeft) {
        let c = if keys.just_pressed(KeyCode::ArrowRight) {
            (col + 1) % cols.len()
        } else {
            (col + cols.len() - 1) % cols.len()
        };
        let list = &cols[c].1;
        view.selected = list[row.min(list.len() - 1)];
    }
    let list = &cols[col].1;
    if keys.just_pressed(KeyCode::ArrowDown) {
        view.selected = list[(row + 1).min(list.len() - 1)];
    }
    if keys.just_pressed(KeyCode::ArrowUp) {
        view.selected = list[row.saturating_sub(1)];
    }
    if view.selected != before {
        view.stale = true;
    }
    if Bindings::just_pressed(&keys, &bindings.confirm) {
        learn(&mut session, view.selected, &mut sfx, &mut toasts);
        view.stale = true;
    }
}

fn learn(
    session: &mut Session,
    skill: usize,
    sfx: &mut MessageWriter<PlaySfx>,
    toasts: &mut MessageWriter<Toast>,
) {
    // Success feedback comes from the SkillLearned event.
    if let Err(err) = session.learn_skill(skill) {
        sfx.write(PlaySfx::new(Sfx::Deny));
        let msg = err.to_string();
        let mut c = msg.chars();
        let msg = c
            .next()
            .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
            .unwrap_or_default();
        toasts.write(Toast::info(msg));
    }
}

fn buttons(
    pressed: Query<(&Interaction, &Act), Changed<Interaction>>,
    session: Option<ResMut<Session>>,
    mut view: ResMut<TreeView>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: MessageWriter<PlaySfx>,
    mut toasts: MessageWriter<Toast>,
) {
    let Some(mut session) = session else { return };
    for (interaction, act) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *act {
            Act::Select(s) => {
                view.selected = s;
                sfx.write(PlaySfx::new(Sfx::Click));
            }
            Act::Learn => learn(&mut session, view.selected, &mut sfx, &mut toasts),
            Act::Close => {
                next.set(PlayState::Playing);
                continue;
            }
        }
        view.stale = true;
    }
}

fn watch_events(mut events: MessageReader<GameEventMsg>, mut view: ResMut<TreeView>) {
    for GameEventMsg(e) in events.read() {
        if matches!(
            e,
            GameEvent::SkillLearned { .. }
                | GameEvent::LevelUp { .. }
                | GameEvent::ToolBought { .. }
        ) {
            view.stale = true;
        }
    }
}

fn rebuild(
    mut commands: Commands,
    mut view: ResMut<TreeView>,
    session: Option<Res<Session>>,
    body: Query<(Entity, Option<&Children>), With<Body>>,
    mut points: Query<&mut Text, With<Points>>,
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
    let data = session.data().clone();
    let selected = view.selected.min(data.skills.len().saturating_sub(1));
    let have = session.skill_points();
    for mut t in &mut points {
        t.0 = format!(
            "{have} skill point{} to spend   ({} earned)",
            if have == 1 { "" } else { "s" },
            session.skill_points_earned()
        );
    }

    commands.entity(body).with_children(|body| {
        // The five branches.
        body.spawn(Node {
            column_gap: Val::Px(space::MD),
            ..default()
        })
        .with_children(|tree| {
            for (branch, list) in columns(&session) {
                tree.spawn(Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Stretch,
                    flex_grow: 1.0,
                    flex_basis: Val::Px(0.0),
                    ..default()
                })
                .with_children(|col| {
                    col.spawn((
                        Node {
                            margin: UiRect::bottom(Val::Px(space::SM)),
                            ..default()
                        },
                        theme::text(branch.label(), font::BODY, palette::INK),
                    ));
                    for (n, &s) in list.iter().enumerate() {
                        if n > 0 {
                            // Connector to the skill above.
                            col.spawn(Node {
                                justify_content: JustifyContent::Center,
                                ..default()
                            })
                            .with_children(|c| {
                                c.spawn((
                                    Node {
                                        width: Val::Px(3.0),
                                        height: Val::Px(12.0),
                                        ..default()
                                    },
                                    BackgroundColor(if session.skill_level(s) > 0 {
                                        palette::SAGE
                                    } else {
                                        palette::BEIGE
                                    }),
                                ));
                            });
                        }
                        skill_node(col, &session, s, s == selected);
                    }
                });
            }
        });

        // Details of the selected skill.
        let def = &data.skills[selected];
        let status = session.skill_status(selected);
        let level = session.skill_level(selected);
        body.spawn((
            Node {
                flex_direction: FlexDirection::Column,
                row_gap: Val::Px(4.0),
                padding: UiRect::all(Val::Px(space::MD)),
                border: UiRect::all(Val::Px(1.5)),
                min_height: Val::Px(150.0),
                border_radius: BorderRadius::all(Val::Px(radius::MD)),
                ..default()
            },
            BackgroundColor(palette::PARCHMENT),
            BorderColor::all(palette::BEIGE),
        ))
        .with_children(|d| {
            d.spawn(Node {
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..default()
            })
            .with_children(|row| {
                row.spawn(theme::text(def.name.clone(), font::TITLE, palette::INK));
                row.spawn(theme::text(
                    format!(
                        "{}   level {level} / {}   {} skill point{} per level",
                        def.branch.label(),
                        def.max_level,
                        def.cost,
                        if def.cost == 1 { "" } else { "s" }
                    ),
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
            });
            d.spawn(theme::text(def.description.clone(), font::BODY, palette::INK));
            let preview = (status != SkillStatus::Maxed).then(|| session.preview_skill(selected));
            for line in change_lines(&session, preview.as_ref(), &def.effects) {
                d.spawn(theme::text(line, font::CAPTION, palette::SAGE_DEEP));
            }
            d.spawn(Node {
                align_items: AlignItems::Center,
                column_gap: Val::Px(space::MD),
                margin: UiRect::top(Val::Px(space::SM)),
                ..default()
            })
            .with_children(|row| match &status {
                SkillStatus::Maxed => {
                    row.spawn(theme::text("Fully learned", font::BODY, palette::SAGE_DEEP));
                }
                SkillStatus::Learnable => {
                    spawn_button(row, "Learn (Enter)", ButtonVariant::Primary, Act::Learn);
                }
                SkillStatus::Locked(lock) => {
                    let why = match lock {
                        SkillLock::NeedsSkill(id) => format!(
                            "Learn {} first",
                            data.skill(id).map_or(id.as_str(), |s| s.name.as_str())
                        ),
                        SkillLock::NeedsTool(id) => format!(
                            "Needs the {}",
                            data.tool(id).map_or(id.as_str(), |t| t.name.as_str())
                        ),
                        SkillLock::Points { needed, have } => format!(
                            "Needs {needed} skill point{}; you have {have}. Level up, complete regions or discover species to earn more.",
                            if *needed == 1 { "" } else { "s" }
                        ),
                    };
                    row.spawn(theme::text(why, font::BODY, palette::BROWN));
                }
            });
        });
    });
}

fn skill_node(col: &mut ChildSpawnerCommands, session: &Session, skill: usize, selected: bool) {
    let def = &session.data().skills[skill];
    let level = session.skill_level(skill);
    let status = session.skill_status(skill);
    let (bg, ink) = match (&status, level) {
        (SkillStatus::Maxed, _) => (palette::PASTEL_GREEN, palette::INK),
        (_, l) if l > 0 => (
            palette::PASTEL_GREEN.mix(&palette::PARCHMENT, 0.5),
            palette::INK,
        ),
        (SkillStatus::Learnable, _) => (palette::PARCHMENT, palette::INK),
        _ => (palette::CREAM, palette::INK_SOFT),
    };
    let border = if selected {
        palette::INK
    } else if status == SkillStatus::Learnable {
        palette::SAGE
    } else {
        palette::BEIGE
    };
    col.spawn((
        Button,
        Act::Select(skill),
        Node {
            flex_direction: FlexDirection::Column,
            padding: UiRect::axes(Val::Px(space::SM), Val::Px(6.0)),
            row_gap: Val::Px(2.0),
            border: UiRect::all(Val::Px(if selected { 2.5 } else { 1.5 })),
            border_radius: BorderRadius::all(Val::Px(radius::SM)),
            ..default()
        },
        BackgroundColor(bg),
        BorderColor::all(border),
    ))
    .with_children(|n| {
        n.spawn(theme::text(def.name.clone(), font::CAPTION, ink));
        n.spawn(theme::text(
            format!("{level} / {}   {} SP", def.max_level, def.cost),
            font::CAPTION,
            palette::INK_SOFT,
        ));
    });
}
