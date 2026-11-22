//! Travelling between regions: walking through a gate at either end of a
//! region's footpath, or choosing a region on the world map.
//!
//! A journey fades the screen out, asks `bloom_core` to enter the region
//! (refused while it is locked), switches [`ActiveRegion`] so the scene is
//! rebuilt, puts the player at the gate they came through (or the region's
//! start), and fades back in. Movement is frozen while it runs.
//!
//! Near a gate a banner names where it leads, or what it still needs.

use bevy::prelude::*;
use bloom_core::player::{CharacterBody, Environment};
use bloom_core::world::GateSide;
use bloom_core::Game;

use crate::audio::{PlaySfx, Sfx};
use crate::game::{ActiveRegion, AppState, PlayState, RegionScene, Session};
use crate::player::Player;
use crate::rendering::SceneSet;
use crate::ui::theme::{self, font, palette, radius, space};
use crate::ui::Toast;

pub struct TravelPlugin;

impl Plugin for TravelPlugin {
    fn build(&self, app: &mut App) {
        app.add_event::<TravelTo>()
            .init_resource::<GateNotice>()
            .add_systems(Startup, spawn_fade)
            .add_systems(OnEnter(AppState::InGame), spawn_banner)
            .add_systems(OnExit(AppState::InGame), abandon_journey)
            .add_systems(
                Update,
                (
                    (walk_through_gates, update_banner)
                        .run_if(in_state(PlayState::Playing))
                        .run_if(not(resource_exists::<Journey>)),
                    start_journey.run_if(in_state(AppState::InGame)),
                    run_journey
                        .after(SceneSet)
                        .run_if(resource_exists::<Journey>),
                )
                    .chain(),
            );
    }
}

/// Where the player appears in the new region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrival {
    /// Just inside this gate of the destination, facing into the region.
    Gate(GateSide),
    /// The region's start (where its footpath begins).
    Start,
}

/// Ask to travel. Locked regions are refused with a toast.
#[derive(Event, Debug, Clone, Copy)]
pub struct TravelTo {
    pub region: usize,
    pub arrive: Arrival,
}

/// A journey in progress (movement is frozen while it exists).
#[derive(Resource, Debug)]
pub struct Journey {
    to: usize,
    arrive: Arrival,
    phase: Phase,
    /// Seconds into the current phase (real time).
    t: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Phase {
    FadeOut,
    /// Waiting for the new scene; counts frames so its meshes are spawned
    /// before the fade lifts.
    Build(u8),
    FadeIn,
}

const FADE_OUT_SECS: f32 = 0.35;
const FADE_IN_SECS: f32 = 0.6;

#[derive(Component)]
struct Fade;

#[derive(Component)]
struct Banner;

#[derive(Component)]
struct BannerTitle;

#[derive(Component)]
struct BannerDetail;

/// The gate the player last bumped into while it was locked, so the
/// refusal is said once per visit to the gate, not every frame.
#[derive(Resource, Default)]
struct GateNotice(Option<GateSide>);

/// How far from a gate its banner shows.
const BANNER_RANGE: f32 = 9.0;

/// "level 3 (you're level 2) and 60 flowers picked (12 so far)": only the
/// parts still missing.
pub fn requirements_text(game: &Game, region: usize) -> String {
    let req = game.region_requirements(region);
    let mut parts = Vec::new();
    if req.level.1 < req.level.0 {
        parts.push(format!(
            "level {} (you're level {})",
            req.level.0, req.level.1
        ));
    }
    if req.flowers.1 < req.flowers.0 {
        parts.push(format!(
            "{} flowers picked ({} so far)",
            req.flowers.0, req.flowers.1
        ));
    }
    parts.join(" and ")
}

fn spawn_fade(mut commands: Commands) {
    commands.spawn((
        Fade,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.1, 0.08, 0.0)),
        GlobalZIndex(60),
        Pickable::IGNORE,
    ));
}

fn spawn_banner(mut commands: Commands) {
    commands
        .spawn((
            StateScoped(AppState::InGame),
            crate::ui::HudRoot,
            // Above the tool bar, clear of the toasts at the top.
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(116.0),
                width: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                ..default()
            },
            Pickable::IGNORE,
        ))
        .with_children(|row| {
            row.spawn((
                Banner,
                Visibility::Hidden,
                Node {
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    padding: UiRect::axes(Val::Px(space::LG), Val::Px(space::SM)),
                    row_gap: Val::Px(2.0),
                    max_width: Val::Px(560.0),
                    ..default()
                },
                BackgroundColor(palette::CREAM.with_alpha(0.9)),
                BorderRadius::all(Val::Px(radius::MD)),
            ))
            .with_children(|b| {
                b.spawn((BannerTitle, theme::text("", font::BODY, palette::INK)));
                b.spawn((
                    BannerDetail,
                    theme::text("", font::CAPTION, palette::INK_SOFT),
                    TextLayout::new_with_justify(JustifyText::Center),
                ));
            });
        });
}

/// Walking into an open gate starts a journey; a locked one says what it
/// still needs (once per visit).
fn walk_through_gates(
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    player: Option<Res<Player>>,
    mut notice: ResMut<GateNotice>,
    mut travel: EventWriter<TravelTo>,
    mut toasts: EventWriter<Toast>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let (Some(session), Some(scene), Some(player)) = (session, scene, player) else {
        return;
    };
    let at = player.body.ground_position();
    let Some(side) = scene.layout.gate_at(at) else {
        notice.0 = None;
        return;
    };
    let Some(to) = session.gate_destination(scene.region, side) else {
        return;
    };
    if session.is_region_unlocked(to) {
        let arrive = session
            .arrival_side(scene.region, to)
            .map_or(Arrival::Start, Arrival::Gate);
        travel.write(TravelTo { region: to, arrive });
    } else if notice.0 != Some(side) {
        notice.0 = Some(side);
        let name = &session.data().regions[to].name;
        toasts.write(Toast::info(format!(
            "{name} opens at {}",
            requirements_text(&session, to)
        )));
        sfx.write(PlaySfx::new(Sfx::Deny));
    }
}

