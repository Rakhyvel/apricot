use crate::ray::Ray;

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

    pub fn raycast(&self, ray: &Ray) -> Option<f32> {
        let m = ray.origin() - self.center;
        let b = m.dot(&ray.dir());
        let c = m.dot(&m) - self.radius * self.radius;

        if c > 0.0 && b > 0.0 {
            return None; // Outside the sphere and pointing away
        }

        let discr = b * b - c;
        if discr < 0.0 {
            return None;
        }

        let sqrt_discr = discr.sqrt();
        let mut t = -b - sqrt_discr;
        if t < 0.0 {
            t = -b + sqrt_discr; // Nearest hit is behind us, try the far side
        }

        Some(t)
    }
}
