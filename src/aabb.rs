//! This module defines an Axis Aligned Bounding Box.

use core::f32;

use std::ops::Range;

use super::{
    frustum::Frustum,
    plane::Plane,
    ray::{Ray, RayHit},
    sphere::Sphere,
};

#[derive(Debug, Copy, Clone)]
/// # Axis Aligned Bounding Box.
/// An axis aligned bounding box is defined by two vertices: the `min`, which contains the smallest bounds on each axis,
/// and the `max`, which contains the maximum values for each axis.
///
/// AABBs are very simple yet powerful, and are used for a lot of things.
pub struct AABB {
    pub min: nalgebra_glm::Vec3,
    pub max: nalgebra_glm::Vec3,
}

impl AABB {
    /// A box with nothing in it, inside out so any point grows it to fit
    pub fn empty() -> Self {
        let min = nalgebra_glm::vec3(f32::MAX, f32::MAX, f32::MAX);
        let max = nalgebra_glm::vec3(f32::MIN, f32::MIN, f32::MIN);
        Self { min, max }
    }

    pub fn from_min_max(min: nalgebra_glm::Vec3, max: nalgebra_glm::Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from an iterator of points. This is slow!
    pub fn from_points(points: impl IntoIterator<Item = nalgebra_glm::Vec3>) -> Self {
        let mut retval = AABB::empty();
        retval.expand_to_fit(points);
        retval
    }

    /// Union two AABBs to form a single AABB that contains both
    pub fn union(&self, b: AABB) -> AABB {
        AABB::from_min_max(
            nalgebra_glm::min2(&self.min, &b.min),
            nalgebra_glm::max2(&self.max, &b.max),
        )
    }

    /// This function finds the _surface_ area of an AABB.
    pub fn surface_area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Determines whether or not an AABB intersects with a frustum
    pub fn within_frustum(&self, frustum: &Frustum) -> bool {
        for (i, plane) in frustum.planes().iter().enumerate() {
            let vmax = self.get_furthest_corner(plane);
            let value = plane.normal().dot(&vmax) + plane.dist();
            if value < 0.0 {
                log::trace!("{self:?} is outside frustum plane {i}: {plane:?}");
                return false;
            }
        }
        true
    }

    /// Whether the box and `sphere` overlap
    pub fn within_sphere(&self, sphere: &Sphere) -> bool {
        let closest = nalgebra_glm::clamp_vec(&sphere.center, &self.min, &self.max);
        nalgebra_glm::distance2(&closest, &sphere.center) < sphere.radius * sphere.radius
    }

    /// The first place `ray` hits the box's surface with `t` inside `t_range`. From inside the
    /// box, that's where it comes out
    pub fn raycast(&self, ray: &Ray, t_range: Range<f32>) -> Option<RayHit> {
        let (o, inv) = (ray.origin(), ray.inv_dir());
        let mut t_enter = f32::NEG_INFINITY;
        let mut t_exit = f32::INFINITY;
        let (mut enter_axis, mut exit_axis) = (0, 0);
        for axis in 0..3 {
            let t1 = (self.min[axis] - o[axis]) * inv[axis];
            let t2 = (self.max[axis] - o[axis]) * inv[axis];
            let (near, far) = (t1.min(t2), t1.max(t2));
            if near > t_enter {
                t_enter = near;
                enter_axis = axis;
            }
            if far < t_exit {
                t_exit = far;
                exit_axis = axis;
            }
        }
        if t_exit < t_enter {
            return None;
        }

        // The face's normal points out along its axis, away from the box's middle
        let face_normal = |axis: usize, t: f32| {
            let mut n = nalgebra_glm::Vec3::zeros();
            n[axis] = (ray.at(t)[axis] - self.center()[axis]).signum();
            n
        };
        [(t_enter, enter_axis), (t_exit, exit_axis)]
            .into_iter()
            .find(|(t, _)| t_range.contains(t))
            .map(|(t, axis)| RayHit::new(ray, t, face_normal(axis, t)))
    }

    /// Calculates the bounding sphere for an AABB
    pub fn bounding_sphere(&self) -> Sphere {
        let center = self.center();
        Sphere::new(center, nalgebra_glm::distance(&center, &self.min))
    }

    /// Finds the centerpoint of an AABB
    pub fn center(&self) -> nalgebra_glm::Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Moves an AABB by a vector
    pub fn translate(&self, center: nalgebra_glm::Vec3) -> Self {
        Self {
            min: self.min + center,
            max: self.max + center,
        }
    }

    /// Scales an AABB by a certain amount
    pub fn scale(&self, factor: nalgebra_glm::Vec3) -> Self {
        Self {
            min: self.min.component_mul(&factor),
            max: self.max.component_mul(&factor),
        }
    }

    /// Expands an AABB to fit all points in some iterator of points. This is __slow__!
    pub fn expand_to_fit(&mut self, points: impl IntoIterator<Item = nalgebra_glm::Vec3>) {
        for corner in points.into_iter() {
            self.min = nalgebra_glm::min2(&self.min, &corner);
            self.max = nalgebra_glm::max2(&self.max, &corner);
        }
    }

    /// Finds the midpoint of the positive z plane. Useful for shadow mapping.
    pub fn pos_z_plane_midpoint(&self) -> nalgebra_glm::Vec4 {
        let bottom_left = nalgebra_glm::vec4(self.min.x, self.min.y, self.max.z, 1.0);
        let top_right = nalgebra_glm::vec4(self.max.x, self.max.y, self.max.z, 1.0);
        0.5 * (bottom_left + top_right)
    }

    /// The smallest AABB that fits this one after it's been moved, scaled, or rotated by `matrix`
    pub fn transformed(&self, matrix: &nalgebra_glm::Mat4) -> Self {
        AABB::from_points(
            self.corners()
                .map(|c| (matrix * nalgebra_glm::vec4(c.x, c.y, c.z, 1.0)).xyz()),
        )
    }

    /// Determines whether two AABBs intersect
    pub fn intersects(&self, other: &AABB) -> bool {
        // Check for separation in the x-axis
        if self.max.x < other.min.x || self.min.x > other.max.x {
            return false;
        }
        // Check for separation in the y-axis
        if self.max.y < other.min.y || self.min.y > other.max.y {
            return false;
        }
        // Check for separation in the z-axis
        if self.max.z < other.min.z || self.min.z > other.max.z {
            return false;
        }

        // No separation found, the AABBs intersect
        true
    }

    /// Determines whether this AABB _fully_ contains the other AABB
    pub fn contains(&self, other: &AABB) -> bool {
        let mut result = true;
        result = result && self.min.x <= other.min.x;
        result = result && self.min.y <= other.min.y;
        result = result && self.min.z <= other.min.z;
        result = result && other.max.x <= self.max.x;
        result = result && other.max.y <= self.max.y;
        result = result && other.max.z <= self.max.z;
        result
    }

    /// Determines whether this AABB contains the point
    pub fn contains_point(&self, point: nalgebra_glm::Vec3) -> bool {
        self.min.x <= point.x
            && self.min.y <= point.y
            && self.min.z <= point.z
            && self.max.x >= point.x
            && self.max.y >= point.y
            && self.max.z >= point.z
    }

    /// Produces the corners of an AABB. This is _SLOW_!
    pub fn corners(&self) -> [nalgebra_glm::Vec3; 8] {
        [
            nalgebra_glm::Vec3::new(self.min.x, self.min.y, self.min.z),
            nalgebra_glm::Vec3::new(self.max.x, self.min.y, self.min.z),
            nalgebra_glm::Vec3::new(self.min.x, self.max.y, self.min.z),
            nalgebra_glm::Vec3::new(self.max.x, self.max.y, self.min.z),
            nalgebra_glm::Vec3::new(self.min.x, self.min.y, self.max.z),
            nalgebra_glm::Vec3::new(self.max.x, self.min.y, self.max.z),
            nalgebra_glm::Vec3::new(self.min.x, self.max.y, self.max.z),
            nalgebra_glm::Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }

    /// Given a normal-plane, returns the further point from the plane along it's normal
    fn get_furthest_corner(&self, plane: &Plane) -> nalgebra_glm::Vec3 {
        nalgebra_glm::vec3(
            if plane.normal().x > 0.0 {
                self.max.x
            } else {
                self.min.x
            },
            // Y axis
            if plane.normal().y > 0.0 {
                self.max.y
            } else {
                self.min.y
            },
            // Z axis
            if plane.normal().z > 0.0 {
                self.max.z
            } else {
                self.min.z
            },
        )
    }
}

impl Default for AABB {
    fn default() -> Self {
        Self::empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frustum::test_frustum;
    use nalgebra_glm::vec3;

    /// At depth `d` the view spans -d to d on both x and y
    fn camera_frustum() -> Frustum {
        test_frustum(1.0)
    }

    #[test]
    fn surface_area_adds_up_all_six_faces() {
        let aabb = AABB::from_min_max(vec3(1.0, -2.0, 3.0), vec3(3.0, 1.0, 7.0));
        // 2 by 3 by 4, so two faces each of 6, 12, and 8
        assert_eq!(aabb.surface_area(), 52.0);
    }

    #[test]
    fn boxes_in_view_or_touching_it_are_within_the_frustum() {
        let frustum = camera_frustum();
        let in_view = AABB::from_min_max(vec3(-0.5, -0.5, -10.5), vec3(0.5, 0.5, -9.5));
        let across_left_edge = AABB::from_min_max(vec3(-10.5, -0.5, -10.5), vec3(-9.5, 0.5, -9.5));
        let across_far_plane = AABB::from_min_max(vec3(-1.0, -1.0, -101.0), vec3(1.0, 1.0, -99.0));
        for aabb in [in_view, across_left_edge, across_far_plane] {
            assert!(aabb.within_frustum(&frustum), "{aabb:?}");
        }
    }

    #[test]
    fn boxes_outside_the_view_are_not_within_the_frustum() {
        let frustum = camera_frustum();
        // Its bounding sphere pokes past the near plane into the view, but the box doesn't
        let just_behind = AABB::from_min_max(vec3(-0.5, -0.5, 0.05), vec3(0.5, 0.5, 1.05));
        let far_behind = AABB::from_min_max(vec3(-0.5, -0.5, 5.0), vec3(0.5, 0.5, 6.0));
        let off_to_the_left = AABB::from_min_max(vec3(-12.0, -0.5, -10.5), vec3(-11.0, 0.5, -9.5));
        let past_far_plane = AABB::from_min_max(vec3(-1.0, -1.0, -110.0), vec3(1.0, 1.0, -105.0));
        for aabb in [just_behind, far_behind, off_to_the_left, past_far_plane] {
            assert!(!aabb.within_frustum(&frustum), "{aabb:?}");
        }
    }

    fn unit_box() -> AABB {
        AABB::from_min_max(
            nalgebra_glm::vec3(-1.0, -1.0, -1.0),
            nalgebra_glm::vec3(1.0, 1.0, 1.0),
        )
    }

    #[test]
    fn raycast_hits_the_face_it_enters() {
        let ray = Ray::new(
            nalgebra_glm::vec3(5.0, 0.2, 0.3),
            nalgebra_glm::vec3(-1.0, 0.0, 0.0),
        );
        let hit = unit_box().raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 4.0).abs() < 1e-5);
        assert!(hit.front_face);
        assert!((hit.normal - nalgebra_glm::vec3(1.0, 0.0, 0.0)).norm() < 1e-5);
    }

    #[test]
    fn raycast_doesnt_care_how_long_the_direction_is() {
        let aabb = AABB::from_min_max(
            nalgebra_glm::vec3(3.0, 1.0, -1.0),
            nalgebra_glm::vec3(5.0, 3.0, 1.0),
        );
        // Enters the x = 3 face at (3, 1.5, 0)
        let expected = nalgebra_glm::vec3(3.0, 1.5, 0.0);
        for length in [0.5, 1.0, 2.0, 10.0] {
            let dir = nalgebra_glm::vec3(2.0, 1.0, 0.0).normalize() * length;
            let ray = Ray::new(nalgebra_glm::Vec3::zeros(), dir);
            let hit = aabb.raycast(&ray, 0.0..f32::INFINITY).unwrap();
            assert!(
                (hit.point - expected).norm() < 1e-5,
                "length {length} hit {:?}",
                hit.point
            );
            assert!((hit.t - expected.norm()).abs() < 1e-5, "length {length} t {}", hit.t);
        }
    }

    #[test]
    fn raycast_from_inside_hits_where_it_comes_out() {
        let ray = Ray::new(
            nalgebra_glm::vec3(0.0, 0.0, 0.0),
            nalgebra_glm::vec3(0.0, 1.0, 0.0),
        );
        let hit = unit_box().raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 1.0).abs() < 1e-5);
        assert!(!hit.front_face);
        assert!((hit.normal - nalgebra_glm::vec3(0.0, -1.0, 0.0)).norm() < 1e-5);
    }

    #[test]
    fn raycast_misses_beside_and_behind() {
        let beside = Ray::new(
            nalgebra_glm::vec3(5.0, 3.0, 0.0),
            nalgebra_glm::vec3(-1.0, 0.0, 0.0),
        );
        assert!(unit_box().raycast(&beside, 0.0..f32::INFINITY).is_none());
        let away = Ray::new(
            nalgebra_glm::vec3(5.0, 0.0, 0.0),
            nalgebra_glm::vec3(1.0, 0.0, 0.0),
        );
        assert!(unit_box().raycast(&away, 0.0..f32::INFINITY).is_none());
    }

    #[test]
    fn within_sphere_checks_the_closest_point() {
        let b = unit_box();
        let sphere = |x: f32, r: f32| Sphere::new(nalgebra_glm::vec3(x, 0.0, 0.0), r);
        assert!(b.within_sphere(&sphere(0.0, 0.1))); // inside
        assert!(b.within_sphere(&sphere(1.5, 0.6))); // poking in from the side
        assert!(!b.within_sphere(&sphere(1.5, 0.4))); // just short
                                                      // Off past a corner, the corner is what counts, not the face
        let corner = Sphere::new(nalgebra_glm::vec3(1.5, 1.5, 0.0), 0.6);
        assert!(!b.within_sphere(&corner));
    }

    #[test]
    fn transformed_fits_a_rotated_box() {
        let unit = AABB::from_min_max(
            nalgebra_glm::vec3(-0.5, -0.5, -0.5),
            nalgebra_glm::vec3(0.5, 0.5, 0.5),
        );
        let rotated =
            nalgebra_glm::rotate_y(&nalgebra_glm::identity(), std::f32::consts::FRAC_PI_4);
        let fitted = unit.transformed(&rotated);

        // Turned 45 degrees, the corners poke out to half the diagonal
        let half_diagonal = 0.5 * 2.0f32.sqrt();
        assert!((fitted.max.x - half_diagonal).abs() < 1e-5);
        assert!((fitted.min.z + half_diagonal).abs() < 1e-5);
        assert!((fitted.max.y - 0.5).abs() < 1e-5);
    }
}
