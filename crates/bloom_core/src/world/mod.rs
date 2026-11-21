//! The physical world: terrain, region layout, static collision and the
//! harvestable flower field.

pub mod collision;
pub mod flowers;
pub mod layout;
pub mod terrain;

pub use collision::{BodySlab, BoxCollider, CircleCollider, ColliderSet};
pub use flowers::{FlowerField, PlacementError};
pub use layout::{
    Interactable, InteractableKind, Landmark, LandmarkKind, RegionLayout, RockProp, TreeProp,
};
pub use terrain::{FlattenArea, Terrain};
