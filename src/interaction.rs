//! Looking at things and pressing E.
//!
//! Every frame a ray from the camera finds what is in the crosshair within
//! interact range: a workshop station (workbench, tool shelf, storage chest,
//! bouquet display) or, with hands/scissors, a flower. E uses the station
//! (opening its screen) or asks the session to pick the flower. The session
//! validates everything; this module turns input into commands and outcomes
//! into feedback.

use bevy::prelude::*;
use bloom_core::data::HarvestMode;
use bloom_core::game::{GameError, GameEvent};
use bloom_core::world::InteractableKind;

use crate::audio::{PlaySfx, Sfx};
use crate::game::{EventsForwarded, GameEventMsg, PlayState, RegionScene, Session};
use crate::input::Bindings;
use crate::player::{CameraUpdated, Player};
use crate::rendering::MainCamera;
use crate::tools::SelectedMode;
use crate::ui::Toast;

pub struct InteractionPlugin;

impl Plugin for InteractionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LookTarget>()
            .init_resource::<OpenScreen>()
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

/// What the crosshair is on: a workshop station, a single flower
/// (hands/scissors), or the group of flowers a basket, rake or cart would
/// gather.
#[derive(Resource, Debug, Default, Clone, PartialEq)]
pub struct LookTarget {
    pub flower: Option<u32>,
    pub species: Option<usize>,
    pub group: Vec<u32>,
    /// A station takes precedence over flowers and tools.
    pub station: Option<InteractableKind>,
}

/// Which screen (and tab) a station asked to open; read when it opens.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct OpenScreen(pub Option<InteractableKind>);

/// Player-facing name and action of a station.
pub fn station_text(kind: InteractableKind) -> (&'static str, &'static str) {
    match kind {
        InteractableKind::Workbench => ("Workbench", "Bind bouquets"),
        InteractableKind::ToolShelf => ("Tool shelf", "Buy tools"),
        InteractableKind::Storage => ("Storage chest", "Open basket"),
        InteractableKind::BouquetDisplay => ("Bouquet display", "Sell bouquets"),
    }
}

fn clear_target(mut target: ResMut<LookTarget>) {
    *target = LookTarget::default();
}

fn eye_ray(cameras: &Query<&Transform, With<MainCamera>>) -> Option<(Vec3, Vec3)> {
    let camera = cameras.single().ok()?;
    Some((camera.translation, camera.forward().as_vec3()))
}

fn look(
    cameras: Query<&Transform, With<MainCamera>>,
    scene: Option<Res<RegionScene>>,
    session: Option<ResMut<Session>>,
    selected: Res<SelectedMode>,
    content: Res<crate::game::Content>,
    mut target: ResMut<LookTarget>,
) {
    if let (Some(scene), Some((eye, dir))) = (scene.as_deref(), eye_ray(&cameras)) {
        let station = scene
            .layout
            .pick_interactable(eye, dir, content.player.interact_range)
            .map(|h| h.0);
        if target.station != station {
            target.station = station;
        }
    }
    if selected.0 != HarvestMode::Single || target.station.is_some() {
        if target.flower.is_some() {
            target.flower = None;
            target.species = None;
        }
        return;
    }
    let (Some(scene), Some(mut session), Some((eye, dir))) = (scene, session, eye_ray(&cameras))
    else {
        return;
    };
    let flower = session.look_at(scene.region, eye, dir);
    let species = flower.map(|i| scene.field.species(i));
    if target.flower != flower {
        target.flower = flower;
        target.species = species;
    }
}

