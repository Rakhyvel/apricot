use nalgebra_glm::{DVec3, Vec3};

use crate::camera;

/// A component that stores a 64-bit floating point world position
pub struct WorldPosition {
    pub pos: DVec3,
}

/// A wrapper for Camera that stores a 64-bit floating point world position
pub struct Camera {
    pub world_pos: DVec3,
    pub inner: camera::Camera,
}

impl Camera {
    pub fn sync(&mut self, target: DVec3) {
        self.inner.set_position(Vec3::zeros());
        self.inner
            .set_lookat(nalgebra_glm::convert(target - self.world_pos));
    }
}
