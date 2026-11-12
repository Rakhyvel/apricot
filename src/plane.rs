//! This module defines a normal-distance plane.

use std::ops::Range;

use crate::ray::{Ray, RayHit};

#[derive(Debug, Copy, Clone)]
/// A plane data structure
pub struct Plane {
    normal: nalgebra_glm::Vec3,
    pub dist: f32,
}

impl Plane {
    /// Create a new plane from a normal and distance from the origin along that normal
    pub fn new(normal: nalgebra_glm::Vec3, dist: f32) -> Self {
        Self { normal, dist }
    }

    /// Create a plane from a centerpoint and normal
    pub fn from_center_normal(center: nalgebra_glm::Vec3, normal: nalgebra_glm::Vec3) -> Self {
        let normal_normal = normal.normalize();
        Self {
            normal: normal_normal,
            dist: -normal_normal.dot(&center),
        }
    }

    /// Retrieve the normal of the plane
    pub fn normal(&self) -> nalgebra_glm::Vec3 {
        self.normal
    }

    /// Retrieve the distance of the plane
    pub fn dist(&self) -> f32 {
        self.dist
    }

    /// Where `ray` crosses the plane, if that's inside `t_range`. Rays running along the plane
    /// never hit it
    pub fn raycast(&self, ray: &Ray, t_range: Range<f32>) -> Option<RayHit> {
        let facing = self.normal.dot(&ray.dir());
        if facing.abs() < 1e-8 {
            return None;
        }
        let t = -(self.normal.dot(&ray.origin()) + self.dist) / facing;
        t_range
            .contains(&t)
            .then(|| RayHit::new(ray, t, self.normal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec3;

    fn floor() -> Plane {
        Plane::from_center_normal(vec3(0.0, -2.0, 0.0), vec3(0.0, 1.0, 0.0))
    }

    #[test]
    fn hits_from_above_facing_up() {
        let ray = Ray::new(vec3(1.0, 3.0, 1.0), vec3(0.0, -1.0, 0.0));
        let hit = floor().raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 5.0).abs() < 1e-5);
        assert!((hit.point - vec3(1.0, -2.0, 1.0)).norm() < 1e-5);
        assert!(hit.front_face);
    }

    #[test]
    fn hits_from_below_facing_down() {
        let ray = Ray::new(vec3(0.0, -4.0, 0.0), vec3(0.0, 1.0, 0.0));
        let hit = floor().raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!(!hit.front_face);
        assert!((hit.normal - vec3(0.0, -1.0, 0.0)).norm() < 1e-5);
    }

    #[test]
    fn misses_when_parallel_or_pointing_away() {
        let along = Ray::new(vec3(0.0, 0.0, 0.0), vec3(1.0, 0.0, 0.0));
        assert!(floor().raycast(&along, 0.0..f32::INFINITY).is_none());
        let up = Ray::new(vec3(0.0, 0.0, 0.0), vec3(0.0, 1.0, 0.0));
        assert!(floor().raycast(&up, 0.0..f32::INFINITY).is_none());
    }
}
