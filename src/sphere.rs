use std::ops::Range;

use crate::ray::{Ray, RayHit};

use super::frustum::Frustum;

#[derive(Debug)]
pub struct Sphere {
    pub center: nalgebra_glm::Vec3,
    pub radius: f32,
}

impl Sphere {
    pub fn new(center: nalgebra_glm::Vec3, radius: f32) -> Self {
        Self { center, radius }
    }

    pub fn within_frustum(&self, frustum: &Frustum) -> bool {
        for plane in frustum.planes() {
            if plane.normal().dot(&self.center) + plane.dist() + self.radius < 0.0 {
                return false;
            }
        }
        true
    }

    /// The first place `ray` hits the sphere with `t` inside `t_range`
    pub fn raycast(&self, ray: &Ray, t_range: Range<f32>) -> Option<RayHit> {
        let m = ray.origin() - self.center;
        let b = m.dot(&ray.dir());
        let c = m.dot(&m) - self.radius * self.radius;

        let discr = b * b - c;
        if discr < 0.0 {
            return None;
        }

        // Near side first, then the far side for rays starting inside
        let sqrt_discr = discr.sqrt();
        [-b - sqrt_discr, -b + sqrt_discr]
            .into_iter()
            .find(|t| t_range.contains(t))
            .map(|t| RayHit::new(ray, t, (ray.at(t) - self.center) / self.radius))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec3;

    #[test]
    fn spheres_in_view_or_touching_it_are_within_the_frustum() {
        // At depth d the view spans -d to d on both x and y
        let frustum = crate::frustum::test_frustum(1.0);
        for sphere in [
            Sphere::new(vec3(1.0, -2.0, -10.0), 2.5),
            Sphere::new(vec3(-11.5, 0.0, -10.0), 2.5), // across the left edge
            Sphere::new(vec3(0.0, 0.0, 1.0), 2.5),     // around the camera
            Sphere::new(vec3(0.0, 0.0, -101.0), 2.5),  // across the far plane
        ] {
            assert!(sphere.within_frustum(&frustum), "{sphere:?}");
        }
    }

    #[test]
    fn spheres_outside_the_view_are_not_within_the_frustum() {
        let frustum = crate::frustum::test_frustum(1.0);
        for sphere in [
            Sphere::new(vec3(-16.0, 0.0, -10.0), 2.5), // off to the left
            Sphere::new(vec3(0.0, 14.0, -10.0), 2.5),  // above
            Sphere::new(vec3(0.0, 0.0, 3.0), 2.5),     // behind
            Sphere::new(vec3(0.0, 0.0, -103.0), 2.5),  // past the far plane
        ] {
            assert!(!sphere.within_frustum(&frustum), "{sphere:?}");
        }
    }

    #[test]
    fn raycast_works_for_spheres_bigger_than_one() {
        let sphere = Sphere::new(vec3(1.0, 2.0, -5.0), 2.5);
        let ray = Ray::new(vec3(1.0, 2.0, 0.0), vec3(0.0, 0.0, -1.0));
        let hit = sphere.raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 2.5).abs() < 1e-5);
        assert!((hit.normal - vec3(0.0, 0.0, 1.0)).norm() < 1e-5);
        // Grazes the side, touching at one point
        let tangent = Ray::new(vec3(3.5, 2.0, 0.0), vec3(0.0, 0.0, -1.0));
        let hit = sphere.raycast(&tangent, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 5.0).abs() < 1e-5);
        assert!((hit.point - vec3(3.5, 2.0, -5.0)).norm() < 1e-5);
    }

    fn unit() -> Sphere {
        Sphere::new(vec3(0.0, 0.0, -5.0), 1.0)
    }

    #[test]
    fn hits_the_near_side_from_outside() {
        let ray = Ray::new(vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0));
        let hit = unit().raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 4.0).abs() < 1e-5);
        assert!(hit.front_face);
        assert!((hit.normal - vec3(0.0, 0.0, 1.0)).norm() < 1e-5);
    }

    #[test]
    fn hits_the_far_side_from_inside_with_the_normal_facing_in() {
        let ray = Ray::new(vec3(0.0, 0.0, -5.0), vec3(1.0, 0.0, 0.0));
        let hit = unit().raycast(&ray, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 1.0).abs() < 1e-5);
        assert!(!hit.front_face);
        assert!((hit.normal - vec3(-1.0, 0.0, 0.0)).norm() < 1e-5);
    }

    #[test]
    fn misses_outside_the_range_and_off_to_the_side() {
        let ray = Ray::new(vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0));
        assert!(unit().raycast(&ray, 0.0..3.0).is_none());
        let beside = Ray::new(vec3(2.0, 0.0, 0.0), vec3(0.0, 0.0, -1.0));
        assert!(unit().raycast(&beside, 0.0..f32::INFINITY).is_none());
        let away = Ray::new(vec3(0.0, 0.0, 0.0), vec3(0.0, 0.0, 1.0));
        assert!(unit().raycast(&away, 0.0..f32::INFINITY).is_none());
    }
}
