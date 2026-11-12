//! This module defines a ray

/// A ray data structure
pub struct Ray {
    /// Where the ray starts.
    origin: nalgebra_glm::Vec3,
    /// The direction of the ray. Normalized on construction.
    dir: nalgebra_glm::Vec3,
    /// The inverse direction of the ray. Pre-computed on construction.
    inv_dir: nalgebra_glm::Vec3,
}

impl Ray {
    pub fn new(origin: nalgebra_glm::Vec3, dir: nalgebra_glm::Vec3) -> Self {
        Self {
            origin,
            dir: dir.normalize(),
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
        self.origin + self.dir * distance
    }
}

/// Where a ray hit something
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit {
    /// How far along the ray the hit is
    pub t: f32,
    /// Where the hit is
    pub point: nalgebra_glm::Vec3,
    /// The surface's normal at the hit, flipped if needed so it faces back toward the ray
    pub normal: nalgebra_glm::Vec3,
    /// Whether the ray hit the outside of the surface rather than the inside
    pub front_face: bool,
}

impl RayHit {
    /// A hit `t` along `ray` on a surface whose normal points out along `outward`
    pub fn new(ray: &Ray, t: f32, outward: nalgebra_glm::Vec3) -> Self {
        let front_face = ray.dir().dot(&outward) < 0.0;
        Self {
            t,
            point: ray.at(t),
            normal: if front_face { outward } else { -outward },
            front_face,
        }
    }
}
