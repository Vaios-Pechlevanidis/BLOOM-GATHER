//! In-game heads-up display: region progress, collection, coins, level and
//! XP, the crosshair, the look-at prompt and a controls hint.
//!
//! Everything shown is backed by the session; the tool slot appears with the
//! tool system (Phase D).

use bevy::prelude::*;

use super::theme::{self, font, palette, radius, space};
use crate::game::{ActiveRegion, AppState, Content, PlayState, Session};
use crate::interaction::LookTarget;

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_hud)
            .add_systems(OnEnter(PlayState::Playing), show_crosshair::<true>)
            .add_systems(OnExit(PlayState::Playing), show_crosshair::<false>)
            .add_systems(
                Update,
                (update_stats, update_prompt).run_if(in_state(AppState::InGame)),
            );
    }
}

#[derive(Component)]
struct Crosshair;

#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Stat {
    Remaining,
    Collection,
    Coins,
    Level,
    XpText,
}

#[derive(Component)]
struct XpFill;

#[derive(Component)]
struct Prompt;

#[derive(Component)]
struct PromptName;

#[derive(Component)]
struct PromptRarity;

fn spawn_hud(mut commands: Commands, content: Res<Content>, active: Res<ActiveRegion>) {
    let region = active.def(&content);

    // Region card, top left.
    commands
        .spawn((
            StateScoped(AppState::InGame),
            theme::card(Node {
                position_type: PositionType::Absolute,
                top: Val::Px(space::MD),
                left: Val::Px(space::MD),
                flex_direction: FlexDirection::Column,
                padding: UiRect::axes(Val::Px(space::LG), Val::Px(space::MD)),
                row_gap: Val::Px(2.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }),
        ))
        .with_children(|card| {
            card.spawn(theme::text(region.name.clone(), font::TITLE, palette::INK));
            card.spawn((
                Stat::Remaining,
                theme::text("", font::CAPTION, palette::INK_SOFT),
            ));
            card.spawn((
                Stat::Collection,
                theme::text("", font::CAPTION, palette::INK_SOFT),
            ));
        });

    // Coins and level, top right.
    commands
        .spawn((
            StateScoped(AppState::InGame),
            theme::card(Node {
                position_type: PositionType::Absolute,
                top: Val::Px(space::MD),
                right: Val::Px(space::MD),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::FlexEnd,
                padding: UiRect::axes(Val::Px(space::LG), Val::Px(space::MD)),
                row_gap: Val::Px(4.0),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }),
        ))
        .with_children(|card| {
            card.spawn((Stat::Coins, theme::text("", font::TITLE, palette::INK)));
            card.spawn((Stat::Level, theme::text("", font::BODY, palette::INK)));
            card.spawn((
                Node {
                    width: Val::Px(150.0),
                    height: Val::Px(7.0),
                    ..default()
                },
                BackgroundColor(palette::BEIGE),
                BorderRadius::all(Val::Px(4.0)),
            ))
            .with_children(|bar| {
                bar.spawn((
                    XpFill,
                    Node {
                        width: Val::Percent(0.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                    BackgroundColor(palette::SAGE),
                    BorderRadius::all(Val::Px(4.0)),
                ));
            });
            card.spawn((
                Stat::XpText,
                theme::text("", font::CAPTION, palette::INK_SOFT),
            ));
        });

    // Crosshair and the look-at prompt just below it.
    commands
        .spawn((
            StateScoped(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
        ))
        .with_children(|root| {
            root.spawn((
                Crosshair,
                Node {
                    width: Val::Px(7.0),
                    height: Val::Px(7.0),
                    border: UiRect::all(Val::Px(1.0)),
                    ..default()
                },
                BackgroundColor(palette::CREAM.with_alpha(0.9)),
                BorderColor(palette::INK.with_alpha(0.55)),
                BorderRadius::all(Val::Px(4.0)),
            ));
            root.spawn((
                Prompt,
                Visibility::Hidden,
                Node {
                    position_type: PositionType::Absolute,
                    top: Val::Percent(56.0),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(space::LG), Val::Px(space::SM)),
                    row_gap: Val::Px(4.0),
                    ..default()
                },
                BackgroundColor(palette::CREAM.with_alpha(0.88)),
                BorderRadius::all(Val::Px(radius::MD)),
            ))
            .with_children(|prompt| {
                prompt.spawn((PromptName, theme::text("", font::BODY, palette::INK)));
                prompt.spawn((
                    PromptRarity,
                    theme::text("", font::CAPTION, palette::INK_SOFT),
                ));
                prompt
                    .spawn(Node {
                        align_items: AlignItems::Center,
                        column_gap: Val::Px(space::SM),
                        ..default()
                    })
                    .with_children(|row| {
                        row.spawn((
                            Node {
                                padding: UiRect::axes(Val::Px(7.0), Val::Px(1.0)),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BorderColor(palette::INK_SOFT),
                            BorderRadius::all(Val::Px(5.0)),
                            BackgroundColor(palette::PARCHMENT),
                        ))
                        .with_children(|key| {
                            key.spawn(theme::text("E", font::CAPTION, palette::INK));
                        });
                        row.spawn(theme::text("Pick", font::CAPTION, palette::INK));
                    });
            });
        });

    // Controls hint, bottom centre.
    commands
        .spawn((
            StateScoped(AppState::InGame),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(space::MD),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
        ))
        .with_children(|row| {
            row.spawn((
                Node {
                    padding: UiRect::axes(Val::Px(space::LG), Val::Px(space::SM)),
                    ..default()
                },
                BackgroundColor(palette::CREAM.with_alpha(0.78)),
                BorderRadius::all(Val::Px(radius::PILL)),
            ))
            .with_children(|hint| {
                hint.spawn(theme::text(
                    "WASD  move   Shift  sprint   Space  jump   E  pick   Tab  basket   Esc  pause   F3  performance",
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
            });
        });
}

fn show_crosshair<const VISIBLE: bool>(mut crosshairs: Query<&mut Visibility, With<Crosshair>>) {
    for mut v in &mut crosshairs {
        *v = if VISIBLE {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn update_stats(
    session: Option<Res<Session>>,
    active: Res<ActiveRegion>,
    mut texts: Query<(&Stat, &mut Text)>,
    mut fill: Query<&mut Node, With<XpFill>>,
) {
    let Some(session) = session else { return };
    let data = session.data();
    let region = &data.regions[active.0];
    let discovered = session.state().discovered.len();
    let level = session.level();
    let progress = session.level_progress();
    for (stat, mut text) in &mut texts {
        let value = match stat {
            Stat::Remaining => format!(
                "{} of {} flowers still blooming",
                session.remaining_in_region(active.0),
                region.flower_total
            ),
            Stat::Collection => format!("Collection {discovered} / {} species", data.species.len()),
            Stat::Coins => format!("{} coins", session.state().coins),
            Stat::Level => format!("Level {level}"),
            Stat::XpText => match progress {
                Some((into, needed)) => format!("{into} / {needed} XP"),
                None => "Max level".to_string(),
            },
        };
        if text.0 != value {
            text.0 = value;
        }
    }
    let pct = progress.map_or(100.0, |(into, needed)| into as f32 / needed as f32 * 100.0);
    for mut node in &mut fill {
        if node.width != Val::Percent(pct) {
            node.width = Val::Percent(pct);
        }
    }
}

fn update_prompt(
    target: Res<LookTarget>,
    state: Res<State<PlayState>>,
    session: Option<Res<Session>>,
    mut prompt: Query<&mut Visibility, With<Prompt>>,
    mut name: Query<&mut Text, (With<PromptName>, Without<PromptRarity>)>,
    mut rarity: Query<&mut Text, (With<PromptRarity>, Without<PromptName>)>,
) {
    if !target.is_changed() && !state.is_changed() {
        return;
    }
    let Some(session) = session else { return };
    let show = *state.get() == PlayState::Playing && target.species.is_some();
    for mut v in &mut prompt {
        *v = if show {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
    let Some(species) = target.species else {
        return;
    };
    let s = &session.data().species[species];
    let known = session.is_discovered(species);
    for mut t in &mut name {
        t.0 = s.name.clone();
    }
    for mut t in &mut rarity {
        t.0 = if known {
            s.rarity.label().to_string()
        } else {
            format!("{}, not yet in your collection", s.rarity.label())
        };
    }
}
