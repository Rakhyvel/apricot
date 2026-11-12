//! Wears a height map down with water and gravity. It needs the whole grid at once, so it
//! only works on finite terrain, not endless chunks

use std::cmp::Ordering;

use nalgebra_glm::{Vec2, Vec3};
use rand::{Rng, SeedableRng};

use crate::{height_grid::HeightGrid, terrain};

/// How raindrops wear down the terrain while eroding
#[derive(Debug, Clone, Copy)]
pub struct ErosionSettings {
    /// How far a drop moves each step
    pub time_step: f32,
    /// How much a drop slows down each step, from 0 to 1
    pub friction: f32,
    /// How quickly a drop picks up or drops dirt, from 0 to 1
    pub deposition: f32,
    /// How much of a drop dries up each step, from 0 to 1
    pub evaporation: f32,
    /// How tall the terrain is when working out slopes. Bigger makes drops run faster
    pub height_scale: f32,
    /// The most steps one drop can take
    pub max_steps: usize,
    /// The farthest a drop can move in one step, in cells
    pub max_step: f32,
    /// How far out a drop digs around itself, in cells. Under 1 digs only the cell it's in
    pub brush_radius: f32,
}

impl Default for ErosionSettings {
    fn default() -> Self {
        Self {
            time_step: 1.2,
            friction: 0.05,
            deposition: 0.1,
            evaporation: 0.001,
            height_scale: 40.0,
            max_steps: 500,
            max_step: 1.0,
            brush_radius: 2.0,
        }
    }
}

impl HeightGrid {
    /// Drop `drops` raindrops at random spots and let them run downhill, carving out dirt where
    /// they speed up and leaving it where they slow down. Every point a drop crosses gains flow.
    /// The same seed always erodes the same way. Expects heights from 0 to 1
    pub fn erode(&mut self, drops: usize, seed: u64) {
        self.erode_with(drops, seed, &ErosionSettings::default());
    }

    /// Erode with custom `settings`
    pub fn erode_with(&mut self, drops: usize, seed: u64, settings: &ErosionSettings) {
        const MIN_VOLUME: f32 = 0.01;
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        let w = self.width as f32;
        let brush = brush(settings.brush_radius);

        for _ in 0..drops {
            let mut pos =
                nalgebra_glm::vec2(rng.gen_range(0.0..w - 1.0), rng.gen_range(0.0..w - 1.0));
            let mut vel = Vec2::zeros();
            let mut volume = 1.0;
            let mut sediment = 0.0;

            for _ in 0..settings.max_steps {
                if volume < MIN_VOLUME {
                    break;
                }
                let old = pos;

                // Roll downhill: the normal leans the way the ground falls away
                let scale = settings.height_scale;
                let n = terrain::surface_normal(
                    |x, z| self.height(nalgebra_glm::vec2(x, z)) * scale,
                    pos.x,
                    pos.y,
                );
                vel += settings.time_step * nalgebra_glm::vec2(n.x, n.z) / volume;
                vel *= 1.0 - settings.time_step * settings.friction;

                // Never skip cells, however fast it's going
                let mut step = settings.time_step * vel;
                if step.norm() > settings.max_step {
                    step = step.normalize() * settings.max_step;
                }
                pos += step;

                if !self.contains(pos) || pos.x >= w - 1.0 || pos.y >= w - 1.0 {
                    break; // ran off the edge, taking its dirt with it
                }

                // A fast drop going downhill can carry more dirt than a slow one on the flat
                let drop = self.surface_height(old) - self.surface_height(pos);
                let capacity = (volume * vel.norm() * drop).max(0.0);
                let change = settings.time_step * settings.deposition * (capacity - sediment);
                sediment += change;

                let amount = volume * change;
                if amount > 0.0 {
                    self.dig(old, amount, &brush);
                } else {
                    self.deposit(old, -amount);
                }
                self.add_flow(nalgebra_glm::floor(&old), volume);

                volume *= 1.0 - settings.time_step * settings.evaporation;
            }
        }
    }

    /// Take `amount` of dirt from around `pos`, spread over `brush`
    fn dig(&mut self, pos: Vec2, amount: f32, brush: &[(Vec2, f32)]) {
        let cell = nalgebra_glm::floor(&pos);
        for (offset, weight) in brush {
            self.add_height(cell + offset, -amount * weight);
        }
    }

    /// Leave `amount` of dirt at `pos`, split between the 4 nearest points by how close they are
    fn deposit(&mut self, pos: Vec2, amount: f32) {
        let cell = nalgebra_glm::floor(&pos);
        let (fx, fz) = (pos.x - cell.x, pos.y - cell.y);
        let corners = [
            (nalgebra_glm::vec2(0.0, 0.0), (1.0 - fx) * (1.0 - fz)),
            (nalgebra_glm::vec2(1.0, 0.0), fx * (1.0 - fz)),
            (nalgebra_glm::vec2(0.0, 1.0), (1.0 - fx) * fz),
            (nalgebra_glm::vec2(1.0, 1.0), fx * fz),
        ];
        for (offset, weight) in corners {
            self.add_height(cell + offset, amount * weight);
        }
    }

