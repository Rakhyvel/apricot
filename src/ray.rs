//! This module defines a ray

/// A ray data structure
pub struct Ray {
    pub origin: nalgebra_glm::Vec3,
    pub dir: nalgebra_glm::Vec3,
}
