//! What makes each region its own place: the gates between regions, water
//! and bridges, mushrooms, softly glowing ferns and fungi, shafts of light
//! through the forest canopy, drifting blossom petals and fireflies.
//!
//! Gates, water and bridges come from `bloom_core::world::layout` (they
//! shape where you can walk); the rest is decoration placed here,
//! deterministically, clear of the path, water and every flower. None of it
//! can be picked or counted.
//!
//! Temporary procedural art (see TODO.md), in the style of the trees.

use std::f32::consts::{FRAC_PI_2, PI, TAU};

use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;
use bloom_core::bitset::BitSet;
use bloom_core::data::{EntranceKind, RegionDef};
use bloom_core::rng::SplitMix64;
use bloom_core::world::features::GATE_HALF_WIDTH;
use bloom_core::world::{Bridge, Gate, GateSide, RegionLayout, Water, WaterKind};

use super::color;
use super::landmarks::{cuboid, wood};
use super::meshes::{hash3, lin, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::{Content, RegionScene, Session};
use crate::player::Player;

pub struct SceneryPlugin;

impl Plugin for SceneryPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_scenery.run_if(resource_exists_and_changed::<RegionScene>),
                refresh_gates,
                (fall_petals, drift_floaters, wander_fireflies, pulse_shafts),
            )
                .chain(),
        );
    }
}

/// A gate model; rebuilt when the region behind it opens.
#[derive(Component)]
struct GateModel {
    side: GateSide,
    open: bool,
}

#[derive(Component)]
struct Petal {
    velocity: Vec3,
    spin: Vec3,
    phase: f32,
}

/// Something floating along a stream (a petal) or bobbing on a pond.
#[derive(Component)]
struct Floater {
    /// Distance travelled along the stream centre line.
    along: f32,
    offset: f32,
    speed: f32,
}

#[derive(Component)]
struct Firefly {
    home: Vec3,
    phase: f32,
    rate: f32,
}

#[derive(Component)]
struct Shaft {
    phase: f32,
}

/// Shared look for vertex-coloured props.
fn painted(materials: &mut Assets<StandardMaterial>) -> Handle<StandardMaterial> {
    materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.88,
        reflectance: 0.15,
        ..default()
    })
}

