//! Helpers for working with terrain described by a height function

use nalgebra_glm::{vec3, Vec3};

use crate::mesh::MeshData;

pub mod chunks;
pub mod grid;
pub mod noise;
mod scatter;

pub use scatter::scatter;

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
    // Each cell is split along the diagonal from (0, 1) to (1, 0), same as `mesh`
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

/// Flat-shaded terrain on the XZ plane, `cells_x` by `cells_z` cells of size 1, with each
/// corner raised to `height(x, z)`. The texture stretches over the whole grid once
pub fn mesh(cells_x: u32, cells_z: u32, height: impl Fn(f32, f32) -> f32) -> MeshData {
    let corner = |x: u32, z: u32| {
        let (x, z) = (x as f32, z as f32);
        vec3(x, height(x, z), z)
    };
    let mut data = MeshData::default();
    for z in 0..cells_z {
        for x in 0..cells_x {
            let tris = [
                [(x, z), (x, z + 1), (x + 1, z)],
                [(x + 1, z), (x, z + 1), (x + 1, z + 1)],
            ];
            for tri in tris {
                let p = tri.map(|(x, z)| corner(x, z));
                let normal = (p[1] - p[0]).cross(&(p[2] - p[0])).normalize();
                for (i, (cx, cz)) in tri.into_iter().enumerate() {
                    data.indices.push(data.vertex_count() as u32);
                    data.positions.extend_from_slice(&[p[i].x, p[i].y, p[i].z]);
                    data.normals
                        .extend_from_slice(&[normal.x, normal.y, normal.z]);
                    data.uvs.extend_from_slice(&[
                        cx as f32 / cells_x as f32,
                        cz as f32 / cells_z as f32,
                        0.0,
                    ]);
                }
            }
        }
    }
    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh::tests::{check, uv};

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
        let mesh = mesh(4, 4, bumpy);
        let tris: Vec<_> = mesh.triangles().collect();
        for i in 0..40 {
            // Stay off the very edges so the downward ray always lands on a triangle
            let (x, z) = (
                0.05 + (i as f32 * 0.37) % 3.9,
                0.05 + (i as f32 * 0.71) % 3.9,
            );
            let ray = crate::geometry::ray::Ray::new(
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
    fn surface_normal_matches_the_mesh_you_see() {
        let mesh = mesh(4, 4, bumpy);
        let tris: Vec<_> = mesh.triangles().collect();
        for x0 in 0..4 {
            for z0 in 0..4 {
                // One point in each of the cell's two triangles
                for (fx, fz) in [(0.25, 0.3), (0.7, 0.8)] {
                    let (x, z) = (x0 as f32 + fx, z0 as f32 + fz);
                    let ray = crate::geometry::ray::Ray::new(
                        nalgebra_glm::vec3(x, 100.0, z),
                        nalgebra_glm::vec3(0.0, -1.0, 0.0),
                    );
                    let hit = tris
                        .iter()
                        .filter_map(|tri| tri.raycast(&ray, 0.0..f32::INFINITY))
                        .min_by(|a, b| a.t.total_cmp(&b.t))
                        .unwrap();
                    let ours = surface_normal(bumpy, x, z);
                    assert!(
                        (ours - hit.normal).norm() < 1e-4,
                        "at ({x}, {z}) {ours:?} vs {:?}",
                        hit.normal
                    );
                }
            }
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
    fn mesh_faces_are_flat_and_the_texture_spans_the_grid() {
        let terrain = mesh(4, 3, |x, z| (x * 0.7).sin() + z * 0.2);
        for t in terrain.triangles().enumerate() {
            let (i, tri) = t;
            let n = terrain.normal(3 * i as u32);
            for edge in [tri.v1() - tri.v0(), tri.v2() - tri.v0()] {
                assert!(n.dot(&edge).abs() < 1e-5, "triangle {i}");
            }
        }
        for i in 0..terrain.vertex_count() as u32 {
            let p = terrain.position(i);
            assert_eq!(uv(&terrain, i), (p.x / 4.0, p.z / 3.0));
        }
    }

    #[test]
    fn mesh_is_valid() {
        let terrain = mesh(4, 3, |x, z| (x * 0.7).sin() + z * 0.2);
        check(&terrain);
        assert_eq!(terrain.indices.len(), 4 * 3 * 2 * 3);
        assert!(terrain.position(0).y.abs() < 1e-6); // corner (0, 0) sits at height(0, 0)
    }
}
