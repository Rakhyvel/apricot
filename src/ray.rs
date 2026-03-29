//! This module defines a ray

/// A ray data structure
pub struct Ray {
    origin: nalgebra_glm::Vec3,
    dir: nalgebra_glm::Vec3,
    // pre-computed
    inv_dir: nalgebra_glm::Vec3,
}

impl Ray {
    pub fn new(origin: nalgebra_glm::Vec3, dir: nalgebra_glm::Vec3) -> Self {
        Self {
            origin,
            dir,
            inv_dir: nalgebra_glm::vec3(1.0 / dir.x, 1.0 / dir.y, 1.0 / dir.z),
        }
    }

    pub fn origin(&self) -> nalgebra_glm::Vec3 {
        self.origin
    }

    pub fn dir(&self) -> nalgebra_glm::Vec3 {
        self.dir
    }

    pub fn inv_dir(&self) -> nalgebra_glm::Vec3 {
        self.inv_dir
    }

    pub fn at(&self, distance: f32) -> nalgebra_glm::Vec3 {
        self.origin + self.dir.normalize() * distance
    }
}
