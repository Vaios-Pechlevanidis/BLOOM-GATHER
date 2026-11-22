//! Maps: a minimap in the corner of the HUD, and the world map (M).
//!
//! Both draw a region from its generated layout — ground, footpath, water,
//! trees, the gates, the workshop and every flower still blooming — into a
//! small image, refreshed as flowers are picked. The world map lists the
//! five regions along the valley with their state (here / open / what they
//! still need, flowers left) and travels to any open region (Enter or the
//! Travel button). Game time is paused while it is open.

use std::collections::HashMap;

use bevy::asset::RenderAssetUsages;
use bevy::prelude::*;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use bloom_core::game::GameEvent;
use bloom_core::world::{FlowerField, GateSide, LandmarkKind, RegionLayout};
use bloom_core::Game;

use super::theme::{self, font, palette, radius, space};
use super::widgets::{spawn_button, ButtonVariant};
use crate::audio::{PlaySfx, Sfx};
use crate::game::{ActiveRegion, AppState, EventsForwarded, GameEventMsg, PlayState, Session};
use crate::input::Bindings;
use crate::player::Player;
use crate::rendering::color;
use crate::travel::{requirements_text, Arrival, TravelTo};

pub struct MapPlugin;

impl Plugin for MapPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapImages>()
            .init_resource::<MapView>()
            .add_systems(OnEnter(AppState::InGame), spawn_minimap)
            .add_systems(OnEnter(PlayState::Map), spawn_screen)
            .add_systems(
                Update,
                (
                    toggle.run_if(in_state(AppState::InGame)),
                    mark_stale.after(EventsForwarded),
                    (refresh_images, place_markers, minimap_setting)
                        .chain()
                        .run_if(in_state(AppState::InGame)),
                    (keys, buttons, rebuild)
                        .chain()
                        .run_if(in_state(PlayState::Map)),
                ),
            );
    }
}

/// Pixels per metre in map images.
const SCALE: f32 = 1.6;
const MINIMAP_WIDTH: f32 = 230.0;
const PREVIEW_WIDTH: f32 = 560.0;

/// One map image per region drawn so far.
#[derive(Resource, Default)]
struct MapImages {
    images: HashMap<usize, MapImage>,
}

struct MapImage {
    handle: Handle<Image>,
    /// Ground, path, water, trees and buildings (no flowers).
    base: Vec<u8>,
    size: UVec2,
    /// World position of pixel (0, 0) and the world size covered.
    origin: Vec2,
    extent: Vec2,
    stale: bool,
}

impl MapImage {
    /// Map position (0..1) of a world point.
    fn uv(&self, p: Vec2) -> Vec2 {
        ((p - self.origin) / self.extent).clamp(Vec2::ZERO, Vec2::ONE)
    }
}

#[derive(Resource, Default)]
struct MapView {
    selected: usize,
    stale: bool,
}

#[derive(Component)]
struct MinimapImage;

#[derive(Component)]
struct MinimapFrame;

/// Player dot and the little pip showing where they face.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
enum Marker {
    Player,
    Facing,
}

#[derive(Component, Clone, Copy)]
enum Act {
    Select(usize),
    Travel,
    Close,
}

#[derive(Component)]
struct Body;

fn rgba(c: Color) -> [u8; 4] {
    let s = c.to_srgba();
    [
        (s.red * 255.0) as u8,
        (s.green * 255.0) as u8,
        (s.blue * 255.0) as u8,
        255,
    ]
}

fn shade(c: [u8; 4], f: f32) -> [u8; 4] {
    [
        (c[0] as f32 * f).min(255.0) as u8,
        (c[1] as f32 * f).min(255.0) as u8,
        (c[2] as f32 * f).min(255.0) as u8,
        255,
    ]
}

