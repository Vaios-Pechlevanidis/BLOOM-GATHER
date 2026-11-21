//! In-game heads-up display.
//!
//! Shows only information that is real today: the active region, its finite
//! flower count, a crosshair and the controls. Coins, level, XP, selected
//! tool and interaction prompts appear with the systems that back them —
//! no placeholder counters.

use bevy::prelude::*;

use super::theme::{self, font, palette, space};
use crate::game::{ActiveRegion, AppState, Content, PlayState};

pub struct HudPlugin;

impl Plugin for HudPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(OnEnter(AppState::InGame), spawn_hud)
            .add_systems(OnEnter(PlayState::Playing), show_crosshair::<true>)
            .add_systems(OnExit(PlayState::Playing), show_crosshair::<false>);
    }
}

#[derive(Component)]
struct Crosshair;

fn spawn_hud(mut commands: Commands, content: Res<Content>, active: Res<ActiveRegion>) {
    let region = active.def(&content);
    let subtitle = format!(
        "{} flowers bloom here, {} species",
        region.flower_total,
        region.flowers.len()
    );

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
            card.spawn(theme::text(subtitle, font::CAPTION, palette::INK_SOFT));
        });

    // Crosshair: a small soft dot with a dark ring, readable on any ground.
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
                BorderRadius::all(Val::Px(theme::radius::PILL)),
            ))
            .with_children(|hint| {
                hint.spawn(theme::text(
                    "WASD  move    Shift  sprint    Space  jump    Mouse  look    Esc  pause    F3  performance",
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
