//! The physical world: terrain, region layout, static collision and the
//! harvestable flower field.

pub mod collision;
pub mod features;
pub mod fields;
pub mod flowers;
mod grid;
pub mod layout;
pub mod terrain;

pub use collision::{BodySlab, BoxCollider, CircleCollider, ColliderSet};
pub use features::{Bridge, Gate, GateSide, Water, WaterKind};
pub use fields::{Bed, FieldPlan};
pub use flowers::{FlowerField, PlacementError};
pub use layout::{
    Interactable, InteractableKind, Landmark, LandmarkKind, RegionLayout, RockProp, TreeProp,
};
pub use terrain::{FlattenArea, Terrain};
