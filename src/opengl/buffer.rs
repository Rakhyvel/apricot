//! GPU buffers and the vertex arrays that describe them

use super::*;

/// OpenGL Vertex Buffer Object. Contains vertex data given as input to the vertex shader.
pub struct Buffer<T> {
    pub(crate) id: GLuint,
    target: GLenum,
    phantom: PhantomData<T>,
}

impl<T> Buffer<T> {
    /// Create a new OpenGL Buffer
    pub fn gen(target: GLenum) -> Self {
        let mut id: GLuint = 0;
        unsafe {
            gl::GenBuffers(1, &mut id);
        }
        check_gl();
        Buffer::<T> {
            id,
            target,
            phantom: PhantomData::<T> {},
        }
    }

    /// Set the buffer's data
    pub fn set_data(&self, data: &[T]) {
        self.bind();
        unsafe {
            gl::BufferData(
                self.target,
                std::mem::size_of_val(data) as gl::types::GLsizeiptr,
                data.as_ptr() as *const gl::types::GLvoid,
                gl::STATIC_DRAW,
            );
        }
        check_gl();
    }

    /// Bind the buffer in OpenGL
    pub fn bind(&self) {
        unsafe {
            gl::BindBuffer(self.target, self.id);
        }
        check_gl();
    }

    pub fn unbind(&self) {
        unsafe {
            gl::BindBuffer(self.target, 0);
        }
        check_gl();
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        unsafe { gl::DeleteBuffers(1, &self.id) };
    }
}

/// OpenGL Vertex Array Object
pub struct Vao {
    pub(crate) id: GLuint,
}

impl Vao {
    /// Create a new VAO
    pub fn gen() -> Self {
        let mut id: GLuint = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut id);
        }
        check_gl();
        Vao { id }
    }

    /// Bind this VAO. All attributes or index state recorded into it becomes active.
    pub fn bind(&self) {
        unsafe {
            gl::BindVertexArray(self.id);
        }
        check_gl();
    }

    /// Point slot `loc` of this VAO at `vbo`
    pub fn attrib(
        &self,
        loc: u32,
        vbo: &Buffer<f32>,
        num_components: usize,
        stride: usize,
        offset: usize,
    ) {
        self.bind();
        vbo.bind();
        unsafe {
            gl::EnableVertexAttribArray(loc);
            gl::VertexAttribPointer(
                loc,
                num_components as i32,
                gl::FLOAT,
                gl::FALSE,
                (stride * std::mem::size_of::<f32>()) as GLint,
                (offset * std::mem::size_of::<f32>()) as *const _,
            );
        }
        check_gl();
    }

    /// Attach `ibo` as this VAO's index buffer
    pub fn set_index_buffer(&self, ibo: &Buffer<u32>) {
        self.bind();
        ibo.bind();
    }

    /// Unbind this VAO
    pub fn unbind(&self) {
        unsafe {
            gl::BindVertexArray(0);
        }
        check_gl();
    }
}

impl Drop for Vao {
    fn drop(&mut self) {
        unsafe { gl::DeleteVertexArrays(1, &self.id) };
    }
}
