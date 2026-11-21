//! Looking at things and pressing E.
//!
//! Every frame a ray from the camera finds the flower in the crosshair
//! (within interact range); E asks the session to harvest it. The session
//! validates everything (reach, cooldown, already picked) — this module only
//! turns input into commands and outcomes into feedback.

use bevy::prelude::*;
use bloom_core::game::{GameError, GameEvent};

use crate::audio::{PlaySfx, Sfx};
use crate::game::{EventsForwarded, GameEventMsg, PlayState, RegionScene, Session};
use crate::input::Bindings;
use crate::player::{CameraUpdated, Player};
use crate::rendering::MainCamera;
use crate::ui::Toast;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookTarget>()
            .add_systems(OnExit(PlayState::Playing), clear_target)
            .add_systems(
                Update,
                (
                    (look, interact)
                        .chain()
                        .after(CameraUpdated)
                        .before(EventsForwarded)
                        .run_if(in_state(PlayState::Playing)),
                    react_to_events.after(EventsForwarded),
                ),
            );
    }
}

/// What the crosshair is on.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq)]
pub struct LookTarget {
    pub flower: Option<u32>,
    pub species: Option<usize>,
}

fn clear_target(mut target: ResMut<LookTarget>) {
    *target = LookTarget::default();
}

fn eye_ray(cameras: &Query<&GlobalTransform, With<MainCamera>>) -> Option<(Vec3, Vec3)> {
    let camera = cameras.single().ok()?;
    Some((camera.translation(), camera.forward().as_vec3()))
}

fn look(
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    scene: Option<Res<RegionScene>>,
    session: Option<ResMut<Session>>,
    mut target: ResMut<LookTarget>,
) {
    let (Some(scene), Some(mut session), Some((eye, dir))) = (scene, session, eye_ray(&cameras))
    else {
        return;
    };
    let flower = session.look_at(scene.region, eye, dir);
    let next = LookTarget {
        flower,
        species: flower.map(|i| scene.field.species(i)),
    };
    if *target != next {
        *target = next;
    }
}

fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    target: Res<LookTarget>,
    cameras: Query<&GlobalTransform, With<MainCamera>>,
    scene: Option<Res<RegionScene>>,
    session: Option<ResMut<Session>>,
    mut sfx: EventWriter<PlaySfx>,
    mut toasts: EventWriter<Toast>,
) {
    if !Bindings::just_pressed(&keys, &bindings.interact) {
        return;
    }
    let (Some(scene), Some(mut session), Some((eye, _))) = (scene, session, eye_ray(&cameras))
    else {
        return;
    };
    let Some(flower) = target.flower else { return };
    match session.harvest(scene.region, flower, eye) {
        Ok(_) => {}
        // Pressing again a moment too early is not an error worth a message.
        Err(GameError::CoolingDown { .. }) => {}
        Err(err) => {
            sfx.write(PlaySfx::new(Sfx::Deny));
            toasts.write(Toast::info(capitalise(&err.to_string())));
        }
    }
}

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next()
        .map(|f| f.to_uppercase().collect::<String>() + c.as_str())
        .unwrap_or_default()
}

/// Sounds, view nudges and toasts for things that happened in the game.
fn react_to_events(
    mut events: EventReader<GameEventMsg>,
    session: Option<Res<Session>>,
    player: Option<ResMut<Player>>,
    mut sfx: EventWriter<PlaySfx>,
    mut toasts: EventWriter<Toast>,
) {
    let Some(session) = session else { return };
    let data = session.data().clone();
    let mut player = player;
    for GameEventMsg(event) in events.read() {
        match *event {
            GameEvent::FlowerHarvested { flower, .. } => {
                // Slight pitch variety so repeated picking doesn't sound mechanical.
                let pitch = 0.92 + (flower % 7) as f32 * 0.025;
                sfx.write(PlaySfx {
                    sfx: Sfx::Pick,
                    pitch,
                });
                if let Some(p) = player.as_mut() {
                    p.nudge(0.02);
                }
            }
            GameEvent::SpeciesDiscovered { species } => {
                let s = &data.species[species];
                sfx.write(PlaySfx::new(Sfx::Discover));
                toasts.write(Toast::highlight(
                    format!("New species discovered: {}", s.name),
                    s.description.clone(),
                ));
            }
            GameEvent::LevelUp { level } => {
                sfx.write(PlaySfx::new(Sfx::LevelUp));
                toasts.write(Toast::highlight(
                    format!("Level {level}!"),
                    "Your gathering grows more practised.".into(),
                ));
            }
            GameEvent::RegionCompleted { region, coins } => {
                sfx.write(PlaySfx::new(Sfx::LevelUp));
                toasts.write(Toast::highlight(
                    format!("{} complete!", data.regions[region].name),
                    format!("Every flower gathered. +{coins} coins"),
                ));
            }
            GameEvent::FlowersSold { .. } => {
                sfx.write(PlaySfx::new(Sfx::Coins));
            }
        }
    }
}
