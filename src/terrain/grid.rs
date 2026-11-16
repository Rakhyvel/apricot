//! A finite square grid of heights, for terrain you shape and erode as a whole

use nalgebra_glm::{Vec2, Vec3};

use crate::terrain::{self, noise};

mod erosion;

pub use erosion::ErosionSettings;

/// A square grid of heights, `width` points on a side, one unit apart. Points are addressed by
/// (x, z), and anything off the edge reads as height 0.
#[derive(Default)]
pub struct HeightGrid {
    cells: Vec<Cell>,
    width: usize,
}

#[derive(Default, Copy, Clone)]
struct Cell {
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
        octaves: u32,
        seed: u64,
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
                let i = grid.point_index(x, z);
                grid.cells[i].height = n * amplitude;
            }
        }
        grid
    }

    /// How many points there are along each side
    pub fn width(&self) -> usize {
        self.width
    }

    /// The height of the grid point at (`x`, `z`). 0 off the edge
    pub fn height(&self, x: usize, z: usize) -> f32 {
        self.point(x, z).map_or(0.0, |i| self.cells[i].height)
    }

    /// Set the height of the grid point at (`x`, `z`). Does nothing off the edge
    pub fn set_height(&mut self, x: usize, z: usize, height: f32) {
        if let Some(i) = self.point(x, z) {
            self.cells[i].height = height;
        }
    }

    /// How much water has run over the point at (`x`, `z`). 0 off the edge
    pub fn flow(&self, x: usize, z: usize) -> f32 {
        self.point(x, z).map_or(0.0, |i| self.cells[i].flow)
    }

    /// The height anywhere on the grid, blending between the points
    pub fn surface_height(&self, p: Vec2) -> f32 {
        terrain::surface_height(|x, z| self.height_at(nalgebra_glm::vec2(x, z)), p.x, p.y)
    }

    /// The Y-up normal of the flat triangle under `p`
    pub fn surface_normal(&self, p: Vec2) -> Vec3 {
        terrain::surface_normal(|x, z| self.height_at(nalgebra_glm::vec2(x, z)), p.x, p.y)
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

    /// Stretch the heights so the lowest is 0 and the highest is 1. A flat grid goes to 0
    pub fn normalize(&mut self) {
        let min = self.cells.iter().map(|c| c.height).fold(f32::MAX, f32::min);
        let max = self.cells.iter().map(|c| c.height).fold(f32::MIN, f32::max);
        let range = max - min;
        for cell in &mut self.cells {
            cell.height = if range > 0.0 {
                (cell.height - min) / range
            } else {
                0.0
            };
        }
    }

    /// The height of the grid point at `p`, rounding down to the nearest point. 0 off the edge
    fn height_at(&self, p: Vec2) -> f32 {
        self.index(p).map_or(0.0, |i| self.cells[i].height)
    }

    /// Where the point at (`x`, `z`) is in `cells`, if it's on the grid
    fn point(&self, x: usize, z: usize) -> Option<usize> {
        (x < self.width && z < self.width).then(|| self.point_index(x, z))
    }

    fn add_height(&mut self, p: Vec2, amount: f32) {
        if let Some(i) = self.index(p) {
            self.cells[i].height += amount;
        }
    }

    fn add_flow(&mut self, p: Vec2, amount: f32) {
        if let Some(i) = self.index(p) {
            self.cells[i].flow += amount;
        }
    }

    fn contains(&self, p: Vec2) -> bool {
        self.index(p).is_some()
    }

    fn index(&self, p: Vec2) -> Option<usize> {
        let w = self.width as f32;
        if p.x < 0.0 || p.y < 0.0 || p.x >= w || p.y >= w {
            return None;
        }
        Some(self.point_index(p.x as usize, p.y as usize))
    }

    /// Where point (`x`, `z`) is in `cells`. Rows of x run one after another along z
    fn point_index(&self, x: usize, z: usize) -> usize {
        x + z * self.width
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec2;

    #[test]
    fn points_are_stored_row_by_row() {
        let mut grid = HeightGrid::new(4);
        for z in 0..4 {
            for x in 0..4 {
                grid.set_height(x, z, (10 * z + x) as f32);
            }
        }
        for (i, cell) in grid.cells.iter().enumerate() {
            assert_eq!(cell.height, (10 * (i / 4) + i % 4) as f32, "cell {i}");
        }

        // Fractions round down to the point below
        grid.add_height(vec2(3.9, 2.2), 0.5);
        grid.add_flow(vec2(3.0, 2.99), 2.0);
        assert_eq!(grid.cells[11].height, 23.5);
        assert_eq!(grid.cells[11].flow, 2.0);
        assert_eq!(grid.cells[grid.point_index(3, 2)].flow, 2.0);
        assert_eq!(grid.flow(3, 2), 2.0);
    }

    #[test]
    fn points_are_read_and_written_by_whole_number_position() {
        let mut grid = HeightGrid::new(4);
        grid.set_height(3, 2, 7.0);
        assert_eq!(grid.height(3, 2), 7.0);
        assert_eq!(grid.height(2, 3), 0.0);
        assert_eq!(grid.flow(3, 2), 0.0);

        // Off the edge, nothing is written and everything reads as zero
        grid.set_height(4, 0, 1.0);
        grid.set_height(0, 4, 1.0);
        assert_eq!(grid.cells.iter().filter(|c| c.height != 0.0).count(), 1);
        assert_eq!((grid.height(4, 0), grid.height(0, 100)), (0.0, 0.0));
        assert_eq!((grid.flow(4, 0), grid.flow(0, 100)), (0.0, 0.0));
    }

    #[test]
    fn the_last_point_on_each_side_is_on_the_grid() {
        let mut grid = HeightGrid::new(4);
        for p in [
            vec2(3.0, 0.0),
            vec2(0.0, 3.0),
            vec2(3.0, 3.0),
            vec2(3.99, 3.99),
        ] {
            assert!(grid.contains(p), "{p:?}");
            let i = grid.index(p).unwrap();
            grid.cells[i].height = 1.0;
            assert_eq!(grid.height_at(p), 1.0, "{p:?}");
        }
        for p in [
            vec2(4.0, 0.0),
            vec2(0.0, 4.0),
            vec2(-0.01, 2.0),
            vec2(2.0, -0.01),
        ] {
            assert!(!grid.contains(p), "{p:?}");
        }
    }

    #[test]
    fn off_the_grid_reads_as_zero() {
        let grid = HeightGrid::from_noise(4, 0.1, 2, 1, 1.0, vec2(0.0, 0.0));
        // x past the edge used to wrap into the next row, or panic past the end
        for p in [(4.0, 0.0), (0.0, 4.0), (-0.5, 1.0), (10.0, 10.0)] {
            let p = vec2(p.0, p.1);
            assert_eq!(grid.height_at(p), 0.0);
        }
    }

    #[test]
    fn from_noise_samples_the_noise_at_the_offset() {
        let offset = vec2(3.0, -5.0);
        let grid = HeightGrid::from_noise(6, 0.2, 3, 7, 2.5, offset);
        for z in 0..6 {
            for x in 0..6 {
                let expected =
                    noise::perlin(x as f32 + offset.x, z as f32 + offset.y, 0.2, 3, 7) * 2.5;
                assert_eq!(grid.height(x, z), expected, "at ({x}, {z})");
            }
        }
    }

    #[test]
    fn normalizing_a_flat_grid_puts_it_all_at_zero() {
        let mut fresh = HeightGrid::new(4);
        let mut raised = HeightGrid::new(4);
        for c in &mut raised.cells {
            c.height = 3.0;
        }
        for grid in [&mut fresh, &mut raised] {
            grid.normalize();
            assert!(grid.cells.iter().all(|c| c.height == 0.0));
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
            assert_eq!(
                grid.surface_height(vec2(x, z)),
                grid.height(x as usize, z as usize)
            );
        }
    }
}
