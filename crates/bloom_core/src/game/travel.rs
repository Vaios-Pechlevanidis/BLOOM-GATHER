//! Travelling between regions.
//!
//! The regions form a chain in content order. Each region's footpath ends
//! at a gate in its north fence that leads on to the next region; a gate in
//! the south fence leads back. The world map can also take the player to
//! any region already open. Entering a region is the one rule here: it has
//! to be unlocked. Where exactly the player stands is presentation.

use super::{Game, GameError, GameEvent};
use crate::world::GateSide;

/// What still stands between the player and a region (`(needed, have)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RegionRequirements {
    pub level: (u32, u32),
    pub flowers: (u64, u64),
}

impl RegionRequirements {
    pub fn met(&self) -> bool {
        self.level.1 >= self.level.0 && self.flowers.1 >= self.flowers.0
    }
}

impl Game {
    /// The region a gate leads to, if any.
    pub fn gate_destination(&self, region: usize, side: GateSide) -> Option<usize> {
        match side {
            GateSide::North => (region + 1 < self.data.regions.len()).then_some(region + 1),
            GateSide::South => region.checked_sub(1),
        }
    }

    /// The gate of `to` you arrive through when coming from `from` on foot
    /// (`None` if the regions aren't neighbours).
    pub fn arrival_side(&self, from: usize, to: usize) -> Option<GateSide> {
        if to == from + 1 {
            Some(GateSide::South)
        } else if from == to + 1 {
            Some(GateSide::North)
        } else {
            None
        }
    }

    /// Level and harvested flowers a region asks for, and what the player
    /// has (skills that lower the threshold included).
    pub fn region_requirements(&self, region: usize) -> RegionRequirements {
        let unlock = self.data.regions[region].unlock;
        RegionRequirements {
            level: (unlock.level, self.level()),
            flowers: (
                self.region_harvest_threshold(region) as u64,
                self.state.stats.flowers_harvested,
            ),
        }
    }

    pub fn has_visited(&self, region: usize) -> bool {
        region == 0
            || self
                .state
                .visited_regions
                .contains(&self.data.regions[region].id)
    }

    /// Step into a region (through a gate or from the map). Refused while it
    /// is locked; the first visit is announced.
    pub fn enter_region(&mut self, region: usize) -> Result<(), GameError> {
        if region >= self.data.regions.len() {
            return Err(GameError::UnknownRegion);
        }
        if !self.is_region_unlocked(region) {
            return Err(GameError::RegionLocked);
        }
        let first_visit = self
            .state
            .visited_regions
            .insert(self.data.regions[region].id.clone());
        if first_visit {
            self.dirty = true;
        }
        self.events.push(GameEvent::RegionEntered {
            region,
            first_visit: first_visit && region != 0,
        });
        Ok(())
    }
}
