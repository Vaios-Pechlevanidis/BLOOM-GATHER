//! Finished bouquets on the workshop's display table.
//!
//! The most valuable bouquets the player owns stand in the three vases
//! (several of one kind fill several vases), each built from its recipe: the named flowers' colours (topped up
//! from the meadow's palette), green stems and leaves, and the recipe's
//! ribbon. Sell them and the vases empty again.

use bevy::prelude::*;
use bloom_core::data::GameData;
use bloom_core::world::LandmarkKind;

use super::color;
use super::landmarks::DISPLAY_SLOTS;
use super::meshes::{lin, mix, MeshBuilder};
use super::scene::SceneEntity;
use crate::game::{RegionScene, Session};

pub struct BouquetsPlugin;

impl Plugin for BouquetsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Update, show_bouquets);
    }
}

#[derive(Component)]
struct DisplayedBouquets;

/// Height of the vase rims above the workshop floor.
const VASE_TOP: f32 = 1.05;

fn show_bouquets(
    mut commands: Commands,
    scene: Option<Res<RegionScene>>,
    session: Option<Res<Session>>,
    shown: Query<Entity, With<DisplayedBouquets>>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut last: Local<Option<Vec<usize>>>,
    mut material: Local<Option<Handle<StandardMaterial>>>,
) {
    let (Some(scene), Some(session)) = (scene, session) else {
        *last = None;
        return;
    };
    let data = session.data();
    let mut kinds: Vec<usize> = (0..data.recipes.len())
        .filter(|&r| session.bouquet_count(r) > 0)
        .collect();
    kinds.sort_by_key(|&r| (std::cmp::Reverse(data.recipes[r].sell_price), r));
    let owned: Vec<usize> = kinds
        .iter()
        .flat_map(|&r| std::iter::repeat_n(r, session.bouquet_count(r) as usize))
        .take(DISPLAY_SLOTS.len())
        .collect();
    // The scene being rebuilt despawns the old display along with it.
    if !scene.is_changed() && last.as_ref() == Some(&owned) {
        return;
    }
    *last = Some(owned.clone());
    for e in &shown {
        commands.entity(e).despawn();
    }
    let Some(workshop) = scene
        .layout
        .landmarks
        .iter()
        .find(|l| l.kind == LandmarkKind::Workshop)
    else {
        return;
    };
    let material = material
        .get_or_insert_with(|| {
            materials.add(StandardMaterial {
                base_color: Color::WHITE,
                perceptual_roughness: 0.8,
                reflectance: 0.2,
                ..default()
            })
        })
        .clone();
    commands
        .spawn((
            SceneEntity,
            DisplayedBouquets,
            Transform::from_xyz(workshop.position.x, workshop.base, workshop.position.y)
                .with_rotation(Quat::from_rotation_y(workshop.yaw)),
            Visibility::default(),
        ))
        .with_children(|table| {
            for (i, (slot, &recipe)) in DISPLAY_SLOTS.iter().zip(&owned).enumerate() {
                table.spawn((
                    Mesh3d(meshes.add(bouquet(data, recipe))),
                    MeshMaterial3d(material.clone()),
                    Transform::from_xyz(slot.x, VASE_TOP, slot.y)
                        .with_rotation(Quat::from_rotation_y(recipe as f32 * 1.3 + i as f32 * 2.1)),
                ));
            }
        });
}

/// One bouquet, standing in a vase whose rim is at the origin.
fn bouquet(data: &GameData, recipe: usize) -> Mesh {
    let r = &data.recipes[recipe];
    // Flower colours: the named species first, then a few from the meadow.
    let mut flowers: Vec<(LinearRgba, LinearRgba)> = r
        .required_species
        .iter()
        .filter_map(|req| data.species(&req.species))
        .map(|s| (lin(color(s.petal_color)), lin(color(s.center_color))))
        .collect();
    let n = data.species.len();
    let mut k = 0;
    while flowers.len() < 3 + (r.tier as usize).min(3) {
        let s = &data.species[(recipe * 5 + k * 3) % n.min(6 + recipe)];
        flowers.push((lin(color(s.petal_color)), lin(color(s.center_color))));
        k += 1;
    }
    let ribbon = lin(color(r.ribbon_color));
    let stem = lin(Color::srgb_u8(86, 128, 70));
    let leaf = lin(Color::srgb_u8(108, 152, 84));
    let size = 1.0 + 0.07 * r.tier as f32;

    let mut b = MeshBuilder::default();
    let stem_mesh = Mesh::from(Cylinder::new(0.008, 0.26));
    let head = Sphere::new(1.0).mesh().uv(10, 6);
    let golden = std::f32::consts::PI * (3.0 - 5f32.sqrt());

    // Heads on a dome; stems run from each head down into the vase.
    let heads = 7 + 2 * r.tier as usize;
    for i in 0..heads {
        let t = (i as f32 + 0.5) / heads as f32;
        let ring = (t * 0.9).sqrt() * 0.075 * size;
        let a = i as f32 * golden;
        let pos = Vec3::new(ring * a.cos(), 0.17 * size - ring * 0.9, ring * a.sin());
        let foot = Vec3::new(pos.x * 0.15, -0.1, pos.z * 0.15);
        let mid = (pos + foot) * 0.5;
        let dir = (pos - foot).normalize();
        b.add(
            &stem_mesh,
            Transform::from_translation(mid)
                .with_rotation(Quat::from_rotation_arc(Vec3::Y, dir))
                .with_scale(Vec3::new(1.0, (pos - foot).length() / 0.26, 1.0)),
            move |_| stem,
        );
        let (petal, centre) = flowers[i % flowers.len()];
        let radius = 0.03 * size * (0.85 + 0.3 * ((i * 7 % 5) as f32 / 4.0));
        b.add(
            &head,
            Transform::from_translation(pos)
                .with_rotation(Quat::from_rotation_arc(
                    Vec3::Y,
                    (pos - foot * 2.0).normalize(),
                ))
                .with_scale(Vec3::new(radius, radius * 0.7, radius)),
            move |p| {
                if p.y > 0.75 {
                    mix(petal, centre, (p.y - 0.75) * 4.0)
                } else {
                    petal * (0.8 + 0.2 * (p.y + 1.0) * 0.5)
                }
            },
        );
    }
    // Leaves fanning out under the flowers.
    for i in 0..5 {
        let a = i as f32 * std::f32::consts::TAU / 5.0 + 0.4;
        let out = Vec3::new(a.cos(), 0.0, a.sin());
        b.add(
            &head,
            Transform::from_translation(out * 0.06 * size + Vec3::Y * 0.07)
                .with_rotation(Quat::from_rotation_arc(
                    Vec3::Y,
                    (out + Vec3::Y * 0.6).normalize(),
                ))
                .with_scale(Vec3::new(0.022, 0.006, 0.05) * size),
            move |p| mix(leaf * 0.8, leaf, p.z * 0.5 + 0.5),
        );
    }
    // The ribbon tying the stems just above the rim.
    b.add(
        &Mesh::from(Cylinder::new(0.028, 0.035)),
        Transform::from_xyz(0.0, 0.03, 0.0),
        move |p| ribbon * (0.85 + p.y * 3.0),
    );
    for side in [-1.0f32, 1.0] {
        b.add(
            &Mesh::from(Cuboid::new(0.012, 0.07, 0.004)),
            Transform::from_xyz(side * 0.016, -0.005, 0.03)
                .with_rotation(Quat::from_rotation_z(side * 0.35)),
            move |_| ribbon,
        );
    }
    b.build()
}
