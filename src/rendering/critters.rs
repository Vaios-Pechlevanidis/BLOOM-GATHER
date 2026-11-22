//! Butterflies and bees drifting from flower to flower.
//!
//! Pure ambience: they never interact with gameplay. They visit flowers that
//! are still blooming (so picked patches gradually go quiet), stay within
//! the fence and above the ground, and freeze while the game is paused.

use bevy::pbr::NotShadowCaster;
use bevy::prelude::*;
use bloom_core::rng::SplitMix64;

use super::meshes::{lin, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::{RegionScene, Session};

pub struct CrittersPlugin;

impl Plugin for CrittersPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                spawn_critters.run_if(resource_exists_and_changed::<RegionScene>),
                (fly, flap),
            )
                .chain(),
        );
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Butterfly,
    Bee,
}

#[derive(Component)]
struct Critter {
    kind: Kind,
    target: Vec3,
    velocity: Vec3,
    retarget_in: f32,
    phase: f32,
    rng: SplitMix64,
}

#[derive(Component)]
struct Wing {
    side: f32,
    kind: Kind,
    phase: f32,
}

const BUTTERFLIES: usize = 9;
const BEES: usize = 7;

fn spawn_critters(
    mut commands: Commands,
    scene: Res<RegionScene>,
    content: Res<crate::game::Content>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let field = &scene.field;
    // Butterflies and bees sleep at night (fireflies come out instead).
    if field.is_empty() || content.regions[scene.region].scenery.atmosphere.night {
        return;
    }
    let mut rng = SplitMix64::new(0xB077_E2F1 ^ scene.region as u64);
    let painted = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.8,
        double_sided: true,
        cull_mode: None,
        ..default()
    });

    // Butterfly: slim dark body, two rounded wings (children, so they flap).
    let body_col = lin(Color::srgb_u8(70, 58, 50));
    let mut body = MeshBuilder::default();
    body.add(
        &Mesh::from(Capsule3d::new(0.012, 0.05)),
        Transform::from_rotation(Quat::from_rotation_x(std::f32::consts::FRAC_PI_2)),
        move |_| body_col,
    );
    let butterfly_body = meshes.add(body.build());
    let wing_colors = [
        (Color::srgb_u8(248, 236, 170), Color::srgb_u8(230, 186, 96)),
        (Color::srgb_u8(250, 248, 240), Color::srgb_u8(214, 210, 200)),
        (Color::srgb_u8(176, 196, 238), Color::srgb_u8(120, 140, 200)),
        (Color::srgb_u8(244, 196, 206), Color::srgb_u8(210, 140, 160)),
    ];
    let wing_meshes: Vec<Handle<Mesh>> = wing_colors
        .iter()
        .map(|(inner, edge)| {
            let (inner, edge) = (lin(*inner), lin(*edge));
            let mut w = MeshBuilder::default();
            let disc = Sphere::new(1.0).mesh().ico(1).expect("valid icosphere");
            // Fore and hind wing, extending along +X from the body.
            for (c, s) in [
                (Vec3::new(0.05, 0.0, 0.02), Vec3::new(0.05, 0.003, 0.04)),
                (Vec3::new(0.04, 0.0, -0.03), Vec3::new(0.035, 0.003, 0.03)),
            ] {
                w.add(
                    &disc,
                    Transform::from_translation(c).with_scale(s),
                    move |p| mix(inner, edge, p.x * 0.5 + 0.5),
                );
            }
            meshes.add(w.build())
        })
        .collect();

    // Bee: fuzzy striped body and two small pale wings.
    let yellow = lin(Color::srgb_u8(232, 186, 70));
    let brown = lin(Color::srgb_u8(70, 52, 36));
    let mut bee = MeshBuilder::default();
    bee.add(
        &Sphere::new(1.0).mesh().ico(2).expect("valid icosphere"),
        Transform::from_scale(Vec3::new(0.016, 0.015, 0.026)),
        move |p| {
            if (p.z * 3.5).rem_euclid(1.0) < 0.45 {
                brown
            } else {
                yellow
            }
        },
    );
    let bee_body = meshes.add(bee.build());
    let pale = lin(Color::srgb_u8(236, 240, 244));
    let mut bw = MeshBuilder::default();
    bw.add(
        &Sphere::new(1.0).mesh().ico(1).expect("valid icosphere"),
        Transform::from_xyz(0.018, 0.0, 0.0).with_scale(Vec3::new(0.02, 0.002, 0.011)),
        move |_| pale,
    );
    let bee_wing = meshes.add(bw.build());

    for i in 0..(BUTTERFLIES + BEES) {
        let kind = if i < BUTTERFLIES {
            Kind::Butterfly
        } else {
            Kind::Bee
        };
        let flower = rng.below(field.len() as u32);
        let (bloom, _) = field.pick_sphere(flower);
        let start = bloom
            + Vec3::new(
                rng.range_f32(-2.0, 2.0),
                rng.range_f32(0.4, 1.0),
                rng.range_f32(-2.0, 2.0),
            );
        // Shown a little larger than life, like the flowers.
        let scale = if kind == Kind::Butterfly { 2.4 } else { 2.0 };
        let phase = rng.range_f32(0.0, 10.0);
        let (body, wing) = match kind {
            Kind::Butterfly => (
                butterfly_body.clone(),
                wing_meshes[i % wing_meshes.len()].clone(),
            ),
            Kind::Bee => (bee_body.clone(), bee_wing.clone()),
        };
        commands
            .spawn((
                SceneEntity,
                Critter {
                    kind,
                    target: start,
                    velocity: Vec3::ZERO,
                    retarget_in: 0.0,
                    phase,
                    rng: SplitMix64::new(rng.next_u64()),
                },
                Mesh3d(body),
                MeshMaterial3d(painted.clone()),
                Transform::from_translation(start).with_scale(Vec3::splat(scale)),
                NotShadowCaster,
            ))
            .with_children(|c| {
                for side in [1.0f32, -1.0] {
                    let lift = if kind == Kind::Bee {
                        Vec3::Y * 0.012
                    } else {
                        Vec3::ZERO
                    };
                    c.spawn((
                        Wing { side, kind, phase },
                        Mesh3d(wing.clone()),
                        MeshMaterial3d(painted.clone()),
                        Transform::from_translation(lift).with_scale(Vec3::new(side, 1.0, 1.0)),
                        NotShadowCaster,
                    ));
                }
            });
    }
}

