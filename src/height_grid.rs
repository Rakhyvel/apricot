//! A finite square grid of heights, for terrain you shape and erode as a whole

use nalgebra_glm::{Vec2, Vec3};

use crate::{noise, terrain};

/// A square grid of heights, `width` points on a side, one unit apart. Points are addressed by
/// (x, z), and anything off the edge reads as height 0.
#[derive(Default)]
pub struct HeightGrid {
    pub(crate) cells: Vec<Cell>,
    pub(crate) width: usize,
}

#[derive(Default, Copy, Clone)]
pub(crate) struct Cell {
    pub height: f32,
    pub flow: f32,
}

impl HeightGrid {
    /// A flat grid at height 0
    pub fn new(width: usize) -> Self {
        Self {
            cells: vec![Cell::default(); width * width],
            width,
        }
    }

    /// A grid filled with `noise::perlin`, scaled by `amplitude`. `offset` moves where in the
    /// noise it samples from
    pub fn from_noise(
        width: usize,
        frequency: f32,
        octaves: i32,
        seed: i32,
        amplitude: f32,
        offset: Vec2,
    ) -> Self {
        let mut grid = Self::new(width);
        for z in 0..width {
            for x in 0..width {
                let n = noise::perlin(
                    x as f32 + offset.x,
                    z as f32 + offset.y,
                    frequency,
                    octaves,
                    seed,
                );
                grid.cells[x + z * width].height = n * amplitude;
            }
        }
        grid
    }

    /// How many points there are along each side
    pub fn width(&self) -> usize {
        self.width
    }

    /// The height of the grid point at `p`, rounding down to the nearest point. 0 off the edge
    pub fn height(&self, p: Vec2) -> f32 {
        self.index(p).map_or(0.0, |i| self.cells[i].height)
    }

    /// How much water has run over the point at `p`. 0 off the edge
    pub fn flow(&self, p: Vec2) -> f32 {
        self.index(p).map_or(0.0, |i| self.cells[i].flow)
    }

    /// The height between grid points
    pub fn surface_height(&self, p: Vec2) -> f32 {
        terrain::surface_height(|x, z| self.height(nalgebra_glm::vec2(x, z)), p.x, p.y)
    }

    /// The Y-up normal of the flat triangle under `p`
    pub fn surface_normal(&self, p: Vec2) -> Vec3 {
        terrain::surface_normal(|x, z| self.height(nalgebra_glm::vec2(x, z)), p.x, p.y)
    }

    /// Push the heights toward the extremes around 0.5, sharpening peaks and deepening valleys.
    /// Expects heights from 0 to 1, like after `normalize`
    pub fn create_bulge(&mut self) {
        for cell in &mut self.cells {
            cell.height = (cell.height - 0.5).powf(3.0) * 150.0 + 0.55;
        }
    }

    /// Drop everything between `shelf_depth` and `shelf_height` down to `shelf_depth`, making a
    /// sudden drop-off like a continental shelf. Anything lower or higher stays put
    pub fn create_shelf(&mut self, shelf_height: f32, shelf_depth: f32) {
        for cell in &mut self.cells {
            if cell.height <= shelf_height {
                cell.height = cell.height.min(shelf_depth);
            }
        }
    }

    /// Stretch the heights so the lowest is 0 and the highest is 1
    pub fn normalize(&mut self) {
        let min = self.cells.iter().map(|c| c.height).fold(f32::MAX, f32::min);
        let max = self.cells.iter().map(|c| c.height).fold(f32::MIN, f32::max);
        for cell in &mut self.cells {
            cell.height = (cell.height - min) / (max - min);
        }
    }

    pub(crate) fn add_height(&mut self, p: Vec2, amount: f32) {
        if let Some(i) = self.index(p) {
            self.cells[i].height += amount;
        }
    }

    pub(crate) fn add_flow(&mut self, p: Vec2, amount: f32) {
        if let Some(i) = self.index(p) {
            self.cells[i].flow += amount;
        }
    }

    pub(crate) fn contains(&self, p: Vec2) -> bool {
        self.index(p).is_some()
    }

    fn index(&self, p: Vec2) -> Option<usize> {
        let w = self.width as f32;
        if p.x < 0.0 || p.y < 0.0 || p.x >= w || p.y >= w {
            return None;
        }
        Some(p.x as usize + p.y as usize * self.width)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec2;

    #[test]
    fn off_the_grid_reads_as_zero() {
        let grid = HeightGrid::from_noise(4, 0.1, 2, 1, 1.0, vec2(0.0, 0.0));
        // x past the edge used to wrap into the next row, or panic past the end
        for p in [(4.0, 0.0), (0.0, 4.0), (-0.5, 1.0), (10.0, 10.0)] {
            let p = vec2(p.0, p.1);
            assert_eq!(grid.flow(p), 0.0);
            assert_eq!(grid.height(p), 0.0);
        }
    }

    #[test]
    fn normalize_spans_zero_to_one() {
        let mut grid = HeightGrid::from_noise(8, 0.2, 3, 5, 10.0, vec2(0.0, 0.0));
        grid.normalize();
        let heights: Vec<f32> = grid.cells.iter().map(|c| c.height).collect();
        assert_eq!(heights.iter().copied().fold(f32::MAX, f32::min), 0.0);
        assert_eq!(heights.iter().copied().fold(f32::MIN, f32::max), 1.0);
    }

    #[test]
    fn shelf_drops_the_band_and_leaves_the_rest() {
        let mut grid = HeightGrid::from_noise(8, 0.2, 3, 5, 1.0, vec2(0.0, 0.0));
        grid.normalize();
        let before: Vec<f32> = grid.cells.iter().map(|c| c.height).collect();
        grid.create_shelf(0.6, 0.3);
        for (old, cell) in before.iter().zip(&grid.cells) {
            let expected = if *old > 0.3 && *old <= 0.6 { 0.3 } else { *old };
            assert_eq!(cell.height, expected);
        }
    }

    #[test]
    fn surface_height_matches_the_grid_points() {
        let grid = HeightGrid::from_noise(6, 0.3, 3, 9, 5.0, vec2(0.0, 0.0));
        for (x, z) in [(1.0, 1.0), (3.0, 2.0), (4.0, 4.0)] {
            assert_eq!(grid.surface_height(vec2(x, z)), grid.height(vec2(x, z)));
        }
    }
}
