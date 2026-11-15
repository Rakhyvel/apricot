//! Vertices with a custom layout, drawn however the shader wants

use std::ptr::null;

use gl::types::GLenum;

use super::{check_gl, Buffer, Vao};

/// How a `RawMesh`'s vertices join up
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Primitive {
    /// Every 3 vertices make a triangle
    Triangles,
    /// Every 2 vertices make a line
    Lines,
    /// One line through every vertex in order
    LineStrip,
    /// Each vertex is its own dot
    Points,
}

impl Primitive {
    fn gl(self) -> GLenum {
        match self {
            Primitive::Triangles => gl::TRIANGLES,
            Primitive::Lines => gl::LINES,
            Primitive::LineStrip => gl::LINE_STRIP,
            Primitive::Points => gl::POINTS,
        }
    }
}

/// Vertices with whatever layout a shader wants, in one interleaved buffer
pub struct RawMesh {
    vao: Vao,
    vbo: Buffer<f32>,
    ibo: Option<Buffer<u32>>,
    stride: usize,
    count: usize,
    primitive: Primitive,
}

impl RawMesh {
    /// A mesh from `vertices`, split up by `layout`: how many floats each shader input takes, in
    /// order. `[3, 1]` gives slot 0 three floats and slot 1 one float, repeating for every vertex
    pub fn new(vertices: &[f32], layout: &[usize], primitive: Primitive) -> Self {
        let vao = Vao::gen();
        let vbo = Buffer::<f32>::gen(gl::ARRAY_BUFFER);
        let stride: usize = layout.iter().sum();
        assert!(stride > 0, "a vertex layout needs at least one float");

        let mut offset = 0;
        for (slot, &components) in layout.iter().enumerate() {
            vao.attrib(slot as u32, &vbo, components, stride, offset);
            offset += components;
        }
        vao.unbind();
        vbo.unbind();

        let mut mesh = Self {
            vao,
            vbo,
            ibo: None,
            stride,
            count: 0,
            primitive,
        };
        mesh.set_vertices(vertices);
        mesh
    }

    /// A mesh from `vertices` that draws them in the order `indices` gives
    pub fn with_indices(
        vertices: &[f32],
        indices: &[u32],
        layout: &[usize],
        primitive: Primitive,
    ) -> Self {
        let mut mesh = Self::new(vertices, layout, primitive);
        let ibo = Buffer::<u32>::gen(gl::ELEMENT_ARRAY_BUFFER);
        ibo.set_data(indices);
        mesh.vao.set_index_buffer(&ibo);
        mesh.vao.unbind();
        mesh.count = indices.len();
        mesh.ibo = Some(ibo);
        mesh
    }

    /// Replace the vertices, keeping the same layout
    pub fn set_vertices(&mut self, vertices: &[f32]) {
        assert_eq!(
            vertices.len() % self.stride,
            0,
            "expected whole vertices of {} floats",
            self.stride
        );
        self.vbo.set_data(vertices);
        self.vbo.unbind();
        if self.ibo.is_none() {
            self.count = vertices.len() / self.stride;
        }
    }

    /// How many vertices get drawn
    pub fn vertex_count(&self) -> usize {
        self.count
    }

    pub(crate) fn draw(&self) {
        self.vao.bind();
        unsafe {
            match self.ibo {
                Some(_) => gl::DrawElements(
                    self.primitive.gl(),
                    self.count as i32,
                    gl::UNSIGNED_INT,
                    null(),
                ),
                None => gl::DrawArrays(self.primitive.gl(), 0, self.count as i32),
            }
        }
        check_gl();
        self.vao.unbind();
    }
}
