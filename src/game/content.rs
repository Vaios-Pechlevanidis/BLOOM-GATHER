use std::sync::Arc;

use bevy::prelude::*;
use bloom_core::data::RegionDef;
use bloom_core::world::{FlowerField, RegionLayout};
use bloom_core::GameData;

use super::AppState;

/// Validated, read-only game content shared by every system.
#[derive(Resource, Clone, Deref)]
pub struct Content(pub Arc<GameData>);

/// Index (into `GameData::regions`) of the region the player is in.
/// Phase F adds travel between regions; until then it is the first region.
#[derive(Resource, Debug, Clone, Copy, Default)]
pub struct ActiveRegion(pub usize);

impl ActiveRegion {
    pub fn def<'a>(&self, content: &'a Content) -> &'a RegionDef {
        &content.regions[self.0]
    }
}

/// While the ending tours the valley, the scene shows other regions but
/// the player still stands in `home`; saves use it.
#[derive(Resource, Debug, Clone, Copy)]
pub struct Touring {
    pub home: usize,
}

/// The generated layout (terrain, props, colliders) and flower field of the
/// region currently built in the 3D scene — the same data the session uses,
/// shared by rendering and the player controller so everyone agrees on where
/// the ground, obstacles and flowers are.
#[derive(Resource, Clone)]
pub struct RegionScene {
    pub region: usize,
    pub layout: Arc<RegionLayout>,
    pub field: Arc<FlowerField>,
}

/// Content is compiled into the binary, so the only way this fails is a
/// developer shipping invalid data — which `cargo test` also catches. If it
/// does happen, log every problem and exit cleanly instead of running a game
/// with broken rules.
pub fn load_content(
    mut commands: Commands,
    mut next_state: ResMut<NextState<AppState>>,
    mut exit: EventWriter<AppExit>,
) {
    let data = match crate::bench::requested() {
        Some(flowers) => bloom_core::bench::benchmark_data(flowers),
        None => GameData::builtin(),
    };
    match data {
        Ok(data) => {
            info!(
                "Loaded game data v{}: {} species, {} tools, {} recipes, {} regions, {} flowers",
                data.version,
                data.species.len(),
                data.tools.len(),
                data.recipes.len(),
                data.regions.len(),
                data.world_flower_total()
            );
            commands.insert_resource(Content(Arc::new(data)));
            commands.insert_resource(ActiveRegion(0));
            next_state.set(AppState::MainMenu);
        }
        Err(err) => {
            error!("{err}");
            exit.write(AppExit::error());
        }
    }
}