#[allow(clippy::type_complexity)]
fn update_banner(
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    player: Option<Res<Player>>,
    mut banner: Query<&mut Visibility, With<Banner>>,
    mut texts: Query<(&mut Text, Has<BannerTitle>), Or<(With<BannerTitle>, With<BannerDetail>)>>,
) {
    let (Some(session), Some(scene), Some(player)) = (session, scene, player) else {
        return;
    };
    let at = player.body.ground_position();
    let near = [GateSide::North, GateSide::South]
        .into_iter()
        .filter_map(|side| {
            let to = session.gate_destination(scene.region, side)?;
            let d = scene.layout.gate_distance(side, at);
            (d < BANNER_RANGE).then_some((to, d))
        })
        .min_by(|a, b| a.1.total_cmp(&b.1));
    let lines = near.map(|(to, _)| {
        let region = &session.data().regions[to];
        if session.is_region_unlocked(to) {
            let detail = if session.has_visited(to) {
                format!(
                    "{} of {} flowers still blooming",
                    session.remaining_in_region(to),
                    region.flower_total
                )
            } else {
                "Not visited yet".to_string()
            };
            (format!("This way to {}", region.name), detail)
        } else {
            (
                format!("{} (locked)", region.name),
                format!("Opens at {}", requirements_text(&session, to)),
            )
        }
    });
    for mut v in &mut banner {
        let want = if lines.is_some() {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if let Some((title, detail)) = lines {
        for (mut text, is_title) in &mut texts {
            let value = if is_title { &title } else { &detail };
            if text.0 != *value {
                text.0 = value.clone();
            }
        }
    }
}

fn start_journey(
    mut commands: Commands,
    mut requests: EventReader<TravelTo>,
    journey: Option<Res<Journey>>,
    session: Option<Res<Session>>,
    mut toasts: EventWriter<Toast>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let Some(request) = requests.read().last().copied() else {
        return;
    };
    let Some(session) = session else { return };
    if journey.is_some() {
        return;
    }
    if !session.is_region_unlocked(request.region) {
        let name = &session.data().regions[request.region].name;
        toasts.write(Toast::info(format!(
            "{name} opens at {}",
            requirements_text(&session, request.region)
        )));
        sfx.write(PlaySfx::new(Sfx::Deny));
        return;
    }
    sfx.write(PlaySfx::new(Sfx::Whoosh));
    commands.insert_resource(Journey {
        to: request.region,
        arrive: request.arrive,
        phase: Phase::FadeOut,
        t: 0.0,
    });
}

#[allow(clippy::too_many_arguments)]
fn run_journey(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut journey: ResMut<Journey>,
    mut active: ResMut<ActiveRegion>,
    mut session: ResMut<Session>,
    scene: Option<Res<RegionScene>>,
    player: Option<ResMut<Player>>,
    mut fade: Query<&mut BackgroundColor, With<Fade>>,
    mut notice: ResMut<GateNotice>,
) {
    journey.t += time.delta_secs();
    let alpha = match journey.phase {
        Phase::FadeOut => {
            let a = (journey.t / FADE_OUT_SECS).min(1.0);
            if a >= 1.0 {
                if session.enter_region(journey.to).is_err() {
                    // Can't happen (checked when the journey began), but
                    // never leave the screen dark.
                    journey.phase = Phase::FadeIn;
                } else {
                    active.0 = journey.to;
                    journey.phase = Phase::Build(0);
                }
                journey.t = 0.0;
            }
            a
        }
        Phase::Build(frames) => {
            let ready = scene.as_ref().is_some_and(|s| s.region == journey.to);
            if ready && frames == 0 {
                if let (Some(scene), Some(mut player)) = (scene, player) {
                    let layout = &scene.layout;
                    let (at, yaw) = match journey.arrive {
                        Arrival::Gate(side) => layout.arrival(side),
                        Arrival::Start => (layout.spawn, layout.spawn_yaw),
                    };
                    let env = Environment {
                        terrain: &layout.terrain,
                        colliders: &layout.colliders,
                    };
                    player.teleport(CharacterBody::standing_at(at, &env), yaw);
                }
                notice.0 = None;
                journey.phase = Phase::Build(1);
            } else if ready {
                // A couple of frames for grass, flowers and props to spawn.
                journey.phase = if frames >= 3 {
                    journey.t = 0.0;
                    Phase::FadeIn
                } else {
                    Phase::Build(frames + 1)
                };
            }
            1.0
        }
        Phase::FadeIn => {
            let a = 1.0 - (journey.t / FADE_IN_SECS).min(1.0);
            if a <= 0.0 {
                commands.remove_resource::<Journey>();
            }
            a
        }
    };
    for mut bg in &mut fade {
        bg.0 = bg.0.with_alpha(alpha);
    }
}

/// Leaving to the title screen mid-journey: finish instantly.
fn abandon_journey(
    mut commands: Commands,
    journey: Option<Res<Journey>>,
    mut fade: Query<&mut BackgroundColor, With<Fade>>,
) {
    if journey.is_some() {
        commands.remove_resource::<Journey>();
        for mut bg in &mut fade {
            bg.0 = bg.0.with_alpha(0.0);
        }
    }
}
