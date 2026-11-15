//! Spreading points out over an area

use nalgebra_glm::{vec2, Vec2};
use rand::{Rng, SeedableRng};

use crate::geometry::rectangle::Rectangle;

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