/// Ground, path, water, trees, rocks, landmarks and open gates.
fn draw_base(game: &Game, region: usize, layout: &RegionLayout) -> (Vec<u8>, UVec2, Vec2, Vec2) {
    let def = &game.data().regions[region];
    let origin = layout.bounds_min - Vec2::splat(2.0);
    let extent = layout.bounds_max - layout.bounds_min + Vec2::splat(4.0);
    let size = (extent * SCALE).ceil().as_uvec2();
    let ground = rgba(color(def.palette.ground));
    let path = rgba(color(def.palette.path));
    let water = if def.scenery.atmosphere.night {
        [44, 62, 104, 255]
    } else {
        [118, 170, 196, 255]
    };
    let fence = [132, 102, 76, 255];
    let mut px = vec![0u8; (size.x * size.y * 4) as usize];
    let world = |x: u32, y: u32| origin + (Vec2::new(x as f32, y as f32) + 0.5) / SCALE;
    for y in 0..size.y {
        for x in 0..size.x {
            let p = world(x, y);
            let c = if !layout.in_bounds(p, -0.4) {
                fence
            } else if layout.water_level_at(p).is_some() {
                water
            } else if layout.distance_to_path(p) < layout.path_half_width {
                path
            } else {
                let h = layout.terrain.height_at(p);
                shade(ground, 0.86 + (h * 0.04).clamp(-0.12, 0.16))
            };
            let i = ((y * size.x + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&c);
        }
    }
    let mut dot = |p: Vec2, r: f32, c: [u8; 4]| {
        let center = (p - origin) * SCALE;
        let rr = (r * SCALE).max(0.8);
        let (x0, x1) = (
            (center.x - rr).floor() as i32,
            (center.x + rr).ceil() as i32,
        );
        let (y0, y1) = (
            (center.y - rr).floor() as i32,
            (center.y + rr).ceil() as i32,
        );
        for y in y0..=y1 {
            for x in x0..=x1 {
                if x < 0 || y < 0 || x >= size.x as i32 || y >= size.y as i32 {
                    continue;
                }
                if Vec2::new(x as f32 + 0.5, y as f32 + 0.5).distance(center) <= rr {
                    let i = ((y as u32 * size.x + x as u32) * 4) as usize;
                    px[i..i + 4].copy_from_slice(&c);
                }
            }
        }
    };
    for b in &layout.bridges {
        for k in 0..=12 {
            dot(b.point(k as f32 / 12.0), b.half_width, [176, 140, 104, 255]);
        }
    }
    let leaf = shade(rgba(color(def.palette.ground_dark)), 0.75);
    for t in &layout.trees {
        dot(t.position, t.canopy_radius * 0.8, leaf);
    }
    for r in &layout.rocks {
        dot(r.position, r.radius, [150, 146, 140, 255]);
    }
    for l in &layout.landmarks {
        let (r, c) = match l.kind {
            LandmarkKind::Workshop => (4.0, [198, 120, 96, 255]),
            LandmarkKind::Well => (1.2, [170, 166, 160, 255]),
            LandmarkKind::Bench | LandmarkKind::Signpost => (0.7, [150, 112, 80, 255]),
            LandmarkKind::Lantern => (0.6, [250, 220, 140, 255]),
        };
        dot(l.position, r, c);
    }
    for side in [GateSide::North, GateSide::South] {
        if let Some(to) = game.gate_destination(region, side) {
            let c = if game.is_region_unlocked(to) {
                [246, 236, 200, 255]
            } else {
                [96, 84, 72, 255]
            };
            dot(layout.gate(side).position, 2.2, c);
        }
    }
    (px, size, origin, extent)
}

/// The base plus every flower still blooming.
fn draw_flowers(game: &Game, region: usize, field: &FlowerField, img: &MapImage) -> Vec<u8> {
    let mut px = img.base.clone();
    let data = game.data();
    for i in 0..field.len() as u32 {
        if game.is_harvested(region, i) {
            continue;
        }
        let s = &data.species[field.species(i)];
        let c = rgba(color(s.petal_color));
        let p = (field.position(i) - img.origin) * SCALE;
        for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
            let (x, y) = (p.x as i32 + dx, p.y as i32 + dy);
            if x >= 0 && y >= 0 && (x as u32) < img.size.x && (y as u32) < img.size.y {
                let k = ((y as u32 * img.size.x + x as u32) * 4) as usize;
                px[k..k + 4].copy_from_slice(&c);
            }
        }
    }
    px
}

