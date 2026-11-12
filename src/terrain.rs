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

/// Interpolated height of `height` at `(x, y)`.
pub fn surface_height(height: impl Fn(f32, f32) -> f32, x: f32, z: f32) -> f32 {
    let (corners, fx, fz) = cell(&height, x, z);
    let [h00, h10, h01, h11] = corners;
    // Each cell is split along the diagonal from (0, 1) to (1, 0), same as the heightmap
    if fx + fz <= 1.0 {
        h00 + fx * (h10 - h00) + fz * (h01 - h00)
    } else {
        h11 + (1.0 - fx) * (h01 - h11) + (1.0 - fz) * (h10 - h11)
    }
}

/// The normal of the flat triangle under (`x`, `z`) on `height`.
pub fn surface_normal(height: impl Fn(f32, f32) -> f32, x: f32, z: f32) -> Vec3 {
    let (corners, fx, fz) = cell(&height, x, z);
    let [h00, h10, h01, h11] = corners;
    // How much the height changes per unit of x and z across the triangle
    let (dx, dz) = if fx + fz <= 1.0 {
        (h10 - h00, h01 - h00)
    } else {
        (h11 - h01, h11 - h10)
    };
    nalgebra_glm::vec3(-dx, 1.0, -dz).normalize()
}

/// The heights at the four corners of the grid cell holding (`x`, `z`), as [(0, 0), (1, 0),
/// (0, 1), (1, 1)], and how far across the cell the point is
fn cell(height: &impl Fn(f32, f32) -> f32, x: f32, z: f32) -> ([f32; 4], f32, f32) {
    let (x0, z0) = (x.floor(), z.floor());
    let corners = [
        height(x0, z0),
        height(x0 + 1.0, z0),
        height(x0, z0 + 1.0),
        height(x0 + 1.0, z0 + 1.0),
    ];
    (corners, x - x0, z - z0)
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

    /// Bumpy enough that the mesh and the raw function disagree between grid points
    fn bumpy(x: f32, z: f32) -> f32 {
        (x * 1.3).sin() * 2.0 + (z * 0.9).cos() * 1.5 + x * z * 0.1
    }

    #[test]
    fn surface_height_matches_the_mesh_you_see() {
        let mesh = crate::mesh_data::MeshData::heightmap(4, 4, bumpy);
        let tris: Vec<_> = mesh.triangles().collect();
        for i in 0..40 {
            // Stay off the very edges so the downward ray always lands on a triangle
            let (x, z) = (
                0.05 + (i as f32 * 0.37) % 3.9,
                0.05 + (i as f32 * 0.71) % 3.9,
            );
            let ray = crate::ray::Ray::new(
                nalgebra_glm::vec3(x, 100.0, z),
                nalgebra_glm::vec3(0.0, -1.0, 0.0),
            );
            let t = tris
                .iter()
                .filter_map(|tri| tri.raycast(&ray, 0.0..f32::INFINITY))
                .map(|hit| hit.t)
                .fold(f32::MAX, f32::min);
            let on_mesh = 100.0 - t;
            let ours = surface_height(bumpy, x, z);
            assert!(
                (ours - on_mesh).abs() < 1e-3,
                "at ({x}, {z}): {ours} vs {on_mesh}"
            );
        }
    }

    #[test]
    fn surface_height_hits_the_grid_points() {
        for (x, z) in [(0.0, 0.0), (2.0, 3.0), (-1.0, 5.0)] {
            assert!((surface_height(bumpy, x, z) - bumpy(x, z)).abs() < 1e-5);
        }
    }

    #[test]
    fn surface_normal_is_flat_per_triangle_and_faces_up() {
        // Two points in the same triangle get the same normal, since the triangle is flat
        let a = surface_normal(bumpy, 1.1, 2.1);
        let b = surface_normal(bumpy, 1.3, 2.2);
        assert!((a - b).norm() < 1e-5);
        assert!(a.y > 0.0);
        // On a plain slope it matches the smooth normal exactly
        let slope = |x: f32, z: f32| 0.5 * x - 0.25 * z;
        assert!((surface_normal(slope, 3.7, 1.2) - normal_at(slope, 3.7, 1.2)).norm() < 1e-5);
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
