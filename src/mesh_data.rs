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

    /// A smooth sphere with radius 1, `segments` slices around and `rings` from top to bottom.
    /// The texture wraps around it once, with the top of the image at the north pole (+Y)
    pub fn uv_sphere(segments: u32, rings: u32) -> Self {
        let (segments, rings) = (segments.max(3), rings.max(2));
        let mut positions = Vec::new();
        let mut uvs = Vec::new();
        for r in 0..=rings {
            let theta = r as f32 / rings as f32 * std::f32::consts::PI; // 0 at the top
            for s in 0..=segments {
                // The seam column is doubled so the texture can wrap all the way around
                let phi = s as f32 / segments as f32 * std::f32::consts::TAU;
                positions.extend_from_slice(&[
                    theta.sin() * phi.cos(),
                    theta.cos(),
                    theta.sin() * phi.sin(),
                ]);
                uvs.extend_from_slice(&[s as f32 / segments as f32, r as f32 / rings as f32, 0.0]);
            }
        }

        let mut indices = Vec::new();
        let row = segments + 1;
        for r in 0..rings {
            for s in 0..segments {
                let (a, c) = (r * row + s, r * row + s + 1);
                let (b, d) = (a + row, c + row);
                // Skip the triangles that squash down to nothing at the poles
                if r != 0 {
                    indices.extend_from_slice(&[a, c, b]);
                }
                if r != rings - 1 {
                    indices.extend_from_slice(&[c, d, b]);
                }
            }
        }

        Self {
            normals: positions.clone(), // on a unit sphere the normal is just the position
            positions,
            uvs,
            indices,
        }
    }

    /// A low-poly sphere with radius 1, made by splitting each face of an icosahedron into 4,
    /// `subdivisions` times. 0 gives 20 faces, 1 gives 80, 2 gives 320. Each face is flat shaded
    /// and samples the texture at its center, so there's no seam
    pub fn icosphere(subdivisions: u32) -> Self {
        let t = (1.0 + 5.0f32.sqrt()) / 2.0;
        let mut verts: Vec<Vec3> = [
            (-1.0, t, 0.0),
            (1.0, t, 0.0),
            (-1.0, -t, 0.0),
            (1.0, -t, 0.0),
            (0.0, -1.0, t),
            (0.0, 1.0, t),
            (0.0, -1.0, -t),
            (0.0, 1.0, -t),
            (t, 0.0, -1.0),
            (t, 0.0, 1.0),
            (-t, 0.0, -1.0),
            (-t, 0.0, 1.0),
        ]
        .iter()
        .map(|&(x, y, z)| vec3(x, y, z).normalize())
        .collect();
        let mut faces: Vec<[u32; 3]> = vec![
            [0, 11, 5],
            [0, 5, 1],
            [0, 1, 7],
            [0, 7, 10],
            [0, 10, 11],
            [1, 5, 9],
            [5, 11, 4],
            [11, 10, 2],
            [10, 7, 6],
            [7, 1, 8],
            [3, 9, 4],
            [3, 4, 2],
            [3, 2, 6],
            [3, 6, 8],
            [3, 8, 9],
            [4, 9, 5],
            [2, 4, 11],
            [6, 2, 10],
            [8, 6, 7],
            [9, 8, 1],
        ];

        for _ in 0..subdivisions {
            // Faces that share an edge share its midpoint, so the surface stays closed
            let mut midpoints = std::collections::HashMap::new();
            let mut midpoint = |a: u32, b: u32, verts: &mut Vec<Vec3>| -> u32 {
                *midpoints.entry((a.min(b), a.max(b))).or_insert_with(|| {
                    verts.push(((verts[a as usize] + verts[b as usize]) / 2.0).normalize());
                    verts.len() as u32 - 1
                })
            };
            let mut split = Vec::with_capacity(faces.len() * 4);
            for &[a, b, c] in &faces {
                let ab = midpoint(a, b, &mut verts);
                let bc = midpoint(b, c, &mut verts);
                let ca = midpoint(c, a, &mut verts);
                split.extend_from_slice(&[[a, ab, ca], [ab, b, bc], [ca, bc, c], [ab, bc, ca]]);
            }
            faces = split;
        }

        let mut data = Self::default();
        for (i, &[a, b, c]) in faces.iter().enumerate() {
            let corners = [verts[a as usize], verts[b as usize], verts[c as usize]];
            let normal = ((corners[0] + corners[1] + corners[2]) / 3.0).normalize();
            let u = normal.z.atan2(normal.x) / std::f32::consts::TAU + 0.5;
            let v = normal.y.clamp(-1.0, 1.0).acos() / std::f32::consts::PI;
            for corner in corners {
                data.positions
                    .extend_from_slice(&[corner.x, corner.y, corner.z]);
                data.normals
                    .extend_from_slice(&[normal.x, normal.y, normal.z]);
                data.uvs.extend_from_slice(&[u, v, 0.0]);
            }
            let base = i as u32 * 3;
            data.indices.extend_from_slice(&[base, base + 1, base + 2]);
        }
        data
    }

    /// Flat-shaded terrain on the XZ plane, `cells_x` by `cells_z` cells of size 1, with each
    /// corner raised to `height(x, z)`. The texture stretches over the whole grid once. Scale it
    /// with the model's transform
    pub fn heightmap(cells_x: u32, cells_z: u32, height: impl Fn(f32, f32) -> f32) -> Self {
        let corner = |x: u32, z: u32| {
            let (x, z) = (x as f32, z as f32);
            vec3(x, height(x, z), z)
        };
        let mut data = Self::default();
        for z in 0..cells_z {
            for x in 0..cells_x {
                let tris = [
                    [(x, z), (x, z + 1), (x + 1, z)],
                    [(x + 1, z), (x, z + 1), (x + 1, z + 1)],
                ];
                for tri in tris {
                    let p = tri.map(|(x, z)| corner(x, z));
                    let normal = (p[1] - p[0]).cross(&(p[2] - p[0])).normalize();
                    for (i, (cx, cz)) in tri.into_iter().enumerate() {
                        data.indices.push(data.vertex_count() as u32);
                        data.positions.extend_from_slice(&[p[i].x, p[i].y, p[i].z]);
                        data.normals
                            .extend_from_slice(&[normal.x, normal.y, normal.z]);
                        data.uvs.extend_from_slice(&[
                            cx as f32 / cells_x as f32,
                            cz as f32 / cells_z as f32,
                            0.0,
                        ]);
                    }
                }
            }
        }
        data
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
    fn uv_sphere_is_valid() {
        let sphere = MeshData::uv_sphere(16, 8);
        check(&sphere);
        let n = sphere.vertex_count() as u32;
        assert!((0..n).all(|i| (sphere.position(i).norm() - 1.0).abs() < 1e-5));
    }

    #[test]
    fn icosphere_is_valid() {
        for (subdivisions, faces) in [(0, 20), (1, 80), (2, 320)] {
            let ico = MeshData::icosphere(subdivisions);
            check(&ico);
            assert_eq!(ico.indices.len(), faces * 3);
        }
    }

    #[test]
    fn heightmap_is_valid() {
        let terrain = MeshData::heightmap(4, 3, |x, z| (x * 0.7).sin() + z * 0.2);
        check(&terrain);
        assert_eq!(terrain.indices.len(), 4 * 3 * 2 * 3);
        assert!(terrain.position(0).y.abs() < 1e-6); // corner (0, 0) sits at height(0, 0)
    }

    #[test]
    fn plane_is_valid() {
        check(&MeshData::plane());
    }
}
