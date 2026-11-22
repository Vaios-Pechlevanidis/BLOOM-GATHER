//! Small notifications at the top of the screen (discoveries, level-ups,
//! things that couldn't be done). They stack, then fade away on their own.

use bevy::prelude::*;

use crate::game::PlayState;

use super::theme::{self, font, palette, radius, space};

pub struct ToastsPlugin;

impl Plugin for ToastsPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<Toast>()
            .add_systems(Startup, spawn_root)
            .add_systems(Update, (place_root, spawn_toasts, age_toasts).chain());
    }
}

#[derive(Event, Debug, Clone)]
pub struct Toast {
    pub title: String,
    pub body: Option<String>,
    pub highlight: bool,
}

impl Toast {
    pub fn info(title: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: None,
            highlight: false,
        }
    }

    pub fn highlight(title: impl Into<String>, body: String) -> Self {
        Self {
            title: title.into(),
            body: Some(body),
            highlight: true,
        }
    }
}

#[derive(Component)]
struct ToastRoot;

/// Toasts sit at the top while playing; while a screen (basket, workbench,
/// skill tree, journal, pause) is open they move to the bottom edge so they
/// never cover its heading.
fn place_root(state: Option<Res<State<PlayState>>>, mut roots: Query<&mut Node, With<ToastRoot>>) {
    let screen_open = state.is_some_and(|s| *s.get() != PlayState::Playing);
    for mut node in &mut roots {
        let (top, bottom) = if screen_open {
            (Val::Auto, Val::Px(space::MD))
        } else {
            (Val::Px(space::MD), Val::Auto)
        };
        if node.top != top || node.bottom != bottom {
            node.top = top;
            node.bottom = bottom;
            node.flex_direction = if screen_open {
                FlexDirection::ColumnReverse
            } else {
                FlexDirection::Column
            };
        }
    }
}

#[derive(Component)]
struct ToastCard {
    age: f32,
    life: f32,
}

const MAX_TOASTS: usize = 3;
const FADE_SECS: f32 = 0.5;

fn spawn_root(mut commands: Commands) {
    commands.spawn((
        ToastRoot,
        Node {
            position_type: PositionType::Absolute,
            top: Val::Px(space::MD),
            width: Val::Percent(100.0),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::Center,
            row_gap: Val::Px(space::SM),
            ..default()
        },
        GlobalZIndex(40),
    ));
}

fn spawn_toasts(
    mut commands: Commands,
    mut events: EventReader<Toast>,
    state: Option<Res<State<PlayState>>>,
    root: Query<Entity, With<ToastRoot>>,
    existing: Query<(Entity, &ToastCard)>,
) {
    let Ok(root) = root.single() else { return };
    // Only the newest few of a burst are shown, and the oldest visible ones
    // make room for them, so there are never more than MAX_TOASTS.
    let incoming: Vec<Toast> = events.read().cloned().collect();
    let skip = incoming.len().saturating_sub(MAX_TOASTS);
    let incoming = &incoming[skip..];
    let mut current: Vec<(Entity, f32)> = existing.iter().map(|(e, c)| (e, c.age)).collect();
    current.sort_by(|a, b| b.1.total_cmp(&a.1)); // oldest first
    let overflow = (current.len() + incoming.len()).saturating_sub(MAX_TOASTS);
    for (entity, _) in current.iter().take(overflow) {
        commands.entity(*entity).despawn();
    }
    for toast in incoming {
        let accent = if toast.highlight {
            palette::MUTED_YELLOW
        } else {
            palette::BEIGE
        };
        let life = if toast.highlight { 4.0 } else { 2.5 };
        commands.entity(root).with_children(|r| {
            r.spawn((
                ToastCard { age: 0.0, life },
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(space::LG), Val::Px(space::SM)),
                    border: UiRect::all(Val::Px(2.0)),
                    max_width: Val::Px(560.0),
                    ..default()
                },
                BackgroundColor(palette::CREAM.with_alpha(0.95)),
                BorderColor(accent),
                BorderRadius::all(Val::Px(radius::MD)),
            ))
            .with_children(|card| {
                card.spawn(theme::text(toast.title.clone(), font::BODY, palette::INK));
                // Over an open screen, titles only: short enough to stay
                // clear of its buttons at the bottom edge.
                let compact = state
                    .as_ref()
                    .is_some_and(|s| *s.get() != PlayState::Playing);
                if let Some(body) = toast.body.as_ref().filter(|_| !compact) {
                    card.spawn(theme::text(body.clone(), font::CAPTION, palette::INK_SOFT));
                }
            });
        });
    }
}

fn age_toasts(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut cards: Query<(Entity, &mut ToastCard, &mut BackgroundColor, &Children)>,
    mut texts: Query<&mut TextColor>,
) {
    for (entity, mut card, mut bg, children) in &mut cards {
        card.age += time.delta_secs();
        if card.age >= card.life {
            commands.entity(entity).despawn();
            continue;
        }
        let alpha = ((card.life - card.age) / FADE_SECS).min(1.0);
        bg.0.set_alpha(0.95 * alpha);
        for child in children.iter() {
            if let Ok(mut c) = texts.get_mut(child) {
                c.0.set_alpha(alpha);
            }
        }
    }
}