/// The image for `region`, drawn on first use.
fn image_for(
    images: &mut MapImages,
    assets: &mut Assets<Image>,
    session: &mut Session,
    region: usize,
) -> Handle<Image> {
    if let Some(img) = images.images.get(&region) {
        return img.handle.clone();
    }
    let runtime = session.region(region).clone();
    let (base, size, origin, extent) = draw_base(&session.game, region, &runtime.layout);
    let mut img = MapImage {
        handle: Handle::default(),
        base,
        size,
        origin,
        extent,
        stale: false,
    };
    let px = draw_flowers(&session.game, region, &runtime.field, &img);
    img.handle = assets.add(Image::new(
        Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        px,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::MAIN_WORLD | RenderAssetUsages::RENDER_WORLD,
    ));
    let handle = img.handle.clone();
    images.images.insert(region, img);
    handle
}

fn aspect(images: &MapImages, region: usize) -> f32 {
    images
        .images
        .get(&region)
        .map_or(0.7, |i| i.size.y as f32 / i.size.x as f32)
}

fn spawn_minimap(
    mut commands: Commands,
    mut images: ResMut<MapImages>,
    mut assets: ResMut<Assets<Image>>,
    mut session: ResMut<Session>,
    active: Res<ActiveRegion>,
) {
    // Gates may have opened since these were drawn.
    images.images.clear();
    let handle = image_for(&mut images, &mut assets, &mut session, active.0);
    let height = MINIMAP_WIDTH * aspect(&images, active.0);
    commands
        .spawn((
            StateScoped(AppState::InGame),
            crate::ui::HudRoot,
            MinimapFrame,
            Node {
                position_type: PositionType::Absolute,
                right: Val::Px(space::MD),
                bottom: Val::Px(56.0),
                width: Val::Px(MINIMAP_WIDTH + 8.0),
                height: Val::Px(height + 8.0),
                padding: UiRect::all(Val::Px(4.0)),
                ..default()
            },
            BackgroundColor(palette::CREAM.with_alpha(0.85)),
            BorderRadius::all(Val::Px(radius::SM)),
            Pickable::IGNORE,
        ))
        .with_children(|frame| {
            frame
                .spawn((
                    MinimapImage,
                    ImageNode::new(handle),
                    Node {
                        width: Val::Percent(100.0),
                        height: Val::Percent(100.0),
                        ..default()
                    },
                ))
                .with_children(|map| {
                    for (marker, size, col) in [
                        (Marker::Facing, 5.0, palette::INK),
                        (Marker::Player, 9.0, Color::srgb(0.95, 0.4, 0.35)),
                    ] {
                        map.spawn((
                            marker,
                            Node {
                                position_type: PositionType::Absolute,
                                width: Val::Px(size),
                                height: Val::Px(size),
                                border: UiRect::all(Val::Px(1.5)),
                                ..default()
                            },
                            BackgroundColor(col),
                            BorderColor(palette::CREAM),
                            BorderRadius::all(Val::Px(size)),
                        ));
                    }
                });
        });
}

/// The minimap can be switched off in the settings.
fn minimap_setting(
    settings: Res<crate::game::Settings>,
    mut frames: Query<&mut Node, With<MinimapFrame>>,
) {
    let display = if settings.show_minimap {
        Display::Flex
    } else {
        Display::None
    };
    for mut node in &mut frames {
        if node.display != display {
            node.display = display;
        }
    }
}

fn toggle(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    state: Res<State<PlayState>>,
    mut next: ResMut<NextState<PlayState>>,
    journey: Option<Res<crate::travel::Journey>>,
) {
    if !Bindings::just_pressed(&keys, &bindings.map) || journey.is_some() {
        return;
    }
    match state.get() {
        PlayState::Playing => next.set(PlayState::Map),
        PlayState::Map => next.set(PlayState::Playing),
        _ => {}
    }
}

