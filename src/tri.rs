#[derive(Clone, Copy)]
pub struct Tri {
    v0: nalgebra_glm::Vec3,
    v1: nalgebra_glm::Vec3,
    v2: nalgebra_glm::Vec3,
    // Pre-computed
    normal: nalgebra_glm::Vec3,
}

impl Tri {
    pub fn new(v0: nalgebra_glm::Vec3, v1: nalgebra_glm::Vec3, v2: nalgebra_glm::Vec3) -> Tri {
        Self {
            v0,
            v1,
            v2,
            normal: (v1 - v0).cross(&(v2 - v0)).normalize(),
        }
    }

    pub fn v0(&self) -> nalgebra_glm::Vec3 {
        self.v0
    }

    pub fn v1(&self) -> nalgebra_glm::Vec3 {
        self.v1
    }

    pub fn v2(&self) -> nalgebra_glm::Vec3 {
        self.v2
    }

    pub fn normal(&self) -> nalgebra_glm::Vec3 {
        self.normal
    }
}