#[allow(clippy::too_many_arguments)]
fn spawn_scenery(
    mut commands: Commands,
    scene: Res<RegionScene>,
    content: Res<Content>,
    session: Res<Session>,
    player: Option<Res<Player>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let started = std::time::Instant::now();
    let region = &content.regions[scene.region];
    let layout = &scene.layout;
    let features = &region.scenery.features;
    let night = region.scenery.atmosphere.night;
    let paint = painted(&mut materials);
    let mut rng = SplitMix64::new(region.seed ^ 0x5CE_4E21);

    // --- Gates ---------------------------------------------------------------
    for side in [GateSide::North, GateSide::South] {
        if let Some(to) = session.gate_destination(scene.region, side) {
            let open = session.is_region_unlocked(to);
            spawn_gate(
                &mut commands,
                &content,
                scene.region,
                to,
                layout.gate(side),
                open,
                &mut meshes,
                &mut materials,
                paint.clone(),
            );
        }
    }

    // --- Water and bridges --------------------------------------------------
    for w in &layout.water {
        let (tint, alpha) = if night {
            (LinearRgba::rgb(0.05, 0.08, 0.18), 0.86)
        } else {
            (LinearRgba::rgb(0.2, 0.42, 0.5), 0.72)
        };
        let water = materials.add(StandardMaterial {
            base_color: Color::LinearRgba(tint.with_alpha(alpha)),
            perceptual_roughness: 0.06,
            reflectance: 0.55,
            alpha_mode: AlphaMode::Blend,
            ..default()
        });
        commands.spawn((
            SceneEntity,
            Mesh3d(meshes.add(water_surface(w))),
            MeshMaterial3d(water),
            NotShadowCaster,
        ));
        spawn_floaters(
            &mut commands,
            w,
            region,
            &mut rng,
            &mut meshes,
            &mut materials,
        );
    }
    if !layout.bridges.is_empty() {
        let mut b = MeshBuilder::default();
        for bridge in &layout.bridges {
            bridge_mesh(&mut b, bridge);
        }
        commands.spawn((
            SceneEntity,
            Mesh3d(meshes.add(b.build())),
            MeshMaterial3d(paint.clone()),
        ));
    }

    // --- Mushrooms and glowing plants --------------------------------------
    let none = BitSet::new(scene.field.len() as u32);
    let free = |p: Vec2, clear: f32| {
        layout.in_bounds(p, 0.5)
            && layout.distance_to_path(p) > layout.path_half_width + 0.5
            && !layout.in_water(p, 0.3)
            && !layout.near_landmark(p, 0.4)
            && layout
                .trees
                .iter()
                .all(|t| t.position.distance(p) > t.trunk_radius + 0.25)
            && layout
                .rocks
                .iter()
                .all(|r| r.position.distance(p) > r.radius + 0.2)
            && scene.field.query_radius(p, clear, &none).is_empty()
    };
    // Mostly at the feet of trees, the rest scattered.
    let spot = |rng: &mut SplitMix64, clear: f32| -> Option<Vec2> {
        for _ in 0..40 {
            let p = if !layout.trees.is_empty() && rng.next_f32() < 0.7 {
                let t = &layout.trees[rng.below(layout.trees.len() as u32) as usize];
                let a = rng.range_f32(0.0, TAU);
                t.position + Vec2::new(a.cos(), a.sin()) * rng.range_f32(0.7, 3.2)
            } else {
                Vec2::new(
                    rng.range_f32(layout.bounds_min.x, layout.bounds_max.x),
                    rng.range_f32(layout.bounds_min.y, layout.bounds_max.y),
                )
            };
            if free(p, clear) {
                return Some(p);
            }
        }
        None
    };
    if features.mushrooms > 0 {
        let mut b = MeshBuilder::default();
        for _ in 0..features.mushrooms {
            if let Some(p) = spot(&mut rng, 0.7) {
                mushroom_cluster(&mut b, &mut rng, p, layout);
            }
        }
        commands.spawn((
            SceneEntity,
            Mesh3d(meshes.add(b.build())),
            MeshMaterial3d(paint.clone()),
        ));
    }
    if features.glow_plants > 0 {
        let glow = if night {
            LinearRgba::rgb(0.35, 0.6, 1.6)
        } else {
            LinearRgba::rgb(0.45, 1.3, 0.85)
        };
        let material = materials.add(StandardMaterial {
            base_color: Color::WHITE,
            emissive: glow * 0.9,
            perceptual_roughness: 0.7,
            ..default()
        });
        let mut b = MeshBuilder::default();
        for _ in 0..features.glow_plants {
            if let Some(p) = spot(&mut rng, 0.8) {
                glow_plant(&mut b, &mut rng, p, layout);
            }
        }
        commands.spawn((
            SceneEntity,
            Mesh3d(meshes.add(b.build())),
            MeshMaterial3d(material),
            NotShadowCaster,
        ));
    }

    // --- Light shafts ---------------------------------------------------------
    if features.light_shafts > 0 {
        let material = materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.93, 0.7, 0.14),
            unlit: true,
            alpha_mode: AlphaMode::Add,
            cull_mode: None,
            double_sided: true,
            ..default()
        });
        let air = region.scenery.atmosphere;
        let tilt = (90.0 - air.sun_elevation_deg).to_radians();
        let toward_sun = Vec2::new(-30.0, 28.0).normalize();
        // Lean the shaft so its top points at the sun.
        let rot = Quat::from_axis_angle(
            Vec3::new(toward_sun.y, 0.0, -toward_sun.x).normalize(),
            -tilt,
        );
        let shaft = meshes.add(Cylinder::new(1.0, 1.0).mesh().resolution(14).without_caps());
        for _ in 0..features.light_shafts {
            let Some(p) = (0..60).find_map(|_| {
                let q = Vec2::new(
                    rng.range_f32(layout.bounds_min.x + 6.0, layout.bounds_max.x - 6.0),
                    rng.range_f32(layout.bounds_min.y + 6.0, layout.bounds_max.y - 6.0),
                );
                layout
                    .trees
                    .iter()
                    .all(|t| t.position.distance(q) > 3.0)
                    .then_some(q)
            }) else {
                continue;
            };
            let height = rng.range_f32(12.0, 18.0);
            let ground = layout.terrain.height_at(p);
            let center = Vec3::new(p.x, ground, p.y) + rot * Vec3::Y * (height / 2.0);
            commands.spawn((
                SceneEntity,
                Shaft {
                    phase: rng.range_f32(0.0, TAU),
                },
                Mesh3d(shaft.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(center)
                    .with_rotation(rot)
                    .with_scale(Vec3::new(
                        rng.range_f32(1.0, 2.0),
                        height,
                        rng.range_f32(1.0, 2.0),
                    )),
                NotShadowCaster,
                NotShadowReceiver,
            ));
        }
    }

    // --- Falling petals ------------------------------------------------------
    if features.petals {
        let tint = lin(color(region.palette.accent));
        let material = materials.add(StandardMaterial {
            base_color: Color::LinearRgba(mix(tint, LinearRgba::WHITE, 0.25)),
            perceptual_roughness: 0.8,
            cull_mode: None,
            double_sided: true,
            ..default()
        });
        let petal = meshes.add(Rectangle::new(0.09, 0.06));
        let around = player
            .as_ref()
            .map_or(layout.spawn, |p| p.body.ground_position());
        for _ in 0..160 {
            let p = around + Vec2::new(rng.range_f32(-22.0, 22.0), rng.range_f32(-22.0, 22.0));
            let y = layout.terrain.height_at(p) + rng.range_f32(0.5, 9.0);
            commands.spawn((
                SceneEntity,
                Petal {
                    velocity: Vec3::new(0.25, -rng.range_f32(0.35, 0.7), 0.12),
                    spin: Vec3::new(
                        rng.range_f32(-2.0, 2.0),
                        rng.range_f32(-2.0, 2.0),
                        rng.range_f32(-2.0, 2.0),
                    ),
                    phase: rng.range_f32(0.0, TAU),
                },
                Mesh3d(petal.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_xyz(p.x, y, p.y),
                NotShadowCaster,
            ));
        }
    }

    // --- Fireflies ------------------------------------------------------------
    if features.fireflies > 0 {
        let material = materials.add(StandardMaterial {
            base_color: Color::LinearRgba(LinearRgba::rgb(3.0, 2.6, 0.9)),
            unlit: true,
            fog_enabled: false,
            ..default()
        });
        let dot = meshes.add(Sphere::new(0.035).mesh().ico(1).expect("valid icosphere"));
        for _ in 0..features.fireflies {
            let p = Vec2::new(
                rng.range_f32(layout.bounds_min.x, layout.bounds_max.x),
                rng.range_f32(layout.bounds_min.y, layout.bounds_max.y),
            );
            let home = Vec3::new(
                p.x,
                layout.terrain.height_at(p) + rng.range_f32(0.4, 1.8),
                p.y,
            );
            commands.spawn((
                SceneEntity,
                Firefly {
                    home,
                    phase: rng.range_f32(0.0, TAU),
                    rate: rng.range_f32(0.5, 1.1),
                },
                Mesh3d(dot.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(home),
                NotShadowCaster,
            ));
        }
    }

    info!(
        "Built scenery for '{}' in {:.1} ms ({} water, {} bridges)",
        region.id,
        started.elapsed().as_secs_f64() * 1000.0,
        layout.water.len(),
        layout.bridges.len()
    );
}

/// Rebuild a gate when the region behind it opens (or a new game resets).
#[allow(clippy::too_many_arguments)]
fn refresh_gates(
    mut commands: Commands,
    scene: Option<Res<RegionScene>>,
    content: Res<Content>,
    session: Option<Res<Session>>,
    gates: Query<(Entity, &GateModel)>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (Some(scene), Some(session)) = (scene, session) else {
        return;
    };
    for (entity, gate) in &gates {
        let Some(to) = session.gate_destination(scene.region, gate.side) else {
            continue;
        };
        let open = session.is_region_unlocked(to);
        if open != gate.open {
            commands.entity(entity).despawn();
            let paint = painted(&mut materials);
            spawn_gate(
                &mut commands,
                &content,
                scene.region,
                to,
                scene.layout.gate(gate.side),
                open,
                &mut meshes,
                &mut materials,
                paint,
            );
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_gate(
    commands: &mut Commands,
    content: &Content,
    here: usize,
    to: usize,
    gate: &Gate,
    open: bool,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
    paint: Handle<StandardMaterial>,
) {
    // The gate between two regions belongs to the later one.
    let kind = content.regions[here.max(to)].scenery.entrance;
    let mut b = MeshBuilder::default();
    let mut glass = MeshBuilder::default();
    gate_mesh(&mut b, &mut glass, kind, open);
    let mut e = commands.spawn((
        SceneEntity,
        GateModel {
            side: gate.side,
            open,
        },
        Mesh3d(meshes.add(b.build())),
        MeshMaterial3d(paint),
        Transform::from_xyz(gate.position.x, gate.base, gate.position.y)
            .with_rotation(Quat::from_rotation_y(gate.yaw)),
    ));
    if kind == EntranceKind::LanternArch {
        let lamp = materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.9, 0.7),
            emissive: LinearRgba::rgb(4.0, 2.6, 1.1),
            ..default()
        });
        let glass_mesh = meshes.add(glass.build());
        e.with_children(|g| {
            g.spawn((Mesh3d(glass_mesh), MeshMaterial3d(lamp), NotShadowCaster));
            g.spawn((
                PointLight {
                    color: Color::srgb(1.0, 0.82, 0.55),
                    intensity: 60_000.0,
                    range: 10.0,
                    shadows_enabled: false,
                    ..default()
                },
                Transform::from_xyz(0.0, 2.6, 0.4),
            ));
        });
    }
}

/// Gate models in local space: the opening spans X in ±`GATE_HALF_WIDTH`,
/// the front (+Z) faces into the region. A closed gate is barred.
fn gate_mesh(b: &mut MeshBuilder, glass: &mut MeshBuilder, kind: EntranceKind, open: bool) {
    let none = Quat::IDENTITY;
    let post_x = GATE_HALF_WIDTH + 0.12;
    let stone = |p: Vec3| {
        mix(
            lin(Color::srgb_u8(150, 144, 134)),
            lin(Color::srgb_u8(196, 190, 176)),
            0.5 + hash3(p * 5.0) * 0.35,
        )
    };
    let iron = |p: Vec3| {
        mix(
            lin(Color::srgb_u8(34, 36, 44)),
            lin(Color::srgb_u8(70, 74, 88)),
            0.5 + hash3(p * 6.0) * 0.3,
        )
    };
    let bark = |p: Vec3| {
        mix(
            lin(Color::srgb_u8(84, 64, 50)),
            lin(Color::srgb_u8(128, 98, 74)),
            0.5 + hash3(p * 4.0) * 0.4,
        )
    };
    match kind {
        EntranceKind::FarmGate => {
            for side in [-1.0, 1.0] {
                cuboid(
                    b,
                    Vec3::new(0.2, 2.4, 0.2),
                    Vec3::new(side * post_x, 0.2, 0.0),
                    none,
                    wood(false),
                );
                cuboid(
                    b,
                    Vec3::new(0.26, 0.1, 0.26),
                    Vec3::new(side * post_x, 1.42, 0.0),
                    none,
                    wood(true),
                );
            }
            // Two five-bar leaves, hinged on the posts.
            let leaf = GATE_HALF_WIDTH - 0.05;
            for side in [-1.0f32, 1.0] {
                let swing = if open { side * -1.75 } else { 0.0 };
                let hinge = Vec3::new(side * (GATE_HALF_WIDTH - 0.02), 0.0, 0.0);
                let r = Quat::from_rotation_y(swing);
                let at = |local: Vec3| hinge + r * local;
                for k in 0..5 {
                    let y = 0.3 + k as f32 * 0.21;
                    cuboid(
                        b,
                        Vec3::new(leaf, 0.07, 0.05),
                        at(Vec3::new(-side * leaf / 2.0, y, 0.0)),
                        r,
                        wood(true),
                    );
                }
                for x in [0.06, leaf - 0.06] {
                    cuboid(
                        b,
                        Vec3::new(0.08, 0.95, 0.06),
                        at(Vec3::new(-side * x, 0.72, 0.0)),
                        r,
                        wood(false),
                    );
                }
                // The diagonal brace.
                let d = Vec3::new(-side * leaf, 0.84, 0.0);
                let brace = Quat::from_rotation_z(-side * (0.84f32).atan2(leaf));
                cuboid(
                    b,
                    Vec3::new((leaf * leaf + 0.7).sqrt(), 0.07, 0.05),
                    at(d / 2.0 + Vec3::Y * 0.3),
                    r * brace,
                    wood(false),
                );
            }
        }
        EntranceKind::StoneArch => {
            for side in [-1.0, 1.0] {
                cuboid(
                    b,
                    Vec3::new(0.55, 3.0, 0.55),
                    Vec3::new(side * (post_x + 0.15), 0.5, 0.0),
                    none,
                    stone,
                );
            }
            // Arch of blocks over the opening.
            let r = post_x + 0.15;
            for k in 0..11 {
                let a = PI * (k as f32 + 0.5) / 11.0;
                let at = Vec3::new(a.cos() * r, 2.0 + a.sin() * r * 0.75, 0.0);
                cuboid(
                    b,
                    Vec3::new(0.5, 0.42, 0.55),
                    at,
                    Quat::from_rotation_z(a - FRAC_PI_2),
                    stone,
                );
            }
            // Blossom tumbling over the top.
            let pink = lin(Color::srgb_u8(240, 190, 204));
            let pale = lin(Color::srgb_u8(252, 228, 234));
            let puff = Sphere::new(1.0).mesh().ico(1).expect("valid icosphere");
            for k in 0..9 {
                let a = PI * (k as f32 + 0.5) / 9.0;
                let at = Vec3::new(
                    a.cos() * r * 1.05,
                    2.25 + a.sin() * r * 0.8,
                    hash3(Vec3::splat(k as f32)) * 0.2,
                );
                let t = Transform::from_translation(at)
                    .with_scale(Vec3::splat(0.32 + hash3(at) * 0.08));
                b.add(&puff, t, move |p| mix(pink, pale, p.y * 0.5 + 0.5));
            }
            if !open {
                barrier(b);
            }
        }
        EntranceKind::ForestArch => {
            // Two trunks leaning in until their crowns meet.
            let trunk = Mesh::from(Cylinder::new(0.22, 1.0).mesh().resolution(8));
            for side in [-1.0f32, 1.0] {
                for (k, (h, lean)) in [(0.0f32, 0.0f32), (1.4, 0.12), (2.6, 0.38)]
                    .iter()
                    .enumerate()
                {
                    let at = Vec3::new(side * (post_x + 0.3 - lean * 2.0), h + 0.7, 0.0);
                    let t = Transform::from_translation(at)
                        .with_rotation(Quat::from_rotation_z(side * lean * 1.4))
                        .with_scale(Vec3::new(1.0 - k as f32 * 0.15, 1.6, 1.0 - k as f32 * 0.15));
                    b.add(&trunk, t, bark);
                }
            }
            let leaf_dark = lin(Color::srgb_u8(70, 96, 70));
            let leaf_light = lin(Color::srgb_u8(126, 156, 110));
            let blob = Sphere::new(1.0).mesh().ico(2).expect("valid icosphere");
            for (x, y, r) in [
                (0.0, 4.1, 0.9),
                (-1.0, 3.8, 0.75),
                (1.0, 3.85, 0.75),
                (0.4, 4.6, 0.6),
                (-0.5, 4.5, 0.6),
            ] {
                let t =
                    Transform::from_translation(Vec3::new(x, y, 0.0)).with_scale(Vec3::splat(r));
                b.add(&blob, t, move |p| {
                    mix(
                        leaf_dark,
                        leaf_light,
                        p.y * 0.5 + 0.5 + hash3(p * 3.0) * 0.1,
                    )
                });
            }
            // Hanging vines.
            let vine = lin(Color::srgb_u8(96, 128, 84));
            for k in 0..7 {
                let x = -1.2 + k as f32 * 0.4;
                let len = 0.6 + hash3(Vec3::new(x, 1.0, 2.0)).abs() * 0.9;
                cuboid(
                    b,
                    Vec3::new(0.04, len, 0.04),
                    Vec3::new(x, 3.55 - len / 2.0, 0.15),
                    none,
                    move |_| vine,
                );
            }
            if !open {
                // A tangle of fallen branches across the way.
                for (y, a) in [(0.5f32, 0.25f32), (0.9, -0.3), (1.25, 0.12)] {
                    cuboid(
                        b,
                        Vec3::new(GATE_HALF_WIDTH * 2.3, 0.14, 0.14),
                        Vec3::new(0.0, y, 0.1),
                        Quat::from_rotation_z(a),
                        bark,
                    );
                }
            }
        }
        EntranceKind::LanternArch => {
            for side in [-1.0, 1.0] {
                cuboid(
                    b,
                    Vec3::new(0.12, 3.0, 0.12),
                    Vec3::new(side * post_x, 0.5, 0.0),
                    none,
                    iron,
                );
                cuboid(
                    b,
                    Vec3::new(0.24, 0.14, 0.24),
                    Vec3::new(side * post_x, 0.02, 0.0),
                    none,
                    iron,
                );
            }
            // A curved iron arch with scrolls.
            let r = post_x;
            for k in 0..16 {
                let a = PI * (k as f32 + 0.5) / 16.0;
                let at = Vec3::new(a.cos() * r, 2.0 + a.sin() * r * 0.6, 0.0);
                cuboid(
                    b,
                    Vec3::new(0.36, 0.07, 0.07),
                    at,
                    Quat::from_rotation_z(a - FRAC_PI_2 + FRAC_PI_2),
                    iron,
                );
            }
            // Two lanterns hanging from the arch.
            for side in [-1.0, 1.0] {
                let x = side * 0.75;
                cuboid(
                    b,
                    Vec3::new(0.02, 0.35, 0.02),
                    Vec3::new(x, 2.55, 0.0),
                    none,
                    iron,
                );
                cuboid(
                    b,
                    Vec3::new(0.26, 0.05, 0.26),
                    Vec3::new(x, 2.37, 0.0),
                    none,
                    iron,
                );
                cuboid(
                    b,
                    Vec3::new(0.22, 0.04, 0.22),
                    Vec3::new(x, 2.03, 0.0),
                    none,
                    iron,
                );
                glass.add(
                    &Mesh::from(Cuboid::new(0.18, 0.3, 0.18)),
                    Transform::from_xyz(x, 2.2, 0.0),
                    |_| LinearRgba::WHITE,
                );
            }
            if !open {
                // Closed iron gates.
                for k in 0..11 {
                    let x =
                        -GATE_HALF_WIDTH + 0.15 + k as f32 * (GATE_HALF_WIDTH * 2.0 - 0.3) / 10.0;
                    cuboid(
                        b,
                        Vec3::new(0.05, 1.6, 0.05),
                        Vec3::new(x, 0.8, 0.0),
                        none,
                        iron,
                    );
                }
                for y in [0.25, 1.4] {
                    cuboid(
                        b,
                        Vec3::new(GATE_HALF_WIDTH * 2.0, 0.06, 0.06),
                        Vec3::new(0.0, y, 0.0),
                        none,
                        iron,
                    );
                }
            }
        }
    }
}

/// A plank barrier with a little hanging sign: "not yet".
fn barrier(b: &mut MeshBuilder) {
    for (y, a) in [(0.55f32, 0.06f32), (1.0, -0.05)] {
        cuboid(
            b,
            Vec3::new(GATE_HALF_WIDTH * 2.2, 0.16, 0.06),
            Vec3::new(0.0, y, 0.12),
            Quat::from_rotation_z(a),
            wood(true),
        );
    }
    let cream = lin(Color::srgb_u8(238, 228, 204));
    cuboid(
        b,
        Vec3::new(0.6, 0.34, 0.03),
        Vec3::new(0.0, 0.78, 0.17),
        Quat::IDENTITY,
        move |_| cream,
    );
}

/// A flat water surface covering the carved bed (banks hide its edges).
fn water_surface(w: &Water) -> Mesh {
    let mut positions = Vec::new();
    let mut indices: Vec<u32> = Vec::new();
    let half = w.outer_radius() + 0.3;
    match w.kind {
        WaterKind::Stream => {
            for (i, &p) in w.points.iter().enumerate() {
                let prev = w.points[i.saturating_sub(1)];
                let next = w.points[(i + 1).min(w.points.len() - 1)];
                let t = (next - prev).normalize_or(Vec2::X);
                let n = Vec2::new(-t.y, t.x);
                for side in [-1.0, 1.0] {
                    let q = p + n * side * half;
                    positions.push([q.x, w.level, q.y]);
                }
                if i > 0 {
                    let a = (i as u32 - 1) * 2;
                    indices.extend([a, a + 1, a + 2, a + 1, a + 3, a + 2]);
                }
            }
        }
        WaterKind::Pond => {
            let c = w.points[0];
            positions.push([c.x, w.level, c.y]);
            let n = 48u32;
            for k in 0..n {
                let a = k as f32 / n as f32 * TAU;
                positions.push([c.x + a.cos() * half, w.level, c.y + a.sin() * half]);
            }
            for k in 0..n {
                indices.extend([0, 1 + (k + 1) % n, 1 + k]);
            }
        }
    }
    // Make every triangle face up whatever the winding came out as.
    for tri in indices.chunks_mut(3) {
        let p = |i: u32| Vec3::from(positions[i as usize]);
        let up = (p(tri[1]) - p(tri[0])).cross(p(tri[2]) - p(tri[0])).y;
        if up < 0.0 {
            tri.swap(1, 2);
        }
    }
    let normals = vec![[0.0, 1.0, 0.0]; positions.len()];
    Mesh::new(
        bevy::mesh::PrimitiveTopology::TriangleList,
        bevy::asset::RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(bevy::mesh::Indices::U32(indices))
}

/// Petals drifting down the stream; lily pads on a pond.
fn spawn_floaters(
    commands: &mut Commands,
    w: &Water,
    region: &RegionDef,
    rng: &mut SplitMix64,
    meshes: &mut Assets<Mesh>,
    materials: &mut Assets<StandardMaterial>,
) {
    match w.kind {
        WaterKind::Stream => {
            if !region.scenery.features.petals {
                return;
            }
            let tint = lin(color(region.palette.accent));
            let material = materials.add(StandardMaterial {
                base_color: Color::LinearRgba(mix(tint, LinearRgba::WHITE, 0.3)),
                perceptual_roughness: 0.8,
                cull_mode: None,
                ..default()
            });
            let petal = meshes.add(Circle::new(0.06).mesh().resolution(6));
            let length: f32 = w.points.windows(2).map(|p| p[0].distance(p[1])).sum();
            for _ in 0..90 {
                commands.spawn((
                    SceneEntity,
                    Floater {
                        along: rng.range_f32(0.0, length),
                        offset: rng.range_f32(-0.8, 0.8) * w.half_width,
                        speed: rng.range_f32(0.35, 0.6),
                    },
                    Mesh3d(petal.clone()),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(0.0, w.level + 0.01, 0.0)
                        .with_rotation(Quat::from_rotation_x(-FRAC_PI_2)),
                    NotShadowCaster,
                ));
            }
        }
        WaterKind::Pond => {
            let pad = lin(Color::srgb_u8(70, 104, 84));
            let pad_light = lin(Color::srgb_u8(104, 140, 108));
            let disc = Mesh::from(Cylinder::new(1.0, 0.02).mesh().resolution(14));
            let mut b = MeshBuilder::default();
            let c = w.points[0];
            for _ in 0..22 {
                let a = rng.range_f32(0.0, TAU);
                let r = w.half_width * rng.range_f32(0.2, 0.85);
                let p = c + Vec2::new(a.cos(), a.sin()) * r;
                let s = rng.range_f32(0.25, 0.5);
                let t = Transform::from_xyz(p.x, w.level + 0.015, p.y)
                    .with_rotation(Quat::from_rotation_y(rng.range_f32(0.0, TAU)))
                    .with_scale(Vec3::new(s, 1.0, s));
                b.add(&disc, t, move |q| {
                    mix(pad, pad_light, 0.5 + hash3(q * 9.0) * 0.4)
                });
            }
            let material = materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.5,
                ..default()
            });
            commands.spawn((
                SceneEntity,
                Mesh3d(meshes.add(b.build())),
                MeshMaterial3d(material),
            ));
        }
    }
}

