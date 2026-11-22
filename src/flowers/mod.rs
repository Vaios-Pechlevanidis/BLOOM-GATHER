//! Flowers in the 3D world: one entity per unharvested flower, wind sway,
//! look-at highlight, and the pick animation with a burst of petals.
//!
//! Which flowers exist comes only from the session (`bloom_core`): this
//! module mirrors it. A harvested flower is despawned after its animation and
//! never spawned again — on reload, harvested indices are simply skipped.

pub mod models;
mod sparkles;

use std::collections::HashMap;

use bevy::prelude::*;
use bloom_core::game::GameEvent;

use crate::game::{Content, EventsForwarded, GameEventMsg, RegionScene, Session};
use crate::interaction::LookTarget;
use crate::rendering::{color, MainCamera};

pub struct FlowersPlugin;

impl Plugin for FlowersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnedFlowers>()
            .add_plugins(sparkles::SparklesPlugin)
            .add_systems(
                Update,
                (
                    (ensure_assets, sync_flowers).chain(),
                    sway,
                    highlight,
                    start_harvest_animation.after(EventsForwarded),
                    animate_harvest,
                    animate_particles,
                ),
            );
    }
}

#[derive(Resource)]
struct FlowerAssets {
    plants: Vec<Handle<Mesh>>,
    glows: Vec<Option<Handle<Mesh>>>,
    material: Handle<StandardMaterial>,
    highlight: Handle<StandardMaterial>,
    glow_materials: Vec<Option<Handle<StandardMaterial>>>,
    particle: Handle<Mesh>,
    particle_materials: Vec<Handle<StandardMaterial>>,
}

/// Root of one flower in the world.
#[derive(Component)]
pub struct FlowerEntity {
    yaw: f32,
    phase: f32,
}

/// The non-glowing mesh child (its material is swapped to highlight).
#[derive(Component)]
struct FlowerBody;

/// Which flowers are currently spawned, by index.
#[derive(Resource, Default)]
pub struct SpawnedFlowers {
    /// (session generation, region) the entities were built for.
    key: Option<(u64, usize)>,
    pub map: HashMap<u32, Entity>,
    highlighted: Vec<Entity>,
}

#[derive(Component)]
struct Harvesting {
    t: f32,
    from: Vec3,
    start_scale: f32,
}

#[derive(Component)]
struct Particle {
    velocity: Vec3,
    spin: Vec3,
    age: f32,
    life: f32,
    size: f32,
}

const HARVEST_SECS: f32 = 0.42;

fn ensure_assets(
    mut commands: Commands,
    assets: Option<Res<FlowerAssets>>,
    content: Option<Res<Content>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let (None, Some(content)) = (assets, content) else {
        return;
    };
    let started = std::time::Instant::now();
    let mut plants = Vec::new();
    let mut glows = Vec::new();
    let mut glow_materials = Vec::new();
    let mut particle_materials = Vec::new();
    for species in &content.species {
        let model = models::build(species);
        plants.push(meshes.add(model.plant));
        glows.push(model.glow.map(|m| meshes.add(m)));
        let petal = color(species.petal_color);
        glow_materials.push((species.glow > 0.0).then(|| {
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                emissive: petal.to_linear() * (species.glow * 4.0),
                perceptual_roughness: 0.6,
                ..default()
            })
        }));
        particle_materials.push(materials.add(StandardMaterial {
            base_color: petal,
            perceptual_roughness: 0.7,
            ..default()
        }));
    }
    let material = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.72,
        reflectance: 0.2,
        ..default()
    });
    // Subtle highlight: the same vertex colours, gently lit from within.
    let highlight = materials.add(StandardMaterial {
        base_color: Color::WHITE,
        emissive: LinearRgba::new(0.10, 0.09, 0.05, 1.0),
        perceptual_roughness: 0.6,
        ..default()
    });
    info!(
        "Built {} flower models in {:.1} ms",
        plants.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    commands.insert_resource(FlowerAssets {
        plants,
        glows,
        material,
        highlight,
        glow_materials,
        particle: meshes.add(models::petal_particle()),
        particle_materials,
    });
}

/// (Re)spawn flowers when the session or region changes.
fn sync_flowers(
    mut commands: Commands,
    assets: Option<Res<FlowerAssets>>,
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    mut spawned: ResMut<SpawnedFlowers>,
) {
    let (Some(assets), Some(session), Some(scene)) = (assets, session, scene) else {
        return;
    };
    let key = (session.generation, scene.region);
    if spawned.key == Some(key) {
        return;
    }
    for (_, entity) in spawned.map.drain() {
        commands.entity(entity).despawn();
    }
    spawned.highlighted.clear();
    let field = &scene.field;
    let mut count = 0;
    for i in 0..field.len() as u32 {
        if session.is_harvested(scene.region, i) {
            continue; // picked flowers never return
        }
        let species = field.species(i);
        let ground = field.ground_point(i);
        let root = commands
            .spawn((
                FlowerEntity {
                    yaw: field.yaw(i),
                    phase: ground.x * 0.31 + ground.z * 0.17,
                },
                Transform::from_translation(ground)
                    .with_rotation(Quat::from_rotation_y(field.yaw(i)))
                    .with_scale(Vec3::splat(field.scale(i))),
                Visibility::default(),
            ))
            .with_children(|flower| {
                flower.spawn((
                    FlowerBody,
                    Mesh3d(assets.plants[species].clone()),
                    MeshMaterial3d(assets.material.clone()),
                ));
                if let (Some(mesh), Some(mat)) =
                    (&assets.glows[species], &assets.glow_materials[species])
                {
                    flower.spawn((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone())));
                }
            })
            .id();
        spawned.map.insert(i, root);
        count += 1;
    }
    spawned.key = Some(key);
    info!(
        "Spawned {count} flowers ({} already picked)",
        field.len() - count
    );
}

