use nalgebra_glm::{vec3, Vec3};

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
}

impl MeshData {
    /// An XY quad spanning -1..1 that faces the -Z direction.
    pub fn quad() -> Self {
        Self {
            positions: vec![
                -1.0, -1.0, 0.0, //
                1.0, -1.0, 0.0, //
                1.0, 1.0, 0.0, //
                -1.0, 1.0, 0.0,
            ],
            normals: [0.0, 0.0, -1.0].repeat(4),
            uvs: vec![
                1.0, 1.0, 0.0, //
                0.0, 1.0, 0.0, //
                0.0, 0.0, 0.0, //
                1.0, 0.0, 0.0, //
            ],
            indices: vec![0, 2, 1, 0, 3, 2],
        }
    }

    /// A unit cube centered at the origin, flat face normals.
    pub fn cube() -> Self {
        let faces: [(Vec3, Vec3, Vec3); 6] = [
            (
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, 1.0, 0.0),
                vec3(0.0, 0.0, 1.0),
            ),
            (
                vec3(-1.0, 0.0, 0.0),
                vec3(0.0, 0.0, 1.0),
                vec3(0.0, 1.0, 0.0),
            ),
            (
                vec3(0.0, 1.0, 0.0),
                vec3(0.0, 0.0, 1.0),
                vec3(1.0, 0.0, 0.0),
            ),
            (
                vec3(0.0, -1.0, 0.0),
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, 0.0, 1.0),
            ),
            (
                vec3(0.0, 0.0, 1.0),
                vec3(1.0, 0.0, 0.0),
                vec3(0.0, 1.0, 0.0),
            ),
            (
                vec3(0.0, 0.0, -1.0),
                vec3(0.0, 1.0, 0.0),
                vec3(1.0, 0.0, 0.0),
            ),
        ];

        let mut data = Self::default();
        for (face, (n, u, v)) in faces.iter().enumerate() {
            for (su, sv) in [(-1.0, -1.0), (1.0, -1.0), (1.0, 1.0), (-1.0, 1.0)] {
                let p = (n + u * su + v * sv) * 0.5;
                data.positions.extend_from_slice(&[p.x, p.y, p.z]);
                data.normals.extend_from_slice(&[n.x, n.y, n.z]);
                data.uvs
                    .extend_from_slice(&[(su + 1.0) * 0.5, (sv + 1.0) * 0.5, 0.0]);
            }
            let base = face as u32 * 4;
            data.indices
                .extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        }
        data
    }

    pub fn plane() -> Self {
        Self {
            positions: vec![
                -0.5, 0.0, 0.5, //
                0.5, 0.0, 0.5, //
                0.5, 0.0, -0.5, //
                -0.5, 0.0, -0.5,
            ],
            normals: [0.0, 1.0, 0.0].repeat(4),
            uvs: vec![
                0.0, 0.0, 0.0, //
                1.0, 0.0, 0.0, //
                1.0, 1.0, 0.0, //
                0.0, 1.0, 0.0,
            ],
            indices: vec![0, 1, 2, 0, 2, 3],
        }
    }

    /// Regular polygon with radius 1 in the XY plane, first vertex on +X
    pub fn ngon(sides: u32) -> Self {
        let mut positions = vec![0.0, 0.0, 0.0]; // the center
        let mut indices = Vec::new();
        for i in 0..sides {
            let a = i as f32 / sides as f32 * std::f32::consts::TAU;
            positions.extend_from_slice(&[a.cos(), a.sin(), 0.0]);
            indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % sides]);
        }
        let n = positions.len() / 3;
        Self {
            positions,
            normals: [0.0, 0.0, 1.0].repeat(n),
            uvs: vec![0.0; n * 3],
            indices,
        }
    }

    /// Outline of a regular polygon, outer radius 1, inner radius `inner` (0..1)
    pub fn ngon_ring(sides: u32, inner: f32) -> Self {
        let mut positions = Vec::new();
        let mut indices = Vec::new();
        for i in 0..sides {
            let a = i as f32 / sides as f32 * std::f32::consts::TAU;
            let (c, s) = (a.cos(), a.sin());
            positions.extend_from_slice(&[c, s, 0.0]); // outer, index 2i
            positions.extend_from_slice(&[c * inner, s * inner, 0.0]); // inner, index 2i+1
        }
        for i in 0..sides {
            let j = (i + 1) % sides;
            let (o0, i0, o1, i1) = (2 * i, 2 * i + 1, 2 * j, 2 * j + 1);
            indices.extend_from_slice(&[o0, o1, i0, i0, o1, i1]);
        }
        let n = positions.len() / 3;
        Self {
            positions,
            normals: [0.0, 0.0, 1.0].repeat(n),
            uvs: vec![0.0; n * 3],
            indices,
        }
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    #[cfg(test)]
    fn position(&self, i: u32) -> Vec3 {
        let i = i as usize * 3;
        vec3(
            self.positions[i],
            self.positions[i + 1],
            self.positions[i + 2],
        )
    }

    #[cfg(test)]
    fn normal(&self, i: u32) -> Vec3 {
        let i = i as usize * 3;
        vec3(self.normals[i], self.normals[i + 1], self.normals[i + 2])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check(data: &MeshData) {
        let n = data.vertex_count();
        assert_eq!(data.normals.len(), n * 3);
        assert_eq!(data.uvs.len(), n * 3);
        assert_eq!(data.indices.len() % 3, 0);
        assert!(data.indices.iter().all(|&i| (i as usize) < n));

        for i in 0..n as u32 {
            assert!((data.normal(i).norm() - 1.0).abs() < 1e-5);
        }

        for tri in data.indices.chunks(3) {
            let (a, b, c) = (
                data.position(tri[0]),
                data.position(tri[1]),
                data.position(tri[2]),
            );
            let face_normal = (b - a).cross(&(c - a));
            assert!(
                face_normal.dot(&data.normal(tri[0])) > 0.0,
                "triangle {tri:?} winds backwards"
            );
        }
    }

    #[test]
    fn quad_is_valid() {
        check(&MeshData::quad());
    }

    #[test]
    fn cube_is_valid() {
        let cube = MeshData::cube();
        check(&cube);
        assert!(cube.positions.iter().all(|p| p.abs() == 0.5));
    }

    #[test]
    fn plane_is_valid() {
        check(&MeshData::plane());
    }
}