/// Planks across an arched deck, beams underneath and railings.
fn bridge_mesh(b: &mut MeshBuilder, bridge: &Bridge) {
    let yaw = Quat::from_rotation_y(bridge.yaw());
    let length = bridge.half_length * 2.0;
    let planks = (length / 0.32).ceil() as usize;
    let at = |u: f32, side: f32, up: f32| {
        let p = bridge.point(u);
        let normal = Vec2::new(-bridge.dir.y, bridge.dir.x);
        let q = p + normal * side;
        Vec3::new(q.x, bridge.deck_height(u) + up, q.y)
    };
    let slope = |u: f32| {
        let du = 0.01;
        let dh = bridge.deck_height((u + du).min(1.0)) - bridge.deck_height((u - du).max(0.0));
        (dh / (2.0 * du * length)).atan()
    };
    for k in 0..planks {
        let u = (k as f32 + 0.5) / planks as f32;
        let r = yaw * Quat::from_rotation_z(slope(u));
        cuboid(
            b,
            Vec3::new(0.28, 0.07, bridge.half_width * 2.0),
            at(u, 0.0, -0.035),
            r,
            wood(k % 3 != 0),
        );
    }
    // Side beams follow the arch in short pieces.
    let pieces = 10;
    for side in [-1.0, 1.0] {
        for k in 0..pieces {
            let u = (k as f32 + 0.5) / pieces as f32;
            let r = yaw * Quat::from_rotation_z(slope(u));
            let seg = length / pieces as f32 + 0.04;
            cuboid(
                b,
                Vec3::new(seg, 0.18, 0.1),
                at(u, side * (bridge.half_width - 0.05), -0.15),
                r,
                wood(false),
            );
            // Railing.
            cuboid(
                b,
                Vec3::new(seg, 0.07, 0.07),
                at(u, side * (bridge.half_width + 0.05), 0.9),
                r,
                wood(true),
            );
        }
        for k in 0..=4 {
            let u = 0.06 + 0.88 * k as f32 / 4.0;
            cuboid(
                b,
                Vec3::new(0.1, 1.0, 0.1),
                at(u, side * (bridge.half_width + 0.05), 0.45),
                yaw,
                wood(false),
            );
        }
    }
}