#[allow(clippy::too_many_arguments)]
fn interact(
    keys: Res<ButtonInput<KeyCode>>,
    bindings: Res<Bindings>,
    target: Res<LookTarget>,
    selected: Res<SelectedMode>,
    cameras: Query<&Transform, With<MainCamera>>,
    scene: Option<Res<RegionScene>>,
    session: Option<ResMut<Session>>,
    mut open: ResMut<OpenScreen>,
    mut next: ResMut<NextState<PlayState>>,
    mut sfx: EventWriter<PlaySfx>,
    mut toasts: EventWriter<Toast>,
) {
    if !Bindings::just_pressed(&keys, &bindings.interact) {
        return;
    }
    if let Some(station) = target.station {
        open.0 = Some(station);
        sfx.write(PlaySfx::new(Sfx::Click));
        next.set(match station {
            InteractableKind::Workbench | InteractableKind::BouquetDisplay => PlayState::Workshop,
            InteractableKind::ToolShelf | InteractableKind::Storage => PlayState::Inventory,
        });
        return;
    }
    if selected.0 != HarvestMode::Single {
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
                    level_up_text(&session, level),
                ));
            }
            GameEvent::AchievementUnlocked { achievement } => {
                let a = &data.achievements[achievement];
                sfx.write(PlaySfx::new(Sfx::Discover));
                let mut body = a.description.clone();
                let r = a.reward;
                match (r.coins, r.skill_points) {
                    (0, 0) => {}
                    (c, 0) => body.push_str(&format!(" +{c} coins")),
                    (0, p) => body.push_str(&format!(" +{p} skill point")),
                    (c, p) => body.push_str(&format!(" +{c} coins, +{p} skill point")),
                }
                toasts.write(Toast::highlight(format!("Achievement: {}", a.name), body));
            }
            GameEvent::RegionCompleted { region, coins } => {
                sfx.write(PlaySfx::new(Sfx::LevelUp));
                toasts.write(Toast::highlight(
                    format!("{} complete!", data.regions[region].name),
                    format!("Every flower gathered. +{coins} coins"),
                ));
            }
            GameEvent::ToolBought { tool } => {
                sfx.write(PlaySfx::new(Sfx::Coins));
                toasts.write(Toast::highlight(
                    format!("New tool: {}", data.tools[tool].name),
                    data.tools[tool].description.clone(),
                ));
            }
            GameEvent::FlowersSold { .. } | GameEvent::BouquetsSold { .. } => {
                sfx.write(PlaySfx::new(Sfx::Coins));
            }
            GameEvent::RecipeDiscovered { recipe } => {
                let r = &data.recipes[recipe];
                sfx.write(PlaySfx::new(Sfx::Discover));
                toasts.write(Toast::highlight(
                    format!("New recipe: {}", r.name),
                    r.description.clone(),
                ));
            }
            GameEvent::CraftStarted { .. } => {
                sfx.write(PlaySfx::new(Sfx::Click));
            }
            GameEvent::BouquetsReady {
                recipe,
                count,
                first,
            } => {
                let r = &data.recipes[recipe];
                sfx.write(PlaySfx::new(Sfx::Discover));
                let title = if count == 1 {
                    format!("{} is ready", r.name)
                } else {
                    format!("{count} {}s are ready", r.name)
                };
                let price = session.bouquet_price(recipe);
                let body = if first {
                    format!("Your first {}! It sells for {price} coins.", r.name)
                } else {
                    format!("Each sells for {price} coins at the display.")
                };
                toasts.write(Toast::highlight(title, body));
            }
            GameEvent::UpgradeBought { upgrade, level } => {
                let u = &data.upgrades[upgrade];
                sfx.write(PlaySfx::new(Sfx::Coins));
                toasts.write(Toast::highlight(
                    format!("{} level {level}", u.name),
                    u.description.clone(),
                ));
            }
            GameEvent::SkillLearned { skill, level } => {
                let k = &data.skills[skill];
                sfx.write(PlaySfx::new(Sfx::LevelUp));
                let title = if k.max_level > 1 {
                    format!("Skill learned: {} {level}", k.name)
                } else {
                    format!("Skill learned: {}", k.name)
                };
                toasts.write(Toast::highlight(title, k.description.clone()));
            }
            GameEvent::RegionUnlocked { region, coins, xp } => {
                let r = &data.regions[region];
                sfx.write(PlaySfx::new(Sfx::Discover));
                let body = if coins > 0 || xp > 0 {
                    format!("{} Unlock bonus: +{coins} coins, +{xp} XP", r.description)
                } else {
                    r.description.clone()
                };
                toasts.write(Toast::highlight(format!("{} is open", r.name), body));
            }
            GameEvent::FlowersSaved { species, count } => {
                let name = &data.species[species].name;
                toasts.write(Toast::info(if count == 1 {
                    format!("Efficient Production saved a {name}")
                } else {
                    format!("Efficient Production saved {count} {name}")
                }));
            }
            GameEvent::CollectionMilestone { species_found } => {
                sfx.write(PlaySfx::new(Sfx::LevelUp));
                toasts.write(Toast::highlight(
                    format!("{species_found} species discovered!"),
                    "Collection milestone: +1 skill point.".into(),
                ));
            }
        }
    }
}

/// What a new level brings: skill points and anything that opens at it.
fn level_up_text(game: &bloom_core::Game, level: u32) -> String {
    let data = game.data();
    let mut parts = Vec::new();
    let points = data.progression.skill_points_per_level;
    if points > 0 {
        parts.push(format!(
            "+{points} skill point{} (K)",
            if points == 1 { "" } else { "s" }
        ));
    }
    let tools: Vec<&str> = data
        .tools
        .iter()
        .filter(|t| t.unlock_level == level)
        .map(|t| t.name.as_str())
        .collect();
    if !tools.is_empty() {
        parts.push(format!("now at the tool shelf: {}", tools.join(", ")));
    }
    // Regions whose level is now reached but which still wait for flowers
    // (ones that open right away get their own announcement).
    for (r, region) in data.regions.iter().enumerate() {
        if region.unlock.level == level && level > 1 && !game.is_region_unlocked(r) {
            parts.push(format!(
                "{} opens after {} flowers picked ({} so far)",
                region.name,
                game.region_harvest_threshold(r),
                game.state().stats.flowers_harvested
            ));
        }
    }
    if parts.is_empty() {
        return "Your gathering grows more practised.".into();
    }
    let mut text = parts.join(". ");
    if let Some(first) = text.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    text.push('.');
    text
}
