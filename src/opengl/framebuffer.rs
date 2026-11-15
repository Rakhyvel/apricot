//! Framebuffers, for drawing into a texture instead of the screen

use super::*;

/// An OpenGL Frame Buffer Object
pub struct Fbo {
    pub(crate) id: GLuint,
}

impl Default for Fbo {
    fn default() -> Self {
        Self::new()
    }
}

impl Fbo {
    /// Create a new Frame Buffer Object
    pub fn new() -> Self {
        let mut id: GLuint = 0;
        unsafe {
            gl::GenFramebuffers(1, &mut id);
        }
        check_gl();
        Self { id }
    }

    /// Bind this FBO
    pub fn bind(&self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.id);
        }
        check_gl();
    }

    /// Unbind this FBO
    pub fn unbind(&self) {
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0) }
        check_gl();
    }

    /// Draw depth into `texture` and no color. Fails if the GPU can't draw that way
    pub fn attach_depth(&self, texture: &Texture) -> Result<(), Error> {
        self.bind();
        let status = unsafe {
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::DEPTH_ATTACHMENT,
                gl::TEXTURE_2D,
                texture.id,
                0,
            );
            gl::DrawBuffer(gl::NONE);
            gl::ReadBuffer(gl::NONE);
            gl::CheckFramebufferStatus(gl::FRAMEBUFFER)
        };
        check_gl();
        self.unbind();
        match status {
            gl::FRAMEBUFFER_COMPLETE => Ok(()),
            status => Err(Error::Framebuffer(status)),
        }
    }
}

impl Drop for Fbo {
    fn drop(&mut self) {
        unsafe { gl::DeleteFramebuffers(1, &self.id) };
    }
}