/// One to four toadstools around `p`.
fn mushroom_cluster(b: &mut MeshBuilder, rng: &mut SplitMix64, p: Vec2, layout: &RegionLayout) {
    let stem_c = lin(Color::srgb_u8(236, 226, 206));
    let caps = [
        lin(Color::srgb_u8(196, 82, 70)),
        lin(Color::srgb_u8(170, 120, 82)),
        lin(Color::srgb_u8(214, 170, 110)),
    ];
    let spot = lin(Color::srgb_u8(248, 242, 228));
    let stem = Mesh::from(Cylinder::new(1.0, 1.0).mesh().resolution(8));
    let cap = Sphere::new(1.0).mesh().uv(10, 6);
    let n = 1 + rng.below(4);
    let cap_c = caps[rng.below(3) as usize];
    for _ in 0..n {
        let q = p + Vec2::new(rng.range_f32(-0.35, 0.35), rng.range_f32(-0.35, 0.35));
        let ground = layout.terrain.height_at(q);
        let h = rng.range_f32(0.08, 0.26);
        let r = h * rng.range_f32(0.55, 0.8);
        b.add(
            &stem,
            Transform::from_xyz(q.x, ground + h / 2.0, q.y).with_scale(Vec3::new(
                r * 0.28,
                h,
                r * 0.28,
            )),
            move |_| stem_c,
        );
        let spotted = cap_c.red > 0.5;
        b.add(
            &cap,
            Transform::from_xyz(q.x, ground + h, q.y).with_scale(Vec3::new(r, r * 0.55, r)),
            move |v| {
                if v.y < 0.05 {
                    stem_c
                } else if spotted && hash3(v * 4.0) > 0.55 {
                    spot
                } else {
                    cap_c
                }
            },
        );
    }
}