    /// Let steep drops around `pos` slump into their neighbors, like loose dirt settling
    pub fn cascade(&mut self, pos: Vec2) {
        const MAX_DIFF: f32 = 0.9;
        const SETTLING: f32 = 0.8;

        let neighbors = [
            nalgebra_glm::vec2(-1.0, -1.0),
            nalgebra_glm::vec2(-1.0, 0.0),
            nalgebra_glm::vec2(-1.0, 1.0),
            nalgebra_glm::vec2(0.0, -1.0),
            nalgebra_glm::vec2(0.0, 1.0),
            nalgebra_glm::vec2(1.0, -1.0),
            nalgebra_glm::vec2(1.0, 0.0),
            nalgebra_glm::vec2(1.0, 1.0),
        ];

        let mut in_bound_neighbors: Vec<Vec3> = neighbors
            .iter()
            .map(|n| n + pos)
            .filter(|npos| self.contains(*npos))
            .map(|npos| nalgebra_glm::vec3(npos.x, npos.y, self.height(npos)))
            .collect();

        in_bound_neighbors.sort_by(|a, b| a.z.partial_cmp(&b.z).unwrap_or(Ordering::Greater));

        for npos in in_bound_neighbors {
            // Full height-different between positions
            let diff = self.height(pos) - npos.z;
            if diff == 0.0 {
                continue;
            }

            // The amount of excess difference
            let excess = if npos.z > 0.1 {
                diff.abs() - MAX_DIFF
            } else {
                diff.abs()
            };
            if excess <= 0.0 {
                continue;
            }

            // Actual amount transferred
            let transfer = SETTLING * excess / 2.0;

            if diff > 0.0 {
                self.add_height(pos, -transfer);
                self.add_height(npos.xy(), transfer);
            } else {
                self.add_height(pos, transfer);
                self.add_height(npos.xy(), -transfer);
            }
        }
    }
}

/// Cell offsets within `radius` and how much of a dig each one takes, adding up to 1. Closer
/// cells take more
fn brush(radius: f32) -> Vec<(Vec2, f32)> {
    if radius < 1.0 {
        return vec![(Vec2::zeros(), 1.0)];
    }
    let reach = radius.ceil() as i32;
    let mut cells: Vec<(Vec2, f32)> = (-reach..=reach)
        .flat_map(|dx| (-reach..=reach).map(move |dz| nalgebra_glm::vec2(dx as f32, dz as f32)))
        .map(|offset| (offset, radius - offset.norm()))
        .filter(|(_, weight)| *weight > 0.0)
        .collect();
    let total: f32 = cells.iter().map(|(_, w)| w).sum();
    for (_, weight) in &mut cells {
        *weight /= total;
    }
    cells
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A grid sloping down from a ridge along x = 0
    fn slope(width: usize) -> HeightGrid {
        let mut grid = HeightGrid::new(width);
        for z in 0..width {
            for x in 0..width {
                let h = 1.0 - x as f32 / width as f32;
                grid.set_height(nalgebra_glm::vec2(x as f32, z as f32), h);
            }
        }
        grid
    }

    fn heights(grid: &HeightGrid) -> Vec<f32> {
        grid.cells.iter().map(|c| c.height).collect()
    }

    #[test]
    fn flat_ground_doesnt_erode() {
        let mut grid = HeightGrid::new(16);
        grid.erode(200, 1);
        assert!(heights(&grid).iter().all(|&h| h == 0.0));
    }

    #[test]
    fn slopes_get_carved_and_leave_flow() {
        let mut grid = slope(32);
        let before = heights(&grid);
        grid.erode(500, 3);
        let after = heights(&grid);
        assert!(after.iter().all(|h| h.is_finite()));
        let moved: f32 = before.iter().zip(&after).map(|(a, b)| (a - b).abs()).sum();
        assert!(moved > 0.01, "barely anything moved: {moved}");
        assert!(grid.cells.iter().any(|c| c.flow > 0.0));
    }

    #[test]
    fn a_brush_digs_shallower_dents() {
        let deepest_dent = |radius: f32| {
            let mut grid = slope(32);
            let before = heights(&grid);
            let settings = ErosionSettings {
                brush_radius: radius,
                ..Default::default()
            };
            grid.erode_with(300, 5, &settings);
            before
                .iter()
                .zip(heights(&grid))
                .map(|(a, b)| a - b)
                .fold(0.0, f32::max)
        };
        assert!(deepest_dent(2.0) < deepest_dent(0.0));
    }

    #[test]
    fn brush_weights_add_up_to_one() {
        for radius in [0.0, 1.0, 2.5, 4.0] {
            let total: f32 = brush(radius).iter().map(|(_, w)| w).sum();
            assert!((total - 1.0).abs() < 1e-5, "radius {radius}: {total}");
        }
    }

    #[test]
    fn same_seed_erodes_the_same() {
        let (mut a, mut b) = (slope(24), slope(24));
        a.erode(300, 9);
        b.erode(300, 9);
        assert_eq!(heights(&a), heights(&b));
    }
}
