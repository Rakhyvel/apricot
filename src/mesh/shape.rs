//! A mesh's triangles kept on the CPU, for picking and culling

use nalgebra_glm::{vec3, vec4, Mat4, Vec3};

use super::MeshData;
use crate::geometry::{aabb::AABB, ray::Ray, tri::Tri};

/// A mesh's positions and triangles, and the box around them, in the mesh's own space
#[derive(Debug, Clone)]
pub struct MeshShape {
    positions: Vec<Vec3>,
    indices: Vec<u32>,
    aabb: AABB,
}

impl From<MeshData> for MeshShape {
    fn from(data: MeshData) -> Self {
        let positions: Vec<Vec3> = data
            .positions
            .chunks_exact(3)
            .map(|p| vec3(p[0], p[1], p[2]))
            .collect();
        Self {
            aabb: AABB::from_points(positions.iter().copied()),
            positions,
            indices: data.indices,
        }
    }
}

impl MeshShape {
    /// Every corner, in the mesh's own space
    pub fn positions(&self) -> &[Vec3] {
        &self.positions
    }

    /// Three corners per triangle, as indices into `positions`
    pub fn indices(&self) -> &[u32] {
        &self.indices
    }

    /// The box around the mesh, in its own space
    pub fn aabb(&self) -> AABB {
        self.aabb
    }

    /// The box around the mesh once it's moved, rotated, or scaled by `transform`
    pub fn world_aabb(&self, transform: &Mat4) -> AABB {
        self.aabb.transformed(transform)
    }

    /// Every triangle, in the mesh's own space
    pub fn triangles(&self) -> impl Iterator<Item = Tri> + '_ {
        let corner = |i: u32| self.positions[i as usize];
        self.indices
            .chunks_exact(3)
            .map(move |t| Tri::new(corner(t[0]), corner(t[1]), corner(t[2])))
    }

    /// How far along `ray` it first hits the mesh placed by `transform`, in world units
    pub fn raycast(&self, ray: &Ray, transform: &Mat4) -> Option<f32> {
        let to_local = transform.try_inverse()?;

        // Move the ray into the mesh's space instead of moving every triangle out
        let (o, d) = (ray.origin(), ray.dir());
        let local_ray = Ray::new(
            (to_local * vec4(o.x, o.y, o.z, 1.0)).xyz(),
            (to_local * vec4(d.x, d.y, d.z, 0.0)).xyz(),
        );
        let t = self
            .triangles()
            .filter_map(|tri| tri.raycast(&local_ray, 0.0..f32::INFINITY))
            .map(|hit| hit.t)
            .min_by(f32::total_cmp)?;

        // Back to world space, so differently scaled meshes compare fairly
        let p = local_ray.at(t);
        let hit = (transform * vec4(p.x, p.y, p.z, 1.0)).xyz();
        Some(nalgebra_glm::distance(&o, &hit))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cube() -> MeshShape {
        MeshShape::from(MeshData::cube())
    }

    /// Moved 3 along x and scaled up 2 times, so it spans 2 to 4 on x and -1 to 1 on y and z
    fn placed() -> Mat4 {
        nalgebra_glm::scale(&nalgebra_glm::translation(&vec3(3.0, 0.0, 0.0)), &vec3(2.0, 2.0, 2.0))
    }

    #[test]
    fn keeps_the_corners_and_triangles_of_the_mesh() {
        let data = MeshData::cube();
        let shape = MeshShape::from(data.clone());
        assert_eq!(shape.positions().len(), data.vertex_count());
        assert_eq!(shape.indices(), &data.indices[..]);
        let theirs: Vec<Tri> = data.triangles().collect();
        let ours: Vec<Tri> = shape.triangles().collect();
        assert_eq!(ours.len(), theirs.len());
        for (a, b) in ours.iter().zip(&theirs) {
            assert_eq!((a.v0(), a.v1(), a.v2()), (b.v0(), b.v1(), b.v2()));
        }
    }

    #[test]
    fn boxes_fit_the_mesh_where_it_is_and_where_it_goes() {
        let aabb = cube().aabb();
        assert_eq!((aabb.min, aabb.max), (vec3(-0.5, -0.5, -0.5), vec3(0.5, 0.5, 0.5)));
        let world = cube().world_aabb(&placed());
        assert_eq!((world.min, world.max), (vec3(2.0, -1.0, -1.0), vec3(4.0, 1.0, 1.0)));
    }

    #[test]
    fn raycast_measures_in_the_world_not_the_mesh() {
        let ray = Ray::new(vec3(-10.0, 0.2, 0.1), vec3(1.0, 0.0, 0.0));
        let distance = cube().raycast(&ray, &placed()).unwrap();
        assert!((distance - 12.0).abs() < 1e-4, "{distance}");
    }

    #[test]
    fn raycast_misses_beside_behind_and_when_squashed_flat() {
        let beside = Ray::new(vec3(-10.0, 3.0, 0.0), vec3(1.0, 0.0, 0.0));
        assert_eq!(cube().raycast(&beside, &placed()), None);
        let away = Ray::new(vec3(-10.0, 0.0, 0.0), vec3(-1.0, 0.0, 0.0));
        assert_eq!(cube().raycast(&away, &placed()), None);
        let flat = nalgebra_glm::scaling(&vec3(1.0, 0.0, 1.0));
        let down = Ray::new(vec3(0.0, 5.0, 0.0), vec3(0.0, -1.0, 0.0));
        assert_eq!(cube().raycast(&down, &flat), None);
    }
}