/// A glowing fern (arching fronds) or a cluster of glowing fungi. Vertex
/// colour scales the material's glow, so tips shine brightest.
fn glow_plant(b: &mut MeshBuilder, rng: &mut SplitMix64, p: Vec2, layout: &RegionLayout) {
    let ground = layout.terrain.height_at(p);
    let dim = LinearRgba::rgb(0.25, 0.32, 0.3);
    let bright = LinearRgba::rgb(0.9, 1.0, 0.95);
    if rng.next_f32() < 0.6 {
        let frond = Mesh::from(Cuboid::new(0.05, 0.012, 1.0));
        let fronds = 5 + rng.below(4);
        for k in 0..fronds {
            let a = k as f32 / fronds as f32 * TAU + rng.range_f32(-0.2, 0.2);
            let len = rng.range_f32(0.35, 0.6);
            let r = Quat::from_rotation_y(a) * Quat::from_rotation_x(-0.6);
            let t = Transform::from_translation(
                Vec3::new(p.x, ground + 0.02, p.y) + r * Vec3::new(0.0, 0.0, len / 2.0),
            )
            .with_rotation(r)
            .with_scale(Vec3::new(1.0, 1.0, len));
            b.add(&frond, t, move |v| {
                mix(dim, bright, (v.z + 0.5).clamp(0.0, 1.0))
            });
        }
    } else {
        let cap = Sphere::new(1.0).mesh().uv(8, 5);
        for _ in 0..3 + rng.below(4) {
            let q = p + Vec2::new(rng.range_f32(-0.25, 0.25), rng.range_f32(-0.25, 0.25));
            let h = rng.range_f32(0.04, 0.12);
            let r = rng.range_f32(0.03, 0.07);
            b.add(
                &cap,
                Transform::from_xyz(q.x, ground + h, q.y).with_scale(Vec3::new(r, r * 0.6, r)),
                move |v| mix(dim, bright, (v.y + 0.3).clamp(0.0, 1.0)),
            );
        }
    }
}

