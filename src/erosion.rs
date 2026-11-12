//! Wears a height map down with water and gravity. It needs the whole grid at once, so it
//! only works on finite terrain, not endless chunks

use std::cmp::Ordering;

use nalgebra_glm::{Vec2, Vec3};
use rand::{Rng, SeedableRng};

use crate::height_grid::HeightGrid;

/// A raindrop running downhill. Works in Z-up space, with the grid's (x, z) as (x, y)
struct Particle {
    age: usize,
    pos: Vec3,
    vel: Vec3,
    volume: f32,
    rain: f32,
}

impl Particle {
    fn new(pos: Vec3) -> Self {
        Self {
            age: 0,
            pos,
            vel: Vec3::zeros(),
            volume: 1.0,
            rain: 0.0,
        }
    }

    // TODO: always returns false and `flying` never changes, so this doesn't erode anything yet
    fn descend(&mut self, grid: &mut HeightGrid) -> bool {
        loop {
            const EVAPORATION_RATE: f32 = 0.1;
            const MAX_AGE: usize = 100;

            if self.age > MAX_AGE {
                break;
            }

            // The grid's normal is Y-up, so swap it into this Z-up space
            let n = grid.surface_normal(self.pos.xy());
            let grad = nalgebra_glm::vec3(n.x, n.z, n.y);

            // Accelerate particle using classical mechanics
            let old_pos = self.pos;
            self.vel += grad / self.volume;
            self.vel += nalgebra_glm::vec3(0.0, 0.1, 0.0);
            if nalgebra_glm::length(&self.vel) > 0.0 {
                self.vel = (2.0_f32).sqrt() * nalgebra_glm::normalize(&self.vel);
            }
            self.vel.z -= 1.0; // gravity
            self.pos += self.vel;
            let mut flying = false;
            if self.pos.z < grid.height(self.pos.xy()) {
                self.pos.z = grid.height(self.pos.xy());
                flying = false;
            }

            // Update flow, momentum
            if !flying {
                if self.pos.z >= 0.5 {
                    grid.add_flow(old_pos.xy(), self.rain);
                } else {
                    self.rain = 1.0;
                }
            }

            self.rain *= 1.0 - EVAPORATION_RATE;

            self.age += 1;
        }
        false
    }
}

impl HeightGrid {
    /// Drop `total_particles` raindrops at random and let them run downhill, wearing the terrain
    /// down. The same seed always erodes the same way
    pub fn erode(&mut self, total_particles: usize, seed: u64) {
        let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
        for _ in 0..total_particles {
            let mut drop = Particle::new(nalgebra_glm::vec3(
                rng.gen_range(0.0..self.width as f32),
                rng.gen_range(0.0..self.width as f32),
                10.0,
            ));
            if self.height(drop.pos.xy()) < 0.5 {
                continue;
            }
            while drop.descend(self) {}
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
