//! Flowers in the 3D world: one entity per unharvested flower (plus one
//! child for the glowing parts of night flowers), swaying in the wind on
//! the GPU (the wind shader; nothing moves on the CPU),
//! look-at highlight, and the pick animation with a burst of petals.
//!
//! Which flowers exist comes only from the session (`bloom_core`): this
//! module mirrors it. A harvested flower is despawned after its animation and
//! never spawned again — on reload, harvested indices are simply skipped.

pub mod imported;
pub mod models;
mod sparkles;
mod sprites;

/// Flower pictures and models from `assets/flowers`, embedded by `build.rs`.
mod embedded {
    include!(concat!(env!("OUT_DIR"), "/flower_sprites.rs"));
}

use std::collections::HashMap;

use bevy::prelude::*;
use bloom_core::game::GameEvent;

use bloom_core::world::FlowerField;

use crate::game::{Content, EventsForwarded, GameEventMsg, RegionScene, Session};
use crate::interaction::LookTarget;
use crate::rendering::{color, sway_by_height, wind_material, MainCamera, Wind, WindMaterial};

pub struct FlowersPlugin;

impl Plugin for FlowersPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<SpawnedFlowers>()
            .init_resource::<MemoryBloom>()
            .init_resource::<NearFlowers>()
            .add_plugins(sparkles::SparklesPlugin)
            .add_systems(
                Update,
                (
                    (ensure_assets, sync_flowers, level_of_detail, face_player).chain(),
                    highlight,
                    start_harvest_animation.after(EventsForwarded),
                    animate_harvest,
                    animate_particles,
                    (sync_memory, grow_memory).chain(),
                    glow_with_daylight,
                ),
            );
    }
}

/// The ending's "blooming in memory": while `active`, every picked flower of
/// the region on screen reappears as a soft, glowing, see-through bloom.
/// Purely visual; the flowers stay picked.
#[derive(Resource, Default)]
pub struct MemoryBloom {
    pub active: bool,
    /// Region the ghosts were spawned for.
    built: Option<usize>,
    entities: Vec<Entity>,
    material: Option<Handle<StandardMaterial>>,
    /// Ghost materials for species drawn from a picture.
    pictured: HashMap<usize, Handle<StandardMaterial>>,
    orbs: HashMap<usize, Handle<StandardMaterial>>,
}

/// One remembered flower, growing in after `delay` seconds.
#[derive(Component)]
struct Memory {
    delay: f32,
    age: f32,
    scale: f32,
}

/// Flowers near the camera get the full model and cast shadows; the rest
/// use a simple stand-in (see `models::build_far`). This set holds the near
/// ones; it is refreshed a few times a second from the flower grid.
#[derive(Resource, Default)]
struct NearFlowers {
    set: std::collections::HashSet<u32>,
    key: Option<(u64, usize)>,
    since: f32,
}

#[derive(Resource)]
struct FlowerAssets {
    plants: Vec<Handle<Mesh>>,
    /// Distant stand-ins, per species.
    fars: Vec<Handle<Mesh>>,
    glows: Vec<Option<Handle<Mesh>>>,
    /// Per species; procedural species share one handle.
    materials: Vec<Handle<WindMaterial>>,
    highlights: Vec<Handle<WindMaterial>>,
    glow_materials: Vec<Option<Handle<WindMaterial>>>,
    /// The painted picture, for species drawn from one (`sprites`).
    pictures: Vec<Option<Handle<Image>>>,
    particle: Handle<Mesh>,
    particle_materials: Vec<Handle<StandardMaterial>>,
}

/// Root of one flower in the world.
#[derive(Component)]
pub struct FlowerEntity;

/// A flower drawn from a picture: its card turns to face the player.
#[derive(Component)]
struct Billboard;

