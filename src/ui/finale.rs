//! The ending. When the last completion requirement is met the game plays
//! a short finale:
//!
//! 1. **Celebration**: golden petals burst around you, the camera lifts away
//!    and "Every flower gathered" fades in.
//! 2. **Tour**: the camera glides along the footpath of each of the five
//!    regions in turn while every flower you picked there blooms again in
//!    memory: soft, glowing and see-through, only for the tour.
//! 3. **Journey summary**: the numbers of the whole journey (frozen at the
//!    moment of completion) and the full collection.
//!
//! Esc skips the tour. "Keep wandering" starts free roam: nothing new is
//! asked of you, and the summary stays in the journal. If the game closes
//! mid-finale, it plays again on the next start.

use bevy::prelude::*;
use bevy::window::{CursorOptions, PrimaryWindow};
use bloom_core::rng::SplitMix64;
use bloom_core::Game;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use super::HudRoot;
use crate::audio::{PlaySfx, Sfx};
use crate::flowers::MemoryBloom;
use crate::game::{ActiveRegion, AppState, PlayState, RegionScene, Session};
use crate::input::Bindings;
use crate::player::Player;
use crate::rendering::{MainCamera, SceneSet};
use crate::travel::Journey;

pub struct FinalePlugin;

impl Plugin for FinalePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<FinaleRequest>()
            .add_systems(OnEnter(PlayState::Finale), (begin, show_hud::<false>))
            .add_systems(OnExit(PlayState::Finale), (end, show_hud::<true>))
            .add_systems(
                Update,
                (
                    start_when_complete.run_if(
                        in_state(PlayState::Playing)
                            .or(in_state(PlayState::Workshop))
                            .or(in_state(PlayState::Inventory)),
                    ),
                    (skip, direct, animate_overlay, fly_petals, summary_buttons)
                        .chain()
                        .after(SceneSet)
                        .run_if(in_state(PlayState::Finale)),
                ),
            );
    }
}