fn fall_petals(
    time: Res<Time>,
    scene: Option<Res<RegionScene>>,
    player: Option<Res<Player>>,
    mut petals: Query<(&Petal, &mut Transform)>,
) {
    let (Some(scene), Some(player)) = (scene, player) else {
        return;
    };
    let dt = time.delta_secs();
    let t = time.elapsed_secs();
    let around = player.body.ground_position();
    for (petal, mut tr) in &mut petals {
        let sway = Vec3::new(
            (t * 1.3 + petal.phase).sin(),
            0.0,
            (t * 0.9 + petal.phase).cos(),
        ) * 0.35;
        tr.translation += (petal.velocity + sway) * dt;
        tr.rotate(Quat::from_euler(
            EulerRot::XYZ,
            petal.spin.x * dt,
            petal.spin.y * dt,
            petal.spin.z * dt,
        ));
        let p = Vec2::new(tr.translation.x, tr.translation.z);
        let ground = scene.layout.terrain.height_at(p);
        // Landed or drifted away: start again high above, near the player.
        if tr.translation.y < ground || p.distance(around) > 26.0 {
            let k = hash3(tr.translation + Vec3::splat(t)) * 0.5 + 0.5;
            let a = (hash3(tr.translation * 1.7) * 0.5 + 0.5) * TAU;
            let q = around + Vec2::new(a.cos(), a.sin()) * (4.0 + k * 20.0);
            tr.translation = Vec3::new(q.x, scene.layout.terrain.height_at(q) + 6.0 + k * 4.0, q.y);
        }
    }
}

