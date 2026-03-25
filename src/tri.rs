#[derive(Clone, Copy)]
pub struct Tri {
    pub v0: nalgebra_glm::Vec3,
    pub v1: nalgebra_glm::Vec3,
    pub v2: nalgebra_glm::Vec3,
}

impl Tri {
    pub fn normal(&self) -> nalgebra_glm::Vec3 {
        (self.v1 - self.v0).cross(&(self.v2 - self.v0)).normalize()
    }
}
