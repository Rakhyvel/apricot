//! Meshes on the CPU, with their positions, normals, uvs, and indices, and ways to build common shapes

use nalgebra_glm::{vec3, Vec3};
use obj::{load_obj, Obj, TexturedVertex};

use crate::{error::Error, geometry::tri::Tri};

mod shape;

pub use shape::MeshShape;

#[derive(Clone, Debug, Default)]
pub struct MeshData {
    pub positions: Vec<f32>,
    pub normals: Vec<f32>,
    pub uvs: Vec<f32>,
    pub indices: Vec<u32>,
}

impl MeshData {
    /// Read a mesh out of the bytes of a Wavefront .obj file
    pub fn from_obj(bytes: &[u8]) -> Result<Self, Error> {
        let obj: Obj<TexturedVertex, u32> = load_obj(bytes)?;
        let flat = |f: fn(&TexturedVertex) -> [f32; 3]| obj.vertices.iter().flat_map(f).collect();
        Ok(Self {
            positions: flat(|v| v.position),
            normals: flat(|v| v.normal),
            uvs: flat(|v| v.texture),
            indices: obj.indices,
        })
    }

    /// Every triangle, in the mesh's own space
    pub fn triangles(&self) -> impl Iterator<Item = Tri> + '_ {
        let corner = |i: u32| self.position_at(i);
        self.indices
            .chunks_exact(3)
            .map(move |t| Tri::new(corner(t[0]), corner(t[1]), corner(t[2])))
    }

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

    /// Every vertex's position, normal, and uv back to back, 9 floats per vertex
    pub(crate) fn interleaved(&self) -> Vec<f32> {
        (0..self.vertex_count())
            .flat_map(|i| {
                let v = i * 3..i * 3 + 3;
                [
                    &self.positions[v.clone()],
                    &self.normals[v.clone()],
                    &self.uvs[v],
                ]
                .concat()
            })
            .collect()
    }

    pub fn vertex_count(&self) -> usize {
        self.positions.len() / 3
    }

    fn position_at(&self, i: u32) -> Vec3 {
        let i = i as usize * 3;
        vec3(
            self.positions[i],
            self.positions[i + 1],
            self.positions[i + 2],
        )
    }

    #[cfg(test)]
    pub(crate) fn position(&self, i: u32) -> Vec3 {
        self.position_at(i)
    }

    #[cfg(test)]
    pub(crate) fn normal(&self, i: u32) -> Vec3 {
        let i = i as usize * 3;
        vec3(self.normals[i], self.normals[i + 1], self.normals[i + 2])
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    pub(crate) fn check(data: &MeshData) {
        let n = data.vertex_count();
        assert!(n > 0 && !data.indices.is_empty(), "empty mesh");
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

    pub(crate) fn uv(data: &MeshData, i: u32) -> (f32, f32) {
        let i = i as usize * 3;
        (data.uvs[i], data.uvs[i + 1])
    }

    /// The total area of every triangle
    fn area(data: &MeshData) -> f32 {
        data.triangles()
            .map(|t| (t.v1() - t.v0()).cross(&(t.v2() - t.v0())).norm() / 2.0)
            .sum()
    }

    /// The volume a closed mesh holds. Negative if its triangles face inward
    fn volume(data: &MeshData) -> f32 {
        data.triangles()
            .map(|t| t.v0().dot(&t.v1().cross(&t.v2())) / 6.0)
            .sum()
    }

    /// How much area a regular polygon with radius 1 covers
    fn ngon_area(sides: u32) -> f32 {
        sides as f32 / 2.0 * (std::f32::consts::TAU / sides as f32).sin()
    }

    #[test]
    fn cube_is_a_closed_unit_cube_with_a_whole_texture_per_face() {
        let cube = MeshData::cube();
        assert_eq!(cube.vertex_count(), 24);
        assert_eq!(cube.indices.len(), 36);
        assert!((area(&cube) - 6.0).abs() < 1e-5);
        assert!((volume(&cube) - 1.0).abs() < 1e-5);
        for face in 0..6 {
            let mut corners: Vec<(f32, f32)> = (face * 4..face * 4 + 4).map(|i| uv(&cube, i)).collect();
            corners.sort_by(|a, b| a.partial_cmp(b).unwrap());
            assert_eq!(corners, [(0.0, 0.0), (0.0, 1.0), (1.0, 0.0), (1.0, 1.0)], "face {face}");
            for i in face * 4..face * 4 + 4 {
                // Every corner of a face sits half a unit out along its normal
                assert_eq!(cube.position(i).dot(&cube.normal(i)), 0.5, "vertex {i}");
            }
        }
    }

    #[test]
    fn plane_is_a_unit_square_facing_up() {
        let plane = MeshData::plane();
        check(&plane);
        assert!((area(&plane) - 1.0).abs() < 1e-6);
        for i in 0..4 {
            let p = plane.position(i);
            assert_eq!((p.x.abs(), p.y, p.z.abs()), (0.5, 0.0, 0.5));
            assert_eq!(plane.normal(i), vec3(0.0, 1.0, 0.0));
            // The texture runs across x and up toward -z
            assert_eq!(uv(&plane, i), (p.x + 0.5, 0.5 - p.z));
        }
    }

    #[test]
    fn ngon_fans_out_from_the_middle() {
        for sides in [3, 5, 8] {
            let ngon = MeshData::ngon(sides);
            check(&ngon);
            assert_eq!(ngon.vertex_count(), sides as usize + 1);
            assert_eq!(ngon.indices.len(), sides as usize * 3);
            assert_eq!(ngon.position(0), vec3(0.0, 0.0, 0.0));
            for i in 0..sides {
                let a = i as f32 / sides as f32 * std::f32::consts::TAU;
                let rim = ngon.position(1 + i);
                assert!((rim - vec3(a.cos(), a.sin(), 0.0)).norm() < 1e-5, "{sides} sides, corner {i}");
            }
            assert!((area(&ngon) - ngon_area(sides)).abs() < 1e-4, "{sides} sides");
        }
    }

    #[test]
    fn ngon_ring_is_the_band_between_two_polygons() {
        for (sides, inner) in [(4, 0.5), (6, 0.25), (9, 0.8)] {
            let ring = MeshData::ngon_ring(sides, inner);
            check(&ring);
            assert_eq!(ring.vertex_count(), 2 * sides as usize);
            for i in 0..sides {
                let a = i as f32 / sides as f32 * std::f32::consts::TAU;
                let dir = vec3(a.cos(), a.sin(), 0.0);
                assert!((ring.position(2 * i) - dir).norm() < 1e-5);
                assert!((ring.position(2 * i + 1) - dir * inner).norm() < 1e-5);
            }
            let expected = ngon_area(sides) * (1.0 - inner * inner);
            assert!((area(&ring) - expected).abs() < 1e-4, "{sides} sides");
        }
    }

    #[test]
    fn uv_sphere_is_a_closed_sphere_with_the_texture_wrapped_once() {
        let (segments, rings) = (32, 16);
        let sphere = MeshData::uv_sphere(segments, rings);
        check(&sphere);
        // A bit under a true sphere's, since the flat triangles cut inside it
        let (true_area, true_volume) = (4.0 * std::f32::consts::PI, 4.0 / 3.0 * std::f32::consts::PI);
        assert!(area(&sphere) < true_area && area(&sphere) > 0.98 * true_area);
        assert!(volume(&sphere) < true_volume && volume(&sphere) > 0.96 * true_volume);
        for r in 0..=rings {
            for s in 0..=segments {
                let i = r * (segments + 1) + s;
                assert_eq!(sphere.normal(i), sphere.position(i));
                assert_eq!(uv(&sphere, i), (s as f32 / segments as f32, r as f32 / rings as f32));
            }
        }
        // The top of the texture is at the north pole
        assert!((sphere.position(0) - vec3(0.0, 1.0, 0.0)).norm() < 1e-6);
    }

    #[test]
    fn icosphere_starts_as_a_regular_icosahedron() {
        let ico = MeshData::icosphere(0);
        let lengths: Vec<f32> = ico
            .triangles()
            .flat_map(|t| [(t.v0() - t.v1()).norm(), (t.v1() - t.v2()).norm(), (t.v2() - t.v0()).norm()])
            .collect();
        assert!(lengths.iter().all(|l| (l - lengths[0]).abs() < 1e-5), "{lengths:?}");
    }

    #[test]
    fn icosphere_closes_in_on_a_sphere_as_it_splits() {
        let true_area = 4.0 * std::f32::consts::PI;
        let mut last = 0.0;
        for subdivisions in 0..4 {
            let ico = MeshData::icosphere(subdivisions);
            let a = area(&ico);
            assert!(a > last && a < true_area, "{subdivisions} splits: area {a}");
            assert!(volume(&ico) > 0.0, "faces point out");
            last = a;
        }
        assert!(last > 0.98 * true_area);
    }

    #[test]
    fn icosphere_faces_point_out_from_their_middle_and_sample_the_texture_there() {
        let ico = MeshData::icosphere(1);
        for t in 0..(ico.vertex_count() / 3) as u32 {
            let corners = [ico.position(3 * t), ico.position(3 * t + 1), ico.position(3 * t + 2)];
            let middle = (corners[0] + corners[1] + corners[2]).normalize();
            for i in 3 * t..3 * t + 3 {
                let n = ico.normal(i);
                assert!((n - middle).norm() < 1e-5, "face {t}");
                // The uv points back at the same spot on the sphere
                let (u, v) = uv(&ico, i);
                assert!((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v), "face {t}: uv ({u}, {v})");
                let (angle, tilt) = ((u - 0.5) * std::f32::consts::TAU, v * std::f32::consts::PI);
                let back = vec3(tilt.sin() * angle.cos(), tilt.cos(), tilt.sin() * angle.sin());
                assert!((back - n).norm() < 1e-4, "face {t}: uv ({u}, {v})");
            }
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
    fn from_obj_reads_one_triangle() {
        let obj =
            b"v 0 0 0\nv 1 0 0\nv 0 1 0\nvt 0 0\nvt 1 0\nvt 0 1\nvn 0 0 1\nf 1/1/1 2/2/1 3/3/1\n";
        let data = MeshData::from_obj(obj).unwrap();
        check(&data);
        assert_eq!(data.vertex_count(), 3);
        assert_eq!(data.triangles().count(), 1);
    }

    #[test]
    fn from_obj_reads_more_vertices_than_fit_in_a_u16() {
        // Separate triangles, so every vertex is its own
        let triangles = 22_000;
        let mut obj = String::from("vt 0 0\nvn 0 0 1\n");
        for i in 0..triangles {
            let x = i as f32;
            obj += &format!("v {x} 0 0\nv {x} 1 0\nv {} 0 0\n", x + 1.0);
        }
        for i in 0..triangles {
            let first = 3 * i + 1;
            obj += &format!("f {}/1/1 {}/1/1 {}/1/1\n", first, first + 1, first + 2);
        }
        let data = MeshData::from_obj(obj.as_bytes()).unwrap();
        assert_eq!(data.vertex_count(), 3 * triangles);
        assert_eq!(data.triangles().count(), triangles);
        assert_eq!(*data.indices.last().unwrap() as usize, 3 * triangles - 1);
    }

    #[test]
    fn triangles_match_the_indices() {
        let quad = MeshData::quad();
        let tris: Vec<Tri> = quad.triangles().collect();
        assert_eq!(tris.len(), 2);
        assert_eq!(tris[0].v0(), quad.position(quad.indices[0]));
        assert_eq!(tris[1].v2(), quad.position(quad.indices[5]));
    }

    #[test]
    fn interleaving_keeps_each_vertex_together() {
        let quad = MeshData::quad();
        let all = quad.interleaved();
        assert_eq!(all.len(), quad.vertex_count() * 9);
        for i in 0..quad.vertex_count() {
            let v = &all[i * 9..i * 9 + 9];
            assert_eq!(v[0..3], quad.positions[i * 3..i * 3 + 3]);
            assert_eq!(v[3..6], quad.normals[i * 3..i * 3 + 3]);
            assert_eq!(v[6..9], quad.uvs[i * 3..i * 3 + 3]);
        }
    }

    #[test]
    fn plane_is_valid() {
        check(&MeshData::plane());
    }
}
