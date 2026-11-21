//! The physical world: terrain, region layout and static collision.

pub mod collision;
pub mod layout;
pub mod terrain;

pub use collision::{BodySlab, BoxCollider, CircleCollider, ColliderSet};
pub use layout::{RegionLayout, RockProp, TreeProp};
pub use terrain::{FlattenArea, Terrain};
