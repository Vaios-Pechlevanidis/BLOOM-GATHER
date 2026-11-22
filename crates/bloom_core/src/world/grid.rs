//! A uniform grid of item indices for fast "what is near here" questions
//! (flowers, trees, rocks, footpath segments). Queries return candidates
//! from the overlapping cells; callers still do the exact test, so using
//! the grid never changes an answer, only how fast it comes.

use glam::Vec2;

#[derive(Debug, Clone)]
pub(crate) struct SpatialGrid {
    origin: Vec2,
    cell: f32,
    nx: usize,
    nz: usize,
    cells: Vec<Vec<u32>>,
}

impl SpatialGrid {
    pub fn new(min: Vec2, max: Vec2, cell: f32) -> Self {
        let size = max - min;
        let nx = (size.x / cell).ceil().max(1.0) as usize;
        let nz = (size.y / cell).ceil().max(1.0) as usize;
        Self {
            origin: min,
            cell,
            nx,
            nz,
            cells: vec![Vec::new(); nx * nz],
        }
    }

    fn cell_of(&self, p: Vec2) -> (usize, usize) {
        let g = (p - self.origin) / self.cell;
        (
            (g.x.floor().max(0.0) as usize).min(self.nx - 1),
            (g.y.floor().max(0.0) as usize).min(self.nz - 1),
        )
    }

    /// Store `i` in the cell containing `p`.
    pub fn insert(&mut self, p: Vec2, i: u32) {
        let (x, z) = self.cell_of(p);
        self.cells[z * self.nx + x].push(i);
    }

    /// Store `i` in every cell the rectangle overlaps.
    pub fn insert_rect(&mut self, min: Vec2, max: Vec2, i: u32) {
        let (x0, z0) = self.cell_of(min);
        let (x1, z1) = self.cell_of(max);
        for z in z0..=z1 {
            for x in x0..=x1 {
                self.cells[z * self.nx + x].push(i);
            }
        }
    }

    /// Items stored in the cell containing `p`.
    pub fn at(&self, p: Vec2) -> &[u32] {
        let (x, z) = self.cell_of(p);
        &self.cells[z * self.nx + x]
    }

    pub fn for_each_in_rect(&self, min: Vec2, max: Vec2, mut f: impl FnMut(u32)) {
        let (x0, z0) = self.cell_of(min);
        let (x1, z1) = self.cell_of(max);
        for z in z0..=z1 {
            for x in x0..=x1 {
                for &i in &self.cells[z * self.nx + x] {
                    f(i);
                }
            }
        }
    }

    /// Whether any point-item lies within `r` of `p`.
    pub fn any_within(&self, p: Vec2, r: f32, positions: &[Vec2]) -> bool {
        let mut found = false;
        self.for_each_in_rect(p - Vec2::splat(r), p + Vec2::splat(r), |i| {
            found |= positions[i as usize].distance_squared(p) < r * r;
        });
        found
    }

    /// Whether `test` holds for any item stored within `r` of `p`.
    pub fn any_near(&self, p: Vec2, r: f32, mut test: impl FnMut(u32) -> bool) -> bool {
        let mut found = false;
        self.for_each_in_rect(p - Vec2::splat(r), p + Vec2::splat(r), |i| {
            found = found || test(i);
        });
        found
    }
}
