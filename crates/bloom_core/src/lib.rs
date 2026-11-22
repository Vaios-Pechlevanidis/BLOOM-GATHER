//! # bloom_core
//!
//! The engine-independent heart of Bloom & Gather. Everything that decides
//! *what happens* in the game lives here; the Bevy application only decides
//! *how it looks and feels*.
//!
//! This crate deliberately has no dependency on Bevy so that:
//! * gameplay rules can't reach into rendering or UI,
//! * tests and benchmarks run quickly without a GPU,
//! * save files only ever contain types defined here.
//!
//! Contents so far: the data-driven content model ([`data`]), the economy
//! configuration ([`economy`]), the XP/level curve ([`progression`]), a
//! deterministic RNG ([`rng`]), the physical world — terrain, region layout
//! and static collision ([`world`]) — and the first-person character
//! controller ([`player`]).

pub mod bitset;
pub mod data;
pub mod economy;
pub mod game;
pub mod persistence;
pub mod player;
pub mod progression;
pub mod rng;
pub mod world;

pub use data::{DataError, GameData};
pub use game::{
    BatchOutcome, Completion, CraftPlan, Game, GameError, GameEvent, GameState, Modifiers,
    RecipeStatus, SkillStatus, ToolStatus, UpgradeStatus,
};