/// Gentle wind: each flower leans and recovers, with gusts rolling across
/// the field (phase from position).
fn sway(time: Res<Time>, mut flowers: Query<(&FlowerEntity, &mut Transform), Without<Harvesting>>) {
    let t = time.elapsed_secs();
    for (flower, mut transform) in &mut flowers {
        let gust = (t * 0.45 + flower.phase * 0.25).sin() * 0.5 + 0.5;
        let lean_x = (t * 1.7 + flower.phase).sin() * (0.025 + 0.05 * gust);
        let lean_z = (t * 1.3 + flower.phase * 1.3).cos() * 0.03;
        transform.rotation = Quat::from_rotation_z(lean_z)
            * Quat::from_rotation_x(lean_x)
            * Quat::from_rotation_y(flower.yaw);
    }
}

fn highlight(
    target: Res<LookTarget>,
    assets: Option<Res<FlowerAssets>>,
    mut spawned: ResMut<SpawnedFlowers>,
    children: Query<&Children>,
    mut bodies: Query<&mut MeshMaterial3d<StandardMaterial>, With<FlowerBody>>,
) {
    let Some(assets) = assets else { return };
    let wanted: Vec<Entity> = target
        .flower
        .iter()
        .chain(&target.group)
        .filter_map(|i| spawned.map.get(i).copied())
        .collect();
    if wanted == spawned.highlighted {
        return;
    }
    let mut set = |root: Entity, material: &Handle<StandardMaterial>| {
        if let Ok(kids) = children.get(root) {
            for kid in kids.iter() {
                if let Ok(mut m) = bodies.get_mut(kid) {
                    m.0 = material.clone();
                }
            }
        }
    };
    for &old in spawned.highlighted.iter().filter(|e| !wanted.contains(e)) {
        set(old, &assets.material);
    }
    for &new in &wanted {
        set(new, &assets.highlight);
    }
    spawned.highlighted = wanted;
}

fn start_harvest_animation(
    mut commands: Commands,
    mut events: EventReader<GameEventMsg>,
    mut spawned: ResMut<SpawnedFlowers>,
    assets: Option<Res<FlowerAssets>>,
    scene: Option<Res<RegionScene>>,
    flowers: Query<&Transform, With<FlowerEntity>>,
) {
    let (Some(assets), Some(scene)) = (assets, scene) else {
        return;
    };
    for GameEventMsg(event) in events.read() {
        let GameEvent::FlowerHarvested {
            region,
            flower,
            species,
        } = *event
        else {
            continue;
        };
        if region != scene.region {
            continue;
        }
        let Some(entity) = spawned.map.remove(&flower) else {
            continue;
        };
        spawned.highlighted.retain(|e| *e != entity);
        let Ok(transform) = flowers.get(entity) else {
            continue;
        };
        commands.entity(entity).insert(Harvesting {
            t: 0.0,
            from: transform.translation,
            start_scale: transform.scale.x,
        });
        // A small burst of petals from the bloom.
        let (bloom, _) = scene.field.pick_sphere(flower);
        for k in 0..10 {
            let a = k as f32 * 2.399 + flower as f32;
            let out = Vec3::new(a.cos(), 0.0, a.sin());
            commands.spawn((
                Mesh3d(assets.particle.clone()),
                MeshMaterial3d(assets.particle_materials[species].clone()),
                Transform::from_translation(bloom).with_scale(Vec3::new(0.016, 0.004, 0.011)),
                Particle {
                    velocity: out * (0.5 + (k % 3) as f32 * 0.25)
                        + Vec3::Y * (1.1 + (k % 4) as f32 * 0.2),
                    spin: Vec3::new(4.0 + k as f32, 2.0, 3.0 - k as f32 * 0.4),
                    age: 0.0,
                    life: 0.8 + (k % 3) as f32 * 0.15,
                    size: 1.0,
                },
            ));
        }
    }
}

/// The picked flower lifts towards the player, spins and shrinks away.
fn animate_harvest(
    mut commands: Commands,
    time: Res<Time>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut flowers: Query<(Entity, &mut Harvesting, &mut Transform)>,
) {
    let eye = cameras
        .single()
        .map(|c| c.translation())
        .unwrap_or(Vec3::ZERO);
    for (entity, mut h, mut transform) in &mut flowers {
        h.t += time.delta_secs() / HARVEST_SECS;
        if h.t >= 1.0 {
            commands.entity(entity).despawn();
            continue;
        }
        let e = h.t * h.t * (3.0 - 2.0 * h.t);
        let toward = h.from.lerp(eye - Vec3::Y * 0.5, e * 0.55);
        transform.translation = toward + Vec3::Y * (e * 0.35);
        transform.rotate_y(time.delta_secs() * 7.0);
        transform.scale = Vec3::splat(h.start_scale * (1.0 - e * e));
    }
}

fn animate_particles(
    mut commands: Commands,
    time: Res<Time>,
    mut particles: Query<(Entity, &mut Particle, &mut Transform)>,
) {
    let dt = time.delta_secs();
    for (entity, mut p, mut transform) in &mut particles {
        p.age += dt;
        if p.age >= p.life {
            commands.entity(entity).despawn();
            continue;
        }
        p.velocity.y -= 3.2 * dt; // petals fall softly
        p.velocity *= 1.0 - 1.8 * dt; // air drag
        transform.translation += p.velocity * dt;
        let spin = p.spin * dt;
        transform.rotate(Quat::from_euler(EulerRot::XYZ, spin.x, spin.y, spin.z));
        p.size = 1.0 - (p.age / p.life).powi(2);
        transform.scale = Vec3::new(0.016, 0.004, 0.011) * p.size;
    }
}
