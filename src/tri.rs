use crate::ray::Ray;

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

    pub fn raycast(&self, ray: &Ray) -> Option<f32> {
        let e1 = self.v1 - self.v0;
        let e2 = self.v2 - self.v0;

        let h = ray.dir().cross(&e2);
        let det = e1.dot(&h);

        let inv_det = 1.0 / det;
        let s = ray.origin() - self.v0;
        let u = inv_det * s.dot(&h);
        if !(0.0..=1.0).contains(&u) {
            return None;
        }

        let q = s.cross(&e1);
        let v = inv_det * ray.dir().dot(&q);
        if v < 0.0 || u + v > 1.0 {
            return None;
        }

        Some(inv_det * e2.dot(&q))
    }
}