fn drift_floaters(
    time: Res<Time>,
    scene: Option<Res<RegionScene>>,
    mut floaters: Query<(&mut Floater, &mut Transform)>,
) {
    let Some(scene) = scene else { return };
    let Some(w) = scene
        .layout
        .water
        .iter()
        .find(|w| w.kind == WaterKind::Stream)
    else {
        return;
    };
    let lengths: Vec<f32> = w.points.windows(2).map(|p| p[0].distance(p[1])).collect();
    let total: f32 = lengths.iter().sum();
    for (mut f, mut tr) in &mut floaters {
        f.along = (f.along + f.speed * time.delta_secs()) % total;
        let mut s = f.along;
        let mut i = 0;
        while i + 1 < lengths.len() && s > lengths[i] {
            s -= lengths[i];
            i += 1;
        }
        let (a, b) = (w.points[i], w.points[i + 1]);
        let dir = (b - a).normalize_or(Vec2::X);
        let n = Vec2::new(-dir.y, dir.x);
        let p = a + dir * s + n * f.offset;
        tr.translation.x = p.x;
        tr.translation.z = p.y;
    }
}

fn wander_fireflies(
    time: Res<Time>,
    mut flies: Query<(&Firefly, &mut Transform, &mut Visibility)>,
) {
    let t = time.elapsed_secs();
    for (f, mut tr, mut vis) in &mut flies {
        let k = t * f.rate + f.phase;
        tr.translation = f.home
            + Vec3::new(
                (k * 0.7).sin() * 1.6,
                (k * 1.3).sin() * 0.35,
                (k * 0.5).cos() * 1.6,
            );
        // Blink: lit most of the time, dark for a moment now and then.
        let lit = (k * 1.7).sin() > -0.55;
        let want = if lit {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *vis != want {
            *vis = want;
        }
    }
}

fn pulse_shafts(
    time: Res<Time>,
    shafts: Query<(&Shaft, &MeshMaterial3d<StandardMaterial>)>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    // All shafts share one material: breathe it gently.
    let Some((shaft, handle)) = shafts.iter().next() else {
        return;
    };
    if let Some(m) = materials.get_mut(&handle.0) {
        let a = 0.14 + 0.04 * (time.elapsed_secs() * 0.4 + shaft.phase).sin();
        m.base_color = m.base_color.with_alpha(a);
    }
}
