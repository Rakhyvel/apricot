//! This module defines a normal-distance plane.

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
}