/// Which flowers are currently spawned, by index.
#[derive(Resource, Default)]
pub struct SpawnedFlowers {
    /// (session generation, region) the entities were built for.
    key: Option<(u64, usize)>,
    pub map: HashMap<u32, Entity>,
    /// Highlighted flowers and their species.
    highlighted: Vec<(Entity, usize)>,
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

/// Lights a looked-at flower gently from within.
const HIGHLIGHT: LinearRgba = LinearRgba::new(0.10, 0.09, 0.05, 1.0);

#[allow(clippy::too_many_arguments)]
fn ensure_assets(
    mut commands: Commands,
    assets: Option<Res<FlowerAssets>>,
    content: Option<Res<Content>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut images: ResMut<Assets<Image>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut wind_materials: ResMut<Assets<WindMaterial>>,
) {
    let (None, Some(content)) = (assets, content) else {
        return;
    };
    // Flowers lean in the wind: rooted at the ground, freest at the bloom.
    let wind = || Wind::new(0.06, 1.7, 0.25);
    let started = std::time::Instant::now();
    // Procedural models are vertex-coloured and all share one material
    // (and one highlight), so they batch together.
    let shared = wind_materials.add(wind_material(
        StandardMaterial {
            base_color: Color::WHITE,
            perceptual_roughness: 0.72,
            reflectance: 0.2,
            ..default()
        },
        wind(),
    ));
    // Subtle highlight: the same vertex colours, gently lit from within.
    let shared_highlight = wind_materials.add(wind_material(
        StandardMaterial {
            base_color: Color::WHITE,
            emissive: HIGHLIGHT,
            perceptual_roughness: 0.6,
            ..default()
        },
        wind(),
    ));
    let mut plants = Vec::new();
    let mut fars = Vec::new();
    let mut glows = Vec::new();
    let mut glow_materials = Vec::new();
    let mut bodies = Vec::new();
    let mut highlights = Vec::new();
    let mut pictures = Vec::new();
    let mut particle_materials = Vec::new();
    let mut imported = 0;
    for species in &content.species {
        let petal = color(species.petal_color);
        particle_materials.push(materials.add(StandardMaterial {
            base_color: petal,
            perceptual_roughness: 0.7,
            ..default()
        }));
        if let Some(mut plant) = imported::load(species) {
            // A model from a 3D tool: vertex-coloured like the procedural
            // models, so it shares their material. Not a night flower's
            // glow (the model has no separate glowing parts).
            let mut far = match imported::load_version(species, "low") {
                Some(low) => low.into_mesh(),
                None => models::build_far(species, &plant),
            };
            sway_by_height(&mut plant, 0.0, species.height, 1.0);
            sway_by_height(&mut far, 0.0, species.height, 1.0);
            plants.push(meshes.add(plant));
            fars.push(meshes.add(far));
            glows.push(None);
            glow_materials.push(None);
            bodies.push(shared.clone());
            highlights.push(shared_highlight.clone());
            pictures.push(None);
            imported += 1;
            continue;
        }
        if let Some(sprite) = sprites::load(species) {
            // A painted picture on a card: cheap enough to serve near and
            // far alike.
            let mut plant = sprite.mesh;
            sway_by_height(&mut plant, 0.0, species.height, 1.0);
            let plant = meshes.add(plant);
            plants.push(plant.clone());
            fars.push(plant);
            glows.push(None);
            glow_materials.push(None);
            let picture = images.add(sprite.image);
            let glow = sprite.glow.map(|g| images.add(g));
            let base = StandardMaterial {
                base_color: Color::WHITE,
                base_color_texture: Some(picture.clone()),
                alpha_mode: AlphaMode::Mask(sprites::CUTOFF),
                emissive: if glow.is_some() {
                    petal.to_linear() * (species.glow.max(0.2) * 4.0)
                } else {
                    LinearRgba::BLACK
                },
                emissive_texture: glow,
                perceptual_roughness: 0.8,
                reflectance: 0.2,
                ..default()
            };
            highlights.push(wind_materials.add(wind_material(
                StandardMaterial {
                    emissive: base.emissive + HIGHLIGHT,
                    ..base.clone()
                },
                wind(),
            )));
            bodies.push(wind_materials.add(wind_material(base, wind())));
            pictures.push(Some(picture));
            continue;
        }
        let model = models::build(species);
        let mut plant = model.plant;
        let mut far = models::build_far(species, &plant);
        sway_by_height(&mut plant, 0.0, species.height, 1.0);
        sway_by_height(&mut far, 0.0, species.height, 1.0);
        plants.push(meshes.add(plant));
        fars.push(meshes.add(far));
        glows.push(model.glow.map(|mut m| {
            with_sway_colors(&mut m);
            sway_by_height(&mut m, 0.0, species.height, 1.0);
            meshes.add(m)
        }));
        glow_materials.push((species.glow > 0.0).then(|| {
            wind_materials.add(wind_material(
                StandardMaterial {
                    base_color: Color::WHITE,
                    emissive: petal.to_linear() * (species.glow * 4.0),
                    perceptual_roughness: 0.6,
                    ..default()
                },
                wind(),
            ))
        }));
        bodies.push(shared.clone());
        highlights.push(shared_highlight.clone());
        pictures.push(None);
    }
    let triangles: usize = plants
        .iter()
        .filter_map(|h| meshes.get(h))
        .map(|m| m.indices().map_or(0, |i| i.len() / 3))
        .sum();
    info!(
        "Built {} flower models ({} from pictures, {imported} imported) in {:.1} ms ({} triangles on average)",
        plants.len(),
        pictures.iter().flatten().count(),
        started.elapsed().as_secs_f64() * 1000.0,
        triangles / plants.len().max(1)
    );
    commands.insert_resource(FlowerAssets {
        plants,
        fars,
        glows,
        materials: bodies,
        highlights,
        glow_materials,
        pictures,
        particle: meshes.add(models::petal_particle()),
        particle_materials,
    });
}

/// Night flowers glow faintly by day and brightly after dark.
fn glow_with_daylight(
    daylight: Res<crate::rendering::Daylight>,
    assets: Option<Res<FlowerAssets>>,
    content: Option<Res<Content>>,
    mut wind_materials: ResMut<Assets<WindMaterial>>,
    mut last: Local<Option<f32>>,
) {
    let (Some(assets), Some(content)) = (assets, content) else {
        return;
    };
    let glow = daylight.glow();
    if last.is_some_and(|g| (g - glow).abs() < 0.01) && !assets.is_changed() {
        return;
    }
    *last = Some(glow);
    for (species, handle) in content.species.iter().zip(&assets.glow_materials) {
        let Some(handle) = handle else { continue };
        if let Some(mut m) = wind_materials.get_mut(handle) {
            m.base.emissive = color(species.petal_color).to_linear() * (species.glow * 4.0 * glow);
        }
    }
}

/// Flowers are spawned around the camera: everything within the stream
/// radius (all of every real region), and despawned again beyond it plus a
/// margin, so even a field of hundreds of thousands keeps only the nearby
/// ones as entities. Picked flowers are never spawned again.
fn stream_radius(q: crate::game::Quality) -> f32 {
    match q {
        crate::game::Quality::Low => 90.0,
        crate::game::Quality::Medium => 120.0,
        crate::game::Quality::High => 160.0,
    }
}
const STREAM_MARGIN: f32 = 15.0;

fn spawn_flower(
    commands: &mut Commands,
    assets: &FlowerAssets,
    field: &FlowerField,
    i: u32,
) -> Entity {
    let species = field.species(i);
    let ground = field.ground_point(i);
    let root = commands
        .spawn((
            FlowerEntity,
            Transform::from_translation(ground)
                .with_rotation(Quat::from_rotation_y(field.yaw(i)))
                .with_scale(Vec3::splat(field.scale(i))),
            // Far stand-in until the level-of-detail pass sees it near.
            Mesh3d(assets.fars[species].clone()),
            MeshMaterial3d(assets.materials[species].clone()),
            bevy::light::NotShadowCaster,
        ))
        .id();
    if assets.pictures[species].is_some() {
        commands.entity(root).insert(Billboard);
    }
    if let (Some(mesh), Some(mat)) = (&assets.glows[species], &assets.glow_materials[species]) {
        commands
            .entity(root)
            .with_child((Mesh3d(mesh.clone()), MeshMaterial3d(mat.clone())));
    }
    root
}

/// Keep the flowers around the camera spawned (and only those). Starts over
/// when the session or region changes.
#[allow(clippy::too_many_arguments)]
fn sync_flowers(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<crate::game::Settings>,
    assets: Option<Res<FlowerAssets>>,
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    mut spawned: ResMut<SpawnedFlowers>,
    mut near: ResMut<NearFlowers>,
    mut since: Local<f32>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
) {
    let (Some(assets), Some(session), Some(scene)) = (assets, session, scene) else {
        return;
    };
    let key = (session.generation, scene.region);
    let fresh = spawned.key != Some(key);
    if fresh {
        for (_, entity) in spawned.map.drain() {
            commands.entity(entity).despawn();
        }
        spawned.highlighted.clear();
        spawned.key = Some(key);
    }
    *since += time.delta_secs();
    if !fresh && *since < 0.25 && !settings.is_changed() {
        return;
    }
    *since = 0.0;
    let field = &scene.field;
    // Around the camera; before it is placed, around the region's start.
    let at = cameras
        .single()
        .map(|c| Vec2::new(c.translation().x, c.translation().z))
        .unwrap_or(scene.layout.spawn);
    let radius = stream_radius(settings.quality);
    let none = bloom_core::bitset::BitSet::new(field.len() as u32);
    let mut added = 0;
    for i in field.query_radius(at, radius, &none) {
        if spawned.map.contains_key(&i) || session.is_harvested(scene.region, i) {
            continue;
        }
        let e = spawn_flower(&mut commands, &assets, field, i);
        spawned.map.insert(i, e);
        added += 1;
    }
    let far = radius + STREAM_MARGIN;
    let leaving: Vec<u32> = spawned
        .map
        .keys()
        .copied()
        .filter(|&i| field.position(i).distance(at) > far)
        .collect();
    for i in &leaving {
        if let Some(e) = spawned.map.remove(i) {
            commands.entity(e).despawn();
        }
        near.set.remove(i);
    }
    if fresh {
        info!(
            "Spawned {added} flowers ({} already picked)",
            (0..field.len() as u32)
                .filter(|&i| session.is_harvested(scene.region, i))
                .count()
        );
    }
}

/// Full models (with shadows) within this distance of the camera.
fn near_distance(q: crate::game::Quality) -> f32 {
    match q {
        crate::game::Quality::Low => 12.0,
        crate::game::Quality::Medium => 22.0,
        // 26 m keeps headroom under 60 Hz at 2560x1600 on an M2 (Bevy 0.20).
        crate::game::Quality::High => 26.0,
    }
}

#[allow(clippy::too_many_arguments)]
fn level_of_detail(
    mut commands: Commands,
    time: Res<Time<Real>>,
    settings: Res<crate::game::Settings>,
    assets: Option<Res<FlowerAssets>>,
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    spawned: Res<SpawnedFlowers>,
    mut near: ResMut<NearFlowers>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut meshes: Query<&mut Mesh3d, With<FlowerEntity>>,
) {
    let (Some(assets), Some(session), Some(scene)) = (assets, session, scene) else {
        return;
    };
    let key = (session.generation, scene.region);
    if near.key != Some(key) {
        // A fresh set of flowers (all far): start over.
        near.set.clear();
        near.key = Some(key);
        near.since = f32::MAX;
    }
    near.since += time.delta_secs();
    if near.since < 0.15 && !settings.is_changed() {
        return;
    }
    near.since = 0.0;
    let Ok(camera) = cameras.single() else { return };
    let at = camera.translation();
    let field = &scene.field;
    let none = bloom_core::bitset::BitSet::new(field.len() as u32);
    let radius = near_distance(settings.quality);
    let now: std::collections::HashSet<u32> = field
        .query_radius(Vec2::new(at.x, at.z), radius, &none)
        .into_iter()
        .filter(|i| spawned.map.contains_key(i))
        .collect();
    let shadows = settings.quality.shadows();
    for &i in now.difference(&near.set) {
        let Some(&e) = spawned.map.get(&i) else {
            continue;
        };
        if let Ok(mut mesh) = meshes.get_mut(e) {
            mesh.0 = assets.plants[field.species(i)].clone();
        }
        if shadows {
            commands.entity(e).remove::<bevy::light::NotShadowCaster>();
        }
    }
    for &i in near.set.difference(&now) {
        let Some(&e) = spawned.map.get(&i) else {
            continue;
        };
        if let Ok(mut mesh) = meshes.get_mut(e) {
            mesh.0 = assets.fars[field.species(i)].clone();
        }
        commands.entity(e).insert(bevy::light::NotShadowCaster);
    }
    near.set = now;
}

/// Turn every pictured flower's card towards the player (around the
/// upright only, so the plant keeps standing). Flowers being picked spin
/// on their own.
fn face_player(
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    mut cards: Query<&mut Transform, (With<Billboard>, Without<Harvesting>)>,
) {
    let Ok(camera) = cameras.single() else { return };
    let eye = camera.translation();
    for mut transform in &mut cards {
        let to_eye = eye - transform.translation;
        if to_eye.x.abs() + to_eye.z.abs() < 1e-4 {
            continue;
        }
        let facing = Quat::from_rotation_y(to_eye.x.atan2(to_eye.z));
        // Only touch flowers that actually need turning (change detection
        // and transform propagation stay cheap while standing still).
        if transform.rotation.angle_between(facing) > 0.002 {
            transform.rotation = facing;
        }
    }
}

/// Give a mesh white vertex colours (if it has none) so the wind shader can
/// read sway weights from their alpha.
fn with_sway_colors(mesh: &mut Mesh) {
    if mesh.attribute(Mesh::ATTRIBUTE_COLOR).is_none() {
        let n = mesh.count_vertices();
        mesh.insert_attribute(Mesh::ATTRIBUTE_COLOR, vec![[1.0f32, 1.0, 1.0, 1.0]; n]);
    }
}

fn highlight(
    target: Res<LookTarget>,
    assets: Option<Res<FlowerAssets>>,
    scene: Option<Res<RegionScene>>,
    mut spawned: ResMut<SpawnedFlowers>,
    mut bodies: Query<&mut MeshMaterial3d<WindMaterial>, With<FlowerEntity>>,
) {
    let (Some(assets), Some(scene)) = (assets, scene) else {
        return;
    };
    let wanted: Vec<(Entity, usize)> = target
        .flower
        .iter()
        .chain(&target.group)
        .filter_map(|&i| Some((*spawned.map.get(&i)?, scene.field.species(i))))
        .collect();
    if wanted == spawned.highlighted {
        return;
    }
    let mut set = |root: Entity, material: &Handle<WindMaterial>| {
        if let Ok(mut m) = bodies.get_mut(root) {
            m.0 = material.clone();
        }
    };
    for &(old, species) in spawned.highlighted.iter().filter(|e| !wanted.contains(e)) {
        set(old, &assets.materials[species]);
    }
    for &(new, species) in &wanted {
        set(new, &assets.highlights[species]);
    }
    spawned.highlighted = wanted;
}

fn start_harvest_animation(
    mut commands: Commands,
    mut events: MessageReader<GameEventMsg>,
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
        spawned.highlighted.retain(|(e, _)| *e != entity);
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

/// Spawn (or clear) the remembered flowers of the region on screen.
fn sync_memory(
    mut commands: Commands,
    mut memory: ResMut<MemoryBloom>,
    assets: Option<Res<FlowerAssets>>,
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let want = if memory.active {
        scene.as_ref().map(|s| s.region)
    } else {
        None
    };
    if want == memory.built {
        return;
    }
    for e in std::mem::take(&mut memory.entities) {
        commands.entity(e).despawn();
    }
    memory.built = want;
    let (Some(region), Some(assets), Some(session), Some(scene)) = (want, assets, session, scene)
    else {
        return;
    };
    let material = memory
        .material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 1.0, 1.0, 0.7),
                emissive: LinearRgba::rgb(0.8, 0.72, 0.55),
                alpha_mode: AlphaMode::Blend,
                perceptual_roughness: 0.5,
                ..default()
            })
        })
        .clone();
    let field = &scene.field;
    // Bloom outwards from the region's start, like a wave of memories.
    let start = scene.layout.spawn;
    for i in 0..field.len() as u32 {
        if !session.is_harvested(region, i) {
            continue;
        }
        let ground = field.ground_point(i);
        let delay = field.position(i).distance(start) * 0.035;
        let species = field.species(i);
        // A small light in the flower's colour, so the field of memories
        // reads from afar.
        let orb = memory
            .orbs
            .entry(species)
            .or_insert_with(|| {
                let petal = color(session.data().species[species].petal_color).to_linear();
                materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(petal * 2.2),
                    unlit: true,
                    ..default()
                })
            })
            .clone();
        // A picture is cut out by its own alpha, faded like the rest.
        let ghost = match &assets.pictures[species] {
            Some(picture) => memory
                .pictured
                .entry(species)
                .or_insert_with(|| {
                    let faded = materials.get(&material).cloned().unwrap_or_default();
                    materials.add(StandardMaterial {
                        base_color_texture: Some(picture.clone()),
                        ..faded
                    })
                })
                .clone(),
            None => material.clone(),
        };
        let bloom_height = (field.pick_sphere(i).0.y - ground.y) / field.scale(i);
        let e = commands
            .spawn((
                Memory {
                    delay,
                    age: 0.0,
                    scale: field.scale(i) * 1.3,
                },
                Mesh3d(assets.plants[species].clone()),
                MeshMaterial3d(ghost),
                Transform::from_translation(ground)
                    .with_rotation(Quat::from_rotation_y(field.yaw(i)))
                    .with_scale(Vec3::splat(0.001)),
                bevy::light::NotShadowCaster,
                children![(
                    Mesh3d(assets.particle.clone()),
                    MeshMaterial3d(orb),
                    Transform::from_xyz(0.0, bloom_height + 0.05, 0.0)
                        .with_scale(Vec3::splat(0.03)),
                    bevy::light::NotShadowCaster,
                )],
            ))
            .id();
        if assets.pictures[species].is_some() {
            commands.entity(e).insert(Billboard);
        }
        memory.entities.push(e);
    }
}

fn grow_memory(time: Res<Time>, mut ghosts: Query<(&mut Memory, &mut Transform)>) {
    let dt = time.delta_secs();
    for (mut m, mut t) in &mut ghosts {
        m.age += dt;
        let k = ((m.age - m.delay) / 1.2).clamp(0.0, 1.0);
        // Ease out with a little overshoot, like a bud opening.
        let grow = 1.0 - (1.0 - k).powi(3) + (k * std::f32::consts::PI).sin() * 0.08;
        t.scale = Vec3::splat((m.scale * grow).max(0.001));
    }
}
