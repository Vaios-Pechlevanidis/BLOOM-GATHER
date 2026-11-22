//! Flower Recognition: with the skill learned, every flower of a species the
//! player hasn't discovered yet has a small glint bobbing above it, so new
//! species can be spotted across the field. Glints vanish as soon as that
//! species is discovered (or the flower is picked).

use bevy::prelude::*;

use super::SpawnedFlowers;
use crate::game::{RegionScene, Session};

pub struct SparklesPlugin;

impl Plugin for SparklesPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, (sync_sparkles, animate_sparkles).chain());
    }
}

#[derive(Component)]
struct Sparkle {
    base: Vec3,
    phase: f32,
}

#[derive(Resource)]
struct SparkleAssets {
    mesh: Handle<Mesh>,
    material: Handle<StandardMaterial>,
}

/// What the glints were last built for.
#[derive(PartialEq, Clone)]
struct Built {
    generation: u64,
    region: usize,
    reveal: bool,
    discovered: usize,
    flowers: usize,
}

#[allow(clippy::too_many_arguments)]
fn sync_sparkles(
    mut commands: Commands,
    session: Option<Res<Session>>,
    scene: Option<Res<RegionScene>>,
    spawned: Res<SpawnedFlowers>,
    existing: Query<Entity, With<Sparkle>>,
    assets: Option<Res<SparkleAssets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut built: Local<Option<Built>>,
) {
    let (Some(session), Some(scene)) = (session, scene) else {
        return;
    };
    let now = Built {
        generation: session.generation,
        region: scene.region,
        reveal: session.modifiers().reveal_undiscovered,
        discovered: session.state().discovered.len(),
        flowers: spawned.map.len(),
    };
    if built.as_ref() == Some(&now) {
        return;
    }
    *built = Some(now.clone());
    for e in &existing {
        commands.entity(e).despawn();
    }
    if !now.reveal {
        return;
    }
    let (mesh, material) = match assets {
        Some(a) => (a.mesh.clone(), a.material.clone()),
        None => {
            let a = SparkleAssets {
                mesh: meshes.add(Sphere::new(0.05).mesh().ico(1).expect("valid ico")),
                // Unlit and brighter than white, so tone mapping still shows
                // a warm golden glint rather than a grey dot.
                material: materials.add(StandardMaterial {
                    base_color: Color::LinearRgba(LinearRgba::rgb(2.4, 1.6, 0.25)),
                    unlit: true,
                    fog_enabled: false,
                    ..default()
                }),
            };
            let pair = (a.mesh.clone(), a.material.clone());
            commands.insert_resource(a);
            pair
        }
    };
    let field = &scene.field;
    for &i in spawned.map.keys() {
        if session.is_discovered(field.species(i)) {
            continue;
        }
        let (center, radius) = field.pick_sphere(i);
        let base = center + Vec3::Y * (radius + 0.2);
        commands.spawn((
            Sparkle {
                base,
                phase: (i as f32 * 2.399).rem_euclid(std::f32::consts::TAU),
            },
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            Transform::from_translation(base),
            bevy::pbr::NotShadowCaster,
        ));
    }
}

fn animate_sparkles(time: Res<Time>, mut sparkles: Query<(&Sparkle, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (s, mut tr) in &mut sparkles {
        let w = t * 2.2 + s.phase;
        tr.translation = s.base + Vec3::Y * (w.sin() * 0.06);
        tr.scale = Vec3::splat(0.75 + 0.35 * (w * 1.7).sin().abs());
        tr.rotation = Quat::from_rotation_y(w);
    }
}