/// Picked flowers, opened gates and travel make images out of date.
fn mark_stale(
    mut events: EventReader<GameEventMsg>,
    mut images: ResMut<MapImages>,
    mut view: ResMut<MapView>,
) {
    for GameEventMsg(e) in events.read() {
        match *e {
            GameEvent::FlowerHarvested { region, .. } => {
                if let Some(img) = images.images.get_mut(&region) {
                    img.stale = true;
                }
            }
            GameEvent::RegionUnlocked { .. } => {
                // Gate colours change: redraw from scratch.
                images.images.clear();
                view.stale = true;
            }
            _ => {}
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn refresh_images(
    time: Res<Time<Real>>,
    mut last: Local<f32>,
    mut images: ResMut<MapImages>,
    mut assets: ResMut<Assets<Image>>,
    mut session: ResMut<Session>,
    active: Res<ActiveRegion>,
    mut minimap: Query<&mut ImageNode, With<MinimapImage>>,
    mut frame: Query<&mut Node, With<MinimapFrame>>,
) {
    // The minimap shows the region you are in (it changes when you travel).
    let handle = match images.images.get(&active.0) {
        Some(img) => img.handle.clone(),
        None => image_for(&mut images, &mut assets, &mut session, active.0),
    };
    for mut node in &mut minimap {
        if node.image != handle {
            node.image = handle.clone();
            let h = MINIMAP_WIDTH * aspect(&images, active.0) + 8.0;
            for mut f in &mut frame {
                f.height = Val::Px(h);
            }
        }
    }
    // Redraw picked flowers at most a few times a second.
    let now = time.elapsed_secs();
    if now - *last < 0.25 {
        return;
    }
    *last = now;
    let stale: Vec<usize> = images
        .images
        .iter()
        .filter(|(_, i)| i.stale)
        .map(|(&r, _)| r)
        .collect();
    for region in stale {
        let field = session.region(region).field.clone();
        let img = &images.images[&region];
        let px = draw_flowers(&session.game, region, &field, img);
        if let Some(image) = assets.get_mut(&img.handle) {
            image.data = Some(px);
        }
        if let Some(img) = images.images.get_mut(&region) {
            img.stale = false;
        }
    }
}

fn place_markers(
    images: Res<MapImages>,
    active: Res<ActiveRegion>,
    player: Option<Res<Player>>,
    mut markers: Query<(&Marker, &mut Node)>,
) {
    let (Some(player), Some(img)) = (player, images.images.get(&active.0)) else {
        return;
    };
    let w = MINIMAP_WIDTH;
    let h = MINIMAP_WIDTH * aspect(&images, active.0);
    let at = player.body.ground_position();
    // Yaw 0 faces −Z, which is up on the map.
    let facing = Vec2::new(-player.yaw.sin(), -player.yaw.cos());
    for (marker, mut node) in &mut markers {
        let (p, size) = match marker {
            Marker::Player => (at, 9.0),
            Marker::Facing => (at + facing * 6.0 / SCALE * 2.0, 5.0),
        };
        let uv = img.uv(p);
        let left = Val::Px(uv.x * w - size / 2.0);
        let top = Val::Px(uv.y * h - size / 2.0);
        if node.left != left || node.top != top {
            node.left = left;
            node.top = top;
        }
    }
}

fn spawn_screen(
    mut commands: Commands,
    mut view: ResMut<MapView>,
    mut images: ResMut<MapImages>,
    active: Res<ActiveRegion>,
    bindings: Res<Bindings>,
) {
    let close = format!("Close ({})", bindings.label(crate::input::Action::Map));
    // Redraw from scratch so gates show what is open right now.
    images.images.clear();
    view.selected = active.0;
    view.stale = true;
    commands
        .spawn((
            StateScoped(PlayState::Map),
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
                    h.spawn(theme::text("The Valley", font::HEADING, palette::INK));
                    h.spawn(theme::text(
                        "Left / Right choose   Enter travel",
                        font::CAPTION,
                        palette::INK_SOFT,
                    ));
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
                    spawn_button(f, &close, ButtonVariant::Compact, Act::Close);
                });
            });
        });
}

fn keys(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    session: Res<Session>,
    mut view: ResMut<MapView>,
    active: Res<ActiveRegion>,
    mut travel: EventWriter<TravelTo>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: EventWriter<PlaySfx>,
) {
    let n = session.data().regions.len();
    let before = view.selected;
    if keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD) {
        view.selected = (view.selected + 1).min(n - 1);
    }
    if keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA) {
        view.selected = view.selected.saturating_sub(1);
    }
    if view.selected != before {
        view.stale = true;
        sfx.write(PlaySfx::new(Sfx::Click));
    }
    if Bindings::just_pressed(&keys, &bindings.confirm) {
        go(
            &session,
            view.selected,
            active.0,
            &mut travel,
            &mut next,
            &mut sfx,
        );
    }
}

