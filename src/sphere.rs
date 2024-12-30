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
}