fn fly(
    time: Res<Time>,
    scene: Option<Res<RegionScene>>,
    session: Option<Res<Session>>,
    mut critters: Query<(&mut Critter, &mut Transform)>,
) {
    let (Some(scene), Some(session)) = (scene, session) else {
        return;
    };
    let dt = time.delta_secs();
    if dt <= 0.0 {
        return;
    }
    let t = time.elapsed_secs();
    let field = &scene.field;
    let layout = &scene.layout;
    for (mut c, mut transform) in &mut critters {
        let pos = transform.translation;
        c.retarget_in -= dt;
        if c.retarget_in <= 0.0 || pos.distance(c.target) < 0.25 {
            let (reach, hover, linger) = match c.kind {
                Kind::Butterfly => (9.0, c.rng.range_f32(0.25, 0.9), c.rng.range_f32(2.5, 5.0)),
                Kind::Bee => (5.0, 0.05, c.rng.range_f32(1.5, 3.0)),
            };
            // Visit a nearby flower that is still blooming.
            let here = Vec2::new(pos.x, pos.z);
            let mut next = None;
            for _ in 0..12 {
                let i = c.rng.below(field.len() as u32);
                if !session.is_harvested(scene.region, i)
                    && field.position(i).distance(here) < reach
                {
                    next = Some(i);
                    break;
                }
            }
            c.target = match next {
                Some(i) => field.pick_sphere(i).0 + Vec3::Y * hover,
                None => {
                    let wander = Vec2::new(c.rng.range_f32(-6.0, 6.0), c.rng.range_f32(-6.0, 6.0));
                    let p = (here + wander).clamp(layout.bounds_min, layout.bounds_max);
                    Vec3::new(p.x, layout.terrain.height_at(p) + 1.2, p.y)
                }
            };
            c.retarget_in = linger + pos.distance(c.target) * 0.6;
        }

        let (speed, turn): (f32, f32) = match c.kind {
            Kind::Butterfly => (1.3, 1.6),
            Kind::Bee => (2.2, 4.0),
        };
        let to = c.target - pos;
        let desired = if to.length() > 0.05 {
            to.normalize() * speed.min(to.length() * 2.0)
        } else {
            Vec3::ZERO
        };
        let blend = 1.0 - (-turn * dt).exp();
        c.velocity = c.velocity.lerp(desired, blend);
        // Butterflies bob and drift; bees jitter.
        let wobble = match c.kind {
            Kind::Butterfly => Vec3::new(
                (t * 1.7 + c.phase).sin() * 0.35,
                (t * 4.0 + c.phase).sin() * 0.6,
                (t * 1.3 + c.phase).cos() * 0.35,
            ),
            Kind::Bee => {
                Vec3::new(
                    (t * 9.0 + c.phase).sin(),
                    (t * 13.0 + c.phase).cos(),
                    (t * 11.0 + c.phase).sin(),
                ) * 0.25
            }
        };
        let mut p = pos + (c.velocity + wobble) * dt;
        let floor = layout.terrain.height_at(Vec2::new(p.x, p.z)) + 0.2;
        p.y = p.y.max(floor);
        let xz = Vec2::new(p.x, p.z).clamp(layout.bounds_min, layout.bounds_max);
        transform.translation = Vec3::new(xz.x, p.y, xz.y);
        let flat = Vec3::new(c.velocity.x, 0.0, c.velocity.z);
        if flat.length_squared() > 1e-4 {
            let goal = Transform::IDENTITY.looking_to(flat, Vec3::Y).rotation;
            transform.rotation = transform.rotation.slerp(goal, blend);
        }
    }
}

fn flap(time: Res<Time>, mut wings: Query<(&Wing, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (wing, mut transform) in &mut wings {
        let angle = match wing.kind {
            // Slow, wide strokes with a pause at the top.
            Kind::Butterfly => ((t * 11.0 + wing.phase).sin() * 0.5 + 0.5).powf(0.7) * 1.4 - 0.2,
            // A fast shimmer.
            Kind::Bee => (t * 70.0 + wing.phase).sin() * 0.5,
        };
        // Mirrored wing (scale.x = -1): the same rotation folds both upwards.
        transform.rotation = Quat::from_rotation_z(angle * wing.side);
    }
}