/// Travel to `to` (refused with a sound if it is locked or where you are).
fn go(
    session: &Session,
    to: usize,
    here: usize,
    travel: &mut EventWriter<TravelTo>,
    next: &mut NextState<PlayState>,
    sfx: &mut EventWriter<PlaySfx>,
) {
    if to == here || !session.is_region_unlocked(to) {
        sfx.write(PlaySfx::new(Sfx::Deny));
        return;
    }
    sfx.write(PlaySfx::new(Sfx::Click));
    next.set(PlayState::Playing);
    travel.write(TravelTo {
        region: to,
        arrive: Arrival::Start,
    });
}

#[allow(clippy::too_many_arguments)]
fn buttons(
    pressed: Query<(&Interaction, &Act), Changed<Interaction>>,
    session: Res<Session>,
    mut view: ResMut<MapView>,
    active: Res<ActiveRegion>,
    mut travel: EventWriter<TravelTo>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: EventWriter<PlaySfx>,
) {
    for (interaction, act) in &pressed {
        if *interaction != Interaction::Pressed {
            continue;
        }
        match *act {
            Act::Select(r) => {
                sfx.write(PlaySfx::new(Sfx::Click));
                view.selected = r;
                view.stale = true;
            }
            Act::Travel => go(
                &session,
                view.selected,
                active.0,
                &mut travel,
                &mut next,
                &mut sfx,
            ),
            Act::Close => {
                sfx.write(PlaySfx::new(Sfx::Click));
                next.set(PlayState::Playing);
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn rebuild(
    mut commands: Commands,
    mut view: ResMut<MapView>,
    mut session: ResMut<Session>,
    mut images: ResMut<MapImages>,
    mut assets: ResMut<Assets<Image>>,
    active: Res<ActiveRegion>,
    body: Query<(Entity, Option<&Children>), With<Body>>,
) {
    let Ok((body, children)) = body.single() else {
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
    let sel = view.selected;
    // Only regions you have walked into are drawn; others are a mystery.
    let preview = session
        .has_visited(sel)
        .then(|| image_for(&mut images, &mut assets, &mut session, sel));
    let preview_aspect = aspect(&images, sel);
    let game = &session.game;
    let data = game.data();
    commands.entity(body).with_children(|body| {
        // The chain of regions, west to east along the valley.
        body.spawn(Node {
            column_gap: Val::Px(space::SM),
            align_items: AlignItems::Stretch,
            ..default()
        })
        .with_children(|row| {
            for r in 0..data.regions.len() {
                if r > 0 {
                    row.spawn((
                        Node {
                            align_self: AlignSelf::Center,
                            ..default()
                        },
                        theme::text(">", font::TITLE, palette::INK_SOFT),
                    ));
                }
                region_card(row, game, r, r == active.0, r == sel);
            }
        });
        // The selected region in detail.
        body.spawn(Node {
            column_gap: Val::Px(space::LG),
            align_items: AlignItems::FlexStart,
            min_height: Val::Px(PREVIEW_WIDTH * 0.7),
            ..default()
        })
        .with_children(|row| {
            match preview {
                Some(handle) => {
                    row.spawn((
                        ImageNode::new(handle),
                        Node {
                            width: Val::Px(PREVIEW_WIDTH),
                            height: Val::Px(PREVIEW_WIDTH * preview_aspect),
                            border: UiRect::all(Val::Px(2.0)),
                            ..default()
                        },
                        BorderColor(palette::BEIGE_DEEP),
                        BorderRadius::all(Val::Px(radius::SM)),
                    ));
                }
                None => {
                    row.spawn((
                        Node {
                            width: Val::Px(PREVIEW_WIDTH),
                            height: Val::Px(PREVIEW_WIDTH * 0.6),
                            justify_content: JustifyContent::Center,
                            align_items: AlignItems::Center,
                            ..default()
                        },
                        BackgroundColor(palette::PARCHMENT),
                        BorderRadius::all(Val::Px(radius::SM)),
                    ))
                    .with_children(|blank| {
                        blank.spawn(theme::text(
                            "Not explored yet",
                            font::BODY,
                            palette::INK_SOFT,
                        ));
                    });
                }
            }
            details(row, game, sel, active.0);
        });
    });
}

fn region_card(row: &mut ChildSpawnerCommands, game: &Game, r: usize, here: bool, selected: bool) {
    let def = &game.data().regions[r];
    let open = game.is_region_unlocked(r);
    let left = game.remaining_in_region(r);
    let accent = color(def.palette.accent);
    row.spawn((
        Button,
        Act::Select(r),
        Node {
            flex_direction: FlexDirection::Column,
            flex_grow: 1.0,
            flex_basis: Val::Px(0.0),
            padding: UiRect::all(Val::Px(space::SM)),
            row_gap: Val::Px(3.0),
            border: UiRect::all(Val::Px(if selected { 3.0 } else { 1.5 })),
            ..default()
        },
        BackgroundColor(if open { palette::CREAM } else { palette::BEIGE }),
        BorderColor(if selected {
            palette::SAGE_DARK
        } else {
            palette::BEIGE_DEEP
        }),
        BorderRadius::all(Val::Px(radius::MD)),
    ))
    .with_children(|c| {
        c.spawn((
            Node {
                height: Val::Px(6.0),
                ..default()
            },
            BackgroundColor(if open {
                accent
            } else {
                accent.with_alpha(0.35)
            }),
            BorderRadius::all(Val::Px(3.0)),
        ));
        c.spawn(theme::text(def.name.clone(), font::BODY, palette::INK));
        let status = if here {
            "You are here".to_string()
        } else if !open {
            format!("Locked (level {})", def.unlock.level)
        } else if left == 0 {
            "Complete".to_string()
        } else if !game.has_visited(r) {
            "Open, not visited".to_string()
        } else {
            "Open".to_string()
        };
        c.spawn(theme::text(
            status,
            font::CAPTION,
            if here {
                palette::SAGE_DEEP
            } else {
                palette::INK_SOFT
            },
        ));
        c.spawn(theme::text(
            format!("{left} / {} flowers left", def.flower_total),
            font::CAPTION,
            palette::INK_SOFT,
        ));
    });
}

fn details(row: &mut ChildSpawnerCommands, game: &Game, r: usize, here: usize) {
    let def = &game.data().regions[r];
    let open = game.is_region_unlocked(r);
    row.spawn(Node {
        flex_direction: FlexDirection::Column,
        row_gap: Val::Px(space::SM),
        flex_grow: 1.0,
        ..default()
    })
    .with_children(|col| {
        col.spawn(theme::text(def.name.clone(), font::TITLE, palette::INK));
        col.spawn(theme::text(
            def.description.clone(),
            font::BODY,
            palette::INK_SOFT,
        ));
        let left = game.remaining_in_region(r);
        col.spawn(theme::text(
            format!(
                "{} of {} flowers picked here",
                def.flower_total - left,
                def.flower_total
            ),
            font::BODY,
            palette::INK,
        ));
        // What grows here (names only once discovered).
        let mut names: Vec<String> = def
            .flowers
            .iter()
            .map(|f| {
                let idx = game.data().species_index(&f.species).unwrap_or(0);
                if game.is_discovered(idx) {
                    game.data().species[idx].name.clone()
                } else {
                    "?".to_string()
                }
            })
            .collect();
        names.dedup();
        col.spawn(theme::text(
            format!("Grows here: {}", names.join(", ")),
            font::CAPTION,
            palette::INK_SOFT,
        ));
        if def.workshop {
            col.spawn(theme::text(
                "The florist's workshop is here: workbench, tool shelf and storage.",
                font::CAPTION,
                palette::SAGE_DEEP,
            ));
        }
        if r == here {
            col.spawn(theme::text("You are here.", font::BODY, palette::SAGE_DEEP));
        } else if open {
            col.spawn(Node {
                margin: UiRect::top(Val::Px(space::SM)),
                ..default()
            })
            .with_children(|b| {
                spawn_button(
                    b,
                    &format!("Travel to {} (Enter)", def.name),
                    ButtonVariant::Primary,
                    Act::Travel,
                );
            });
        } else {
            col.spawn(theme::text(
                format!("Opens at {}.", requirements_text(game, r)),
                font::BODY,
                palette::INK,
            ));
        }
        col.spawn(theme::text(
            "You can also walk: every path ends at a gate to the next region.",
            font::CAPTION,
            palette::INK_SOFT,
        ));
    });
}
