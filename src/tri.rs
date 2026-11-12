use std::ops::Range;

use crate::ray::{Ray, RayHit};

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

    /// This triangle moved, rotated, or scaled by `matrix`
    pub fn transformed(&self, matrix: &nalgebra_glm::Mat4) -> Tri {
        let apply = |v: nalgebra_glm::Vec3| (matrix * nalgebra_glm::vec4(v.x, v.y, v.z, 1.0)).xyz();
        Tri::new(apply(self.v0), apply(self.v1), apply(self.v2))
    }

    /// Where `ray` hits the triangle, if that's inside `t_range`. Hits either side
    pub fn raycast(&self, ray: &Ray, t_range: Range<f32>) -> Option<RayHit> {
        let e1 = self.v1 - self.v0;
        let e2 = self.v2 - self.v0;

        let h = ray.dir().cross(&e2);
        let det = e1.dot(&h);

        let inv_det = 1.0 / det; // if det=0.0, this'll fail through the contains check below
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

        let t = inv_det * e2.dot(&q);
        t_range
            .contains(&t)
            .then(|| RayHit::new(ray, t, self.normal))
    }
}

impl core::ops::Mul<f32> for Tri {
    type Output = Tri;

    fn mul(self, rhs: f32) -> Self::Output {
        Tri::new(self.v0 * rhs, self.v1 * rhs, self.v2 * rhs)
    }
}

impl From<Tri> for [f32; 9] {
    fn from(val: Tri) -> Self {
        [
            val.v0.x, val.v0.y, val.v0.z, val.v1.x, val.v1.y, val.v1.z, val.v2.x, val.v2.y,
            val.v2.z,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec3;

    /// Lying flat at y = 0, wound so its normal points up
    fn flat() -> Tri {
        Tri::new(
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 0.0, 2.0),
            vec3(2.0, 0.0, 0.0),
        )
    }

    #[test]
    fn hits_inside_from_both_sides() {
        let down = Ray::new(vec3(0.5, 1.0, 0.5), vec3(0.0, -1.0, 0.0));
        let hit = flat().raycast(&down, 0.0..f32::INFINITY).unwrap();
        assert!((hit.t - 1.0).abs() < 1e-5);
        assert!(hit.front_face);

        let up = Ray::new(vec3(0.5, -1.0, 0.5), vec3(0.0, 1.0, 0.0));
        let hit = flat().raycast(&up, 0.0..f32::INFINITY).unwrap();
        assert!(!hit.front_face);
        assert!((hit.normal - vec3(0.0, -1.0, 0.0)).norm() < 1e-5);
    }

    #[test]
    fn misses_outside_the_edges_and_the_range() {
        let outside = Ray::new(vec3(1.5, 1.0, 1.5), vec3(0.0, -1.0, 0.0));
        assert!(flat().raycast(&outside, 0.0..f32::INFINITY).is_none());
        let short = Ray::new(vec3(0.5, 1.0, 0.5), vec3(0.0, -1.0, 0.0));
        assert!(flat().raycast(&short, 0.0..0.5).is_none());
    }
}
