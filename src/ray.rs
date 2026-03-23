//! This module defines a ray

/// A ray data structure
pub struct Ray {
    pub origin: nalgebra_glm::Vec3,
    pub dir: nalgebra_glm::Vec3,
}

impl Ray {
    pub fn point_at_distance(&self, distance: f32) -> nalgebra_glm::Vec3 {
        self.origin + self.dir.normalize() * distance
    }
}
