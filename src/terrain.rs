//! Helpers for working with terrain described by a height function

use nalgebra_glm::{vec2, Vec2, Vec3};
use rand::{Rng, SeedableRng};

use crate::rectangle::Rectangle;

/// The surface normal of `height` at (`x`, `z`), found using finite differencing
pub fn normal_at(height: impl Fn(f32, f32) -> f32, x: f32, z: f32) -> Vec3 {
    const STEP: f32 = 0.1;
    nalgebra_glm::vec3(
        height(x - STEP, z) - height(x + STEP, z),
        2.0 * STEP,
        height(x, z - STEP) - height(x, z + STEP),
    )
    .normalize()
}

/// Up to `attempts` random points in `area`, at least `min_spacing` apart, keeping only the ones
/// `keep` likes.
pub fn scatter(
    area: Rectangle,
    attempts: u32,
    min_spacing: f32,
    seed: u64,
    keep: impl Fn(Vec2) -> bool,
) -> Vec<Vec2> {
    let mut rng = rand::rngs::StdRng::seed_from_u64(seed);
    let mut points: Vec<Vec2> = Vec::new();
    for _ in 0..attempts {
        let p = area.pos
            + vec2(
                rng.gen_range(0.0..area.size.x),
                rng.gen_range(0.0..area.size.y),
            );
        let crowded = points
            .iter()
            .any(|other| nalgebra_glm::distance(&p, other) < min_spacing);
        if !crowded && keep(p) {
            points.push(p);
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_points_up_on_flat_ground() {
        let n = normal_at(|_, _| 3.0, 1.0, 2.0);
        assert!((n - nalgebra_glm::vec3(0.0, 1.0, 0.0)).norm() < 1e-5);
    }

    #[test]
    fn normal_leans_away_from_uphill() {
        // Ground rising one unit per unit of x is a 45 degree slope, so the normal leans back toward -x
        let n = normal_at(|x, _| x, 0.0, 0.0);
        let expected = nalgebra_glm::vec3(-1.0, 1.0, 0.0).normalize();
        assert!((n - expected).norm() < 1e-5);
    }

    #[test]
    fn scatter_stays_inside_spaced_out_and_repeatable() {
        let area = Rectangle::new(-10.0, 5.0, 20.0, 20.0);
        let keep = |p: Vec2| p.x > 0.0; // only the right half
        let points = scatter(area, 500, 2.0, 7, keep);
        assert!(!points.is_empty());
        for (i, p) in points.iter().enumerate() {
            assert!(area.contains_point(p) && keep(*p));
            for q in &points[i + 1..] {
                assert!(nalgebra_glm::distance(p, q) >= 2.0);
            }
        }
        assert_eq!(points, scatter(area, 500, 2.0, 7, keep));
    }
}
