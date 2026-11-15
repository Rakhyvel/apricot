//! Keeping far-off positions precise by drawing them relative to a nearby origin

use nalgebra_glm::{DVec3, Mat4, Vec3};

/// A movable `f64` origin, for turning far-off `f64` positions into small `f32` ones near it
#[derive(Debug, Copy, Clone, Default)]
pub struct FloatingOrigin {
    pub pos: DVec3,
}

impl FloatingOrigin {
    /// An origin at `pos`
    pub fn new(pos: DVec3) -> Self {
        Self { pos }
    }

    /// Where `world` is relative to the origin
    pub fn relative(&self, world: DVec3) -> Vec3 {
        nalgebra_glm::convert(world - self.pos)
    }

    /// A model transform for something at `world`, with its own rotation and scale
    pub fn transform(&self, world: DVec3, rotation_scale: Mat4) -> Mat4 {
        nalgebra_glm::translation(&self.relative(world)) * rotation_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn floating_origin_keeps_precision_far_away() {
        // 1000 km out, f32 can only step in ~6 cm increments, but the offset is small
        let origin = FloatingOrigin::new(DVec3::new(1.0e6, 0.0, 0.0));
        let rel = origin.relative(DVec3::new(1.0e6 + 0.001, 0.0, 0.0));
        assert!((rel.x - 0.001).abs() < 1e-6);
    }

    #[test]
    fn floating_origin_transform_moves_then_keeps_rotation_scale() {
        let origin = FloatingOrigin::new(DVec3::new(10.0, 0.0, 0.0));
        let scale = nalgebra_glm::scaling(&nalgebra_glm::vec3(2.0, 2.0, 2.0));
        let m = origin.transform(DVec3::new(13.0, 0.0, 0.0), scale);
        let p = m * nalgebra_glm::vec4(1.0, 0.0, 0.0, 1.0);
        assert!((p.x - 5.0).abs() < 1e-6); // scaled to 2, then moved 3 along
    }
}