/// Open the finale at the summary only (from the journal, after the end).
#[derive(Resource, Default)]
pub struct FinaleRequest {
    pub summary_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    Tour(usize),
    Home,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Phase {
    Celebrate,
    /// Fading to black before showing `Stop`.
    Out(Stop),
    /// Waiting for the region to be built (frames counted after it is).
    Build(Stop, u8),
    /// Gliding over a region (fades in at the start).
    Fly(usize),
    /// Back where the player stood, fading in.
    Return,
    Summary,
}

#[derive(Resource)]
struct Director {
    phase: Phase,
    /// Real seconds into the current phase.
    t: f32,
    /// The region the player stands in (the tour returns there).
    home: usize,
    /// Camera at the start of the celebration.
    from: Transform,
    summary_spawned: bool,
}

const CELEBRATE_SECS: f32 = 5.5;
const FADE_OUT_SECS: f32 = 0.6;
const FADE_IN_SECS: f32 = 0.9;
const FLY_SECS: f32 = 8.0;

#[derive(Component)]
struct Fade;

#[derive(Component)]
struct Title;

#[derive(Component)]
struct Caption;

#[derive(Component)]
struct CaptionDetail;

#[derive(Component)]
struct SkipHint;

#[derive(Component)]
struct SummaryCard;

#[derive(Component)]
struct GoldPetal {
    velocity: Vec3,
    spin: Vec3,
}

#[derive(Component, Clone, Copy)]
enum Act {
    Wander,
    Collection,
    Title,
}

/// Start the ending shortly after the last requirement is met (so the
/// toast and sound of that moment come first). Also after loading a save
/// that completed but never finished its ending.
fn start_when_complete(
    time: Res<Time<Real>>,
    mut waited: Local<f32>,
    session: Option<Res<Session>>,
    journey: Option<Res<Journey>>,
    mut request: ResMut<FinaleRequest>,
    mut next: ResMut<NextState<PlayState>>,
) {
    let pending = session.is_some_and(|s| s.finale_pending());
    if !pending || journey.is_some() {
        *waited = 0.0;
        return;
    }
    *waited += time.delta_secs();
    if *waited >= 1.5 {
        *waited = 0.0;
        request.summary_only = false;
        next.set(PlayState::Finale);
    }
}

fn set_cursor(windows: &mut Query<&mut CursorOptions, With<PrimaryWindow>>, visible: bool) {
    for mut w in windows.iter_mut() {
        w.visible = visible;
    }
}

#[allow(clippy::too_many_arguments)]
fn begin(
    mut commands: Commands,
    request: Res<FinaleRequest>,
    active: Res<ActiveRegion>,
    player: Option<Res<Player>>,
    cameras: Query<&Transform, With<MainCamera>>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    let from = cameras.single().copied().unwrap_or_default();
    let summary_only = request.summary_only;
    commands.insert_resource(crate::game::Touring { home: active.0 });
    commands.insert_resource(Director {
        phase: if summary_only {
            Phase::Summary
        } else {
            Phase::Celebrate
        },
        t: 0.0,
        home: active.0,
        from,
        summary_spawned: false,
    });
    set_cursor(&mut windows, summary_only);
    spawn_overlay(&mut commands);
    if summary_only {
        return;
    }
    sfx.write(PlaySfx::new(Sfx::LevelUp));
    sfx.write(PlaySfx {
        sfx: Sfx::Discover,
        pitch: 0.8,
    });
    // A burst of golden petals around the player.
    let at = player.map_or(from.translation, |p| p.body.position + Vec3::Y * 1.2);
    let petal = meshes.add(Rectangle::new(0.1, 0.07));
    let gold = materials.add(StandardMaterial {
        base_color: Color::LinearRgba(LinearRgba::rgb(2.2, 1.6, 0.5)),
        unlit: true,
        cull_mode: None,
        double_sided: true,
        ..default()
    });
    let mut rng = SplitMix64::new(0x60_1D);
    for _ in 0..160 {
        let a = rng.range_f32(0.0, std::f32::consts::TAU);
        let out = rng.range_f32(0.8, 3.0);
        commands.spawn((
            DespawnOnExit(PlayState::Finale),
            GoldPetal {
                velocity: Vec3::new(a.cos() * out, rng.range_f32(2.0, 5.5), a.sin() * out),
                spin: Vec3::new(
                    rng.range_f32(-4.0, 4.0),
                    rng.range_f32(-4.0, 4.0),
                    rng.range_f32(-4.0, 4.0),
                ),
            },
            Mesh3d(petal.clone()),
            MeshMaterial3d(gold.clone()),
            Transform::from_translation(at + Vec3::new(a.cos(), 0.0, a.sin()) * 0.5),
        ));
    }
}

fn shadow() -> TextShadow {
    TextShadow {
        offset: Vec2::splat(2.0),
        color: Color::srgba(0.1, 0.08, 0.06, 0.0),
    }
}

/// Text and its shadow fade together.
fn set_alpha(color: &mut TextColor, shadow: &mut TextShadow, a: f32) {
    color.0 = color.0.with_alpha(a);
    shadow.color = shadow.color.with_alpha(a * 0.5);
}

fn spawn_overlay(commands: &mut Commands) {
    let scoped = DespawnOnExit(PlayState::Finale);
    commands.spawn((
        scoped.clone(),
        Fade,
        Node {
            position_type: PositionType::Absolute,
            width: Val::Percent(100.0),
            height: Val::Percent(100.0),
            ..default()
        },
        BackgroundColor(Color::srgba(0.12, 0.1, 0.08, 0.0)),
        GlobalZIndex(55),
        Pickable::IGNORE,
    ));
    commands
        .spawn((
            scoped.clone(),
            Node {
                position_type: PositionType::Absolute,
                top: Val::Percent(16.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(space::SM),
                ..default()
            },
            GlobalZIndex(56),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            c.spawn((
                Title,
                theme::text("Every flower gathered", font::HERO, palette::CREAM),
                shadow(),
            ));
            c.spawn((
                Title,
                theme::text(
                    "The valley is quiet now. Every bloom lives on in your memory.",
                    font::TITLE,
                    palette::CREAM,
                ),
                shadow(),
            ));
        });
    commands
        .spawn((
            scoped.clone(),
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(70.0),
                width: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: Val::Px(4.0),
                ..default()
            },
            GlobalZIndex(56),
            Pickable::IGNORE,
        ))
        .with_children(|c| {
            c.spawn((
                Caption,
                theme::text("", font::HEADING, palette::CREAM),
                shadow(),
            ));
            c.spawn((
                CaptionDetail,
                theme::text("", font::BODY, palette::CREAM),
                shadow(),
            ));
        });
    commands
        .spawn((
            scoped,
            SkipHint,
            Node {
                position_type: PositionType::Absolute,
                bottom: Val::Px(space::MD),
                right: Val::Px(space::MD),
                padding: UiRect::axes(Val::Px(space::MD), Val::Px(space::XS)),
                border_radius: BorderRadius::all(Val::Px(radius::PILL)),
                ..default()
            },
            BackgroundColor(palette::CREAM.with_alpha(0.7)),
            GlobalZIndex(56),
        ))
        .with_children(|c| {
            c.spawn(theme::text("Esc  skip", font::CAPTION, palette::INK_SOFT));
        });
}

fn end(
    mut commands: Commands,
    mut memory: ResMut<MemoryBloom>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
) {
    commands.remove_resource::<Director>();
    commands.remove_resource::<crate::game::Touring>();
    memory.active = false;
    set_cursor(&mut windows, true);
}

fn show_hud<const VISIBLE: bool>(mut roots: Query<&mut Visibility, With<HudRoot>>) {
    for mut v in &mut roots {
        *v = if VISIBLE {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Esc / Enter / Space during the celebration or the tour: straight home.
fn skip(keys: Res<ButtonInput<KeyCode>>, bindings: Res<Bindings>, mut director: ResMut<Director>) {
    let pressed = Bindings::just_pressed(&keys, &bindings.pause)
        || Bindings::just_pressed(&keys, &bindings.confirm)
        || Bindings::just_pressed(&keys, &bindings.jump);
    if !pressed {
        return;
    }
    match director.phase {
        Phase::Celebrate | Phase::Fly(_) | Phase::Build(Stop::Tour(_), _) => {
            director.phase = Phase::Out(Stop::Home);
            director.t = 0.0;
        }
        _ => {}
    }
}

/// A point along a region's footpath at `u` (0 = start, 1 = end).
fn along(path: &[Vec2], u: f32) -> Vec2 {
    let f = u.clamp(0.0, 1.0) * (path.len() - 1) as f32;
    let i = (f.floor() as usize).min(path.len() - 2);
    path[i].lerp(path[i + 1], f - i as f32)
}

#[allow(clippy::too_many_arguments)]
fn direct(
    time: Res<Time<Real>>,
    mut director: ResMut<Director>,
    mut active: ResMut<ActiveRegion>,
    mut memory: ResMut<MemoryBloom>,
    scene: Option<Res<RegionScene>>,
    content: Res<crate::game::Content>,
    player: Option<Res<Player>>,
    mut cameras: Query<&mut Transform, With<MainCamera>>,
    mut fade: Query<&mut BackgroundColor, With<Fade>>,
) {
    let dt = time.delta_secs();
    director.t += dt;
    let t = director.t;
    let regions = content.regions.len();
    let Ok(mut camera) = cameras.single_mut() else {
        return;
    };
    let eye = player.as_ref().map(|p| {
        let pos = p.body.position + Vec3::Y * content.player.eye_height;
        Transform::from_translation(pos).with_rotation(Quat::from_euler(
            EulerRot::YXZ,
            p.yaw,
            p.pitch,
            0.0,
        ))
    });
    let mut alpha = 0.0;
    match director.phase {
        Phase::Celebrate => {
            // Lift up and away, turning slowly, looking down at the spot.
            let k = (t / CELEBRATE_SECS).min(1.0);
            let e = k * k * (3.0 - 2.0 * k);
            let from = director.from;
            let back = from.rotation * Vec3::Z;
            let pos = from.translation
                + Vec3::Y * 5.0 * e
                + back.with_y(0.0).normalize_or_zero() * 4.0 * e;
            let target = from.translation - Vec3::Y * 1.2 + from.forward().with_y(0.0) * 1.5;
            let look = Transform::from_translation(pos).looking_at(target, Vec3::Y);
            camera.translation = pos;
            camera.rotation = from.rotation.slerp(look.rotation, e);
            if t >= CELEBRATE_SECS {
                director.phase = Phase::Out(Stop::Tour(0));
                director.t = 0.0;
            }
        }
        Phase::Out(stop) => {
            alpha = (t / FADE_OUT_SECS).min(1.0);
            if t >= FADE_OUT_SECS {
                let region = match stop {
                    Stop::Tour(r) => r,
                    Stop::Home => director.home,
                };
                memory.active = matches!(stop, Stop::Tour(_));
                if active.0 != region {
                    active.0 = region;
                }
                director.phase = Phase::Build(stop, 0);
                director.t = 0.0;
            }
        }
        Phase::Build(stop, frames) => {
            alpha = 1.0;
            let region = match stop {
                Stop::Tour(r) => r,
                Stop::Home => director.home,
            };
            if scene.as_ref().is_some_and(|s| s.region == region) {
                if frames >= 3 {
                    director.phase = match stop {
                        Stop::Tour(r) => Phase::Fly(r),
                        Stop::Home => Phase::Return,
                    };
                    director.t = 0.0;
                } else {
                    director.phase = Phase::Build(stop, frames + 1);
                }
            }
        }
        Phase::Fly(r) => {
            alpha = 1.0 - (t / FADE_IN_SECS).min(1.0);
            if let Some(scene) = &scene {
                let layout = &scene.layout;
                let u = 0.08 + 0.62 * (t / FLY_SECS).min(1.0);
                let p = along(&layout.path, u);
                let ahead = along(&layout.path, u + 0.18);
                let side = (ahead - p).perp().normalize_or_zero() * 4.0;
                let cam = p + side;
                let ground = layout.terrain.height_at(cam);
                let pos = Vec3::new(cam.x, ground + 5.0, cam.y);
                let look = Vec3::new(ahead.x, layout.terrain.height_at(ahead) + 0.5, ahead.y);
                *camera = Transform::from_translation(pos).looking_at(look, Vec3::Y);
            }
            if t >= FLY_SECS {
                director.phase = Phase::Out(if r + 1 < regions {
                    Stop::Tour(r + 1)
                } else {
                    Stop::Home
                });
                director.t = 0.0;
                // Keep the last frame's fade (0) until Out starts.
            }
        }
        Phase::Return => {
            alpha = 1.0 - (t / FADE_IN_SECS).min(1.0);
            if let Some(eye) = eye {
                *camera = eye;
            }
            if t >= FADE_IN_SECS {
                director.phase = Phase::Summary;
                director.t = 0.0;
            }
        }
        Phase::Summary => {}
    }
    for mut bg in &mut fade {
        bg.0 = bg.0.with_alpha(alpha);
    }
}

type TitleTexts<'w, 's> = Query<
    'w,
    's,
    (&'static mut TextColor, &'static mut TextShadow),
    (With<Title>, Without<Caption>, Without<CaptionDetail>),
>;
type CaptionTexts<'w, 's, A, B> = Query<
    'w,
    's,
    (
        &'static mut Text,
        &'static mut TextColor,
        &'static mut TextShadow,
    ),
    (With<A>, Without<B>),
>;

#[allow(clippy::too_many_arguments)]
fn animate_overlay(
    mut commands: Commands,
    mut director: ResMut<Director>,
    session: Res<Session>,
    content: Res<crate::game::Content>,
    mut windows: Query<&mut CursorOptions, With<PrimaryWindow>>,
    mut titles: TitleTexts,
    mut caption: CaptionTexts<Caption, CaptionDetail>,
    mut detail: CaptionTexts<CaptionDetail, Caption>,
    mut hint: Query<&mut Visibility, With<SkipHint>>,
    bindings: Res<Bindings>,
) {
    let t = director.t;
    // Title: in during the celebration, out before the tour.
    let title_alpha = match director.phase {
        Phase::Celebrate => {
            ((t - 0.8) / 1.2).clamp(0.0, 1.0) * (1.0 - ((t - 4.3) / 1.0).clamp(0.0, 1.0))
        }
        _ => 0.0,
    };
    for (mut c, mut sh) in &mut titles {
        set_alpha(&mut c, &mut sh, title_alpha);
    }
    let (name, line, cap_alpha) = match director.phase {
        Phase::Fly(r) => {
            let def = &content.regions[r];
            let a = ((t - 0.6) / 0.8).clamp(0.0, 1.0)
                * (1.0 - ((t - (FLY_SECS - 1.0)) / 0.8).clamp(0.0, 1.0));
            (
                def.name.clone(),
                format!("{} flowers, every one gathered", def.flower_total),
                a,
            )
        }
        _ => (String::new(), String::new(), 0.0),
    };
    for (mut text, mut c, mut sh) in &mut caption {
        if text.0 != name && !name.is_empty() {
            text.0 = name.clone();
        }
        set_alpha(&mut c, &mut sh, cap_alpha);
    }
    for (mut text, mut c, mut sh) in &mut detail {
        if text.0 != line && !line.is_empty() {
            text.0 = line.clone();
        }
        set_alpha(&mut c, &mut sh, cap_alpha);
    }
    let touring = !matches!(director.phase, Phase::Summary | Phase::Return);
    for mut v in &mut hint {
        let want = if touring {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *v != want {
            *v = want;
        }
    }
    if director.phase == Phase::Summary && !director.summary_spawned {
        director.summary_spawned = true;
        set_cursor(&mut windows, true);
        spawn_summary(&mut commands, &session.game, &bindings);
    }
}

fn fly_petals(time: Res<Time>, mut petals: Query<(&mut GoldPetal, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut p, mut t) in &mut petals {
        p.velocity.y -= 2.2 * dt;
        p.velocity *= 1.0 - 0.6 * dt;
        p.velocity.y = p.velocity.y.max(-0.6);
        t.translation += p.velocity * dt;
        let s = p.spin * dt;
        t.rotate(Quat::from_euler(EulerRot::XYZ, s.x, s.y, s.z));
    }
}

/// "1,900" style thousands.
pub fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::new();
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

fn duration(secs: f64) -> String {
    let m = (secs / 60.0).round() as u64;
    format!("{} h {:02} min", m / 60, m % 60)
}

fn spawn_summary(commands: &mut Commands, game: &Game, bindings: &Bindings) {
    let collection = format!(
        "Collection ({})",
        bindings.label(crate::input::Action::Collection)
    );
    let data = game.data();
    let Some(s) = game.journey_summary().cloned() else {
        return;
    };
    let walked = if s.distance_walked_m >= 1000.0 {
        format!("{:.1} km", s.distance_walked_m / 1000.0)
    } else {
        format!("{:.0} m", s.distance_walked_m)
    };
    let levels = |n: u32| format!("{n} level{}", if n == 1 { "" } else { "s" });
    let rows: [(&str, String); 10] = [
        (
            "Flowers picked",
            format!(
                "{}  ({} rare)",
                thousands(s.flowers_harvested),
                s.rare_flowers
            ),
        ),
        (
            "Bouquets bound",
            format!(
                "{}  ({} sold)",
                thousands(s.bouquets_crafted),
                thousands(s.bouquets_sold)
            ),
        ),
        (
            "Coins earned",
            format!(
                "{}  ({} spent)",
                thousands(s.coins_earned),
                thousands(s.coins_spent)
            ),
        ),
        ("Time played", duration(s.playtime_secs)),
        ("Distance walked", walked),
        ("Level", s.level.to_string()),
        (
            "Tools / regions",
            format!(
                "{} / {}   {} / {}",
                s.tools,
                data.tools.len(),
                s.regions,
                data.regions.len()
            ),
        ),
        ("Recipes", format!("{} / {}", s.recipes, data.recipes.len())),
        (
            "Upgrades / skills",
            format!("{}   {}", levels(s.upgrade_levels), levels(s.skill_levels)),
        ),
        (
            "Achievements",
            format!("{} / {}", s.achievements, data.achievements.len()),
        ),
    ];
    commands
        .spawn((
            DespawnOnExit(PlayState::Finale),
            SummaryCard,
            Node {
                position_type: PositionType::Absolute,
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            BackgroundColor(palette::SCRIM),
            GlobalZIndex(57),
        ))
        .with_children(|root| {
            root.spawn(theme::card(Node {
                width: Val::Px(1080.0),
                max_width: Val::Percent(96.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(space::XL)),
                row_gap: Val::Px(space::MD),
                border: UiRect::all(Val::Px(2.0)),
                ..default()
            }))
            .with_children(|card| {
                card.spawn(theme::text("Bloom & Gather", font::BODY, palette::INK_SOFT));
                card.spawn(theme::text("Your journey", font::HEADING, palette::INK));
                card.spawn(theme::text(
                    "Every flower in the valley was picked, every species found, every bouquet made.",
                    font::BODY,
                    palette::INK_SOFT,
                ));
                // Two columns of numbers.
                card.spawn(Node {
                    column_gap: Val::Px(space::XXL),
                    margin: UiRect::vertical(Val::Px(space::SM)),
                    ..default()
                })
                .with_children(|cols| {
                    for half in rows.chunks(5) {
                        cols.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            row_gap: Val::Px(6.0),
                            width: Val::Px(440.0),
                            ..default()
                        })
                        .with_children(|col| {
                            for (label, value) in half {
                                col.spawn(Node {
                                    justify_content: JustifyContent::SpaceBetween,
                                    ..default()
                                })
                                .with_children(|row| {
                                    row.spawn(theme::text(*label, font::BODY, palette::INK_SOFT));
                                    row.spawn(theme::text(value.clone(), font::BODY, palette::INK));
                                });
                            }
                        });
                    }
                });
                card.spawn(theme::text(
                    format!("The full collection: {} species", data.species.len()),
                    font::BODY,
                    palette::INK,
                ));
                card.spawn(Node {
                    flex_wrap: FlexWrap::Wrap,
                    justify_content: JustifyContent::Center,
                    column_gap: Val::Px(space::SM),
                    row_gap: Val::Px(space::SM),
                    max_width: Val::Px(980.0),
                    ..default()
                })
                .with_children(|grid| {
                    for sp in &data.species {
                        grid.spawn(Node {
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Center,
                            width: Val::Px(100.0),
                            row_gap: Val::Px(3.0),
                            ..default()
                        })
                        .with_children(|cell| {
                            super::collection::swatch(cell, sp, true, 30.0);
                            cell.spawn((
                                theme::text(sp.name.clone(), font::CAPTION, palette::INK_SOFT),
                                TextLayout::new_with_justify(Justify::Center),
                            ));
                        });
                    }
                });
                card.spawn(Node {
                    column_gap: Val::Px(space::MD),
                    margin: UiRect::top(Val::Px(space::MD)),
                    ..default()
                })
                .with_children(|b| {
                    spawn_button(b, "Keep wandering", ButtonVariant::Primary, Act::Wander);
                    spawn_button(b, &collection, ButtonVariant::Secondary, Act::Collection);
                    spawn_button(b, "Title screen", ButtonVariant::Secondary, Act::Title);
                });
                card.spawn(theme::text(
                    "Enter: keep wandering. In free roam nothing new is asked of you; this summary stays in the journal.",
                    font::CAPTION,
                    palette::INK_SOFT,
                ));
            });
        });
}

#[allow(clippy::too_many_arguments)]
fn summary_buttons(
    director: Res<Director>,
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    pressed: Query<(&Interaction, &Act), Changed<Interaction>>,
    mut session: ResMut<Session>,
    mut next: ResMut<NextState<PlayState>>,
    mut next_app: ResMut<NextState<AppState>>,
    mut sfx: MessageWriter<PlaySfx>,
) {
    if director.phase != Phase::Summary || !director.summary_spawned {
        return;
    }
    let mut act = None;
    for (interaction, a) in &pressed {
        if *interaction == Interaction::Pressed {
            act = Some(*a);
        }
    }
    // Keys only once the summary has been up a moment (no accidental skip).
    if director.t > 0.4 {
        if Bindings::just_pressed(&keys, &bindings.confirm) {
            act = Some(Act::Wander);
        } else if Bindings::just_pressed(&keys, &bindings.collection) {
            act = Some(Act::Collection);
        }
    }
    let Some(act) = act else { return };
    sfx.write(PlaySfx::new(Sfx::Click));
    session.mark_finale_seen();
    match act {
        Act::Wander => next.set(PlayState::Playing),
        Act::Collection => next.set(PlayState::Collection),
        Act::Title => next_app.set(AppState::MainMenu),
    }
}
