//! Images on the GPU

use std::cell::Cell;

use gl::types::{GLenum, GLint, GLuint};

use super::check_gl;

/// An OpenGL Texture
pub struct Texture {
    pub(crate) id: GLuint,
    width: Cell<i32>,
    height: Cell<i32>,
}

impl Default for Texture {
    fn default() -> Self {
        Self::new()
    }
}

impl Texture {
    /// Create a new blank OpenGL texture
    pub fn new() -> Self {
        let mut id: GLuint = 0;
        unsafe { gl::GenTextures(1, &mut id) }
        check_gl();
        Self {
            id,
            width: Cell::new(0),
            height: Cell::new(0),
        }
    }

    /// Bind this texture
    pub fn bind(&self) {
        unsafe { gl::BindTexture(gl::TEXTURE_2D, self.id) }
        check_gl();
    }

    /// Fill the texture with tightly packed RGBA bytes, `width * height * 4` of them. Updating a
    /// texture without changing its size is cheap
    pub fn set_pixels(&self, width: i32, height: i32, data: &[u8]) {
        assert_eq!(
            data.len(),
            (width * height * 4) as usize,
            "expected {width}x{height} RGBA pixels"
        );
        self.bind();

        if self.get_dimensions() == Some((width, height)) {
            // Same size, so just overwrite the pixels instead of reallocating
            unsafe {
                gl::TexSubImage2D(
                    gl::TEXTURE_2D,
                    0,
                    0,
                    0,
                    width,
                    height,
                    gl::RGBA,
                    gl::UNSIGNED_BYTE,
                    data.as_ptr() as *const _,
                );
            }
            check_gl();
            return;
        }

        self.set_size(width, height);
        unsafe {
            set_sampler(gl::CLAMP_TO_EDGE, gl::NEAREST);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width,
                height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
            check_gl();
        }
    }

    /// Load this texture as a depth buffer
    pub fn load_depth_buffer(&self, width: i32, height: i32) {
        self.bind();
        self.set_size(width, height);

        unsafe {
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::DEPTH_COMPONENT32F as GLint,
                width,
                height,
                0,
                gl::DEPTH_COMPONENT,
                gl::FLOAT,
                std::ptr::null(),
            );
            check_gl();
            set_sampler(gl::CLAMP_TO_BORDER, gl::LINEAR);
            let border_color: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
            gl::TexParameterfv(
                gl::TEXTURE_2D,
                gl::TEXTURE_BORDER_COLOR,
                border_color.as_ptr(),
            );
            check_gl();
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_COMPARE_MODE,
                gl::COMPARE_REF_TO_TEXTURE as GLint,
            );
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_COMPARE_FUNC,
                gl::LEQUAL as GLint,
            );
            check_gl();
        }
    }

    /// Method to call after binding
    /// Activate this texture
    pub fn activate(&self, unit: u32) {
        unsafe {
            gl::ActiveTexture(gl::TEXTURE0 + unit);
            self.bind();
        }
        check_gl();
    }

    /// The width and height of this texture, or `None` if it has no pixels yet
    pub fn get_dimensions(&self) -> Option<(i32, i32)> {
        let (width, height) = (self.width.get(), self.height.get());
        (width > 0 && height > 0).then_some((width, height))
    }

    fn set_size(&self, width: i32, height: i32) {
        self.width.set(width);
        self.height.set(height);
    }
}

/// Set how the bound texture repeats past its edges and blends when scaled
fn set_sampler(wrap: GLenum, filter: GLenum) {
    unsafe {
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_S, wrap as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_WRAP_T, wrap as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, filter as GLint);
        gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, filter as GLint);
    }
    check_gl();
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe { gl::DeleteTextures(1, &self.id) };
    }
}
