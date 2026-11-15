//! The OpenGL layer. Everything that calls `gl::` lives in here

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CStr, CString},
    marker::PhantomData,
    path::Path,
    ptr::{null, null_mut},
};

use gl::types::{GLchar, GLenum, GLint, GLuint};

use image::{EncodableLayout, ImageError};
use nalgebra_glm::{Mat4, Vec2, Vec3, Vec4};

use crate::error::Error;

mod buffer;
mod draw_options;
mod framebuffer;
mod program;
mod raw_mesh;
mod texture;

pub use buffer::{Buffer, Vao};
pub use draw_options::{DrawOptions, Stencil};
pub use framebuffer::Fbo;
pub use program::{create_program, Program, Shader, Uniform};
pub use raw_mesh::{Primitive, RawMesh};
pub use texture::Texture;

/// Load OpenGL's functions with `loader` and set up apricot's default state
pub(crate) fn init(loader: impl FnMut(&'static str) -> *const std::ffi::c_void) {
    gl::load_with(loader);
    unsafe {
        gl::DepthFunc(gl::LESS);
        gl::CullFace(gl::BACK);
        gl::Enable(gl::MULTISAMPLE);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
    }
    DrawOptions::default().restore();
}

/// Draw to the `width` by `height` pixels in the corner of whatever's being drawn to
pub(crate) fn set_viewport(width: i32, height: i32) {
    unsafe { gl::Viewport(0, 0, width, height) };
    check_gl();
}

/// Only draw inside `rect`, measured from the top left of a window `window_height` pixels tall.
/// `None` draws everywhere
pub(crate) fn set_scissor(rect: Option<crate::geometry::rectangle::Rectangle>, window_height: i32) {
    unsafe {
        match rect {
            Some(r) => {
                gl::Enable(gl::SCISSOR_TEST);
                gl::Scissor(
                    r.pos.x as i32,
                    window_height - (r.pos.y + r.size.y) as i32,
                    r.size.x as i32,
                    r.size.y as i32,
                );
            }
            None => gl::Disable(gl::SCISSOR_TEST),
        }
    }
    check_gl();
}

/// Fill the screen with `color`, and reset depth and stencil
pub(crate) fn clear(color: Vec4) {
    unsafe {
        gl::ClearColor(color.x, color.y, color.z, color.w);
        gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
    }
    check_gl();
}

/// Reset depth only
pub(crate) fn clear_depth() {
    unsafe { gl::Clear(gl::DEPTH_BUFFER_BIT) };
    check_gl();
}

/// Bind the texture with GL id `id` to texture unit `unit`, then go back to unit 0
pub(crate) fn bind_texture_id(unit: u32, id: GLuint) {
    unsafe {
        gl::ActiveTexture(gl::TEXTURE0 + unit);
        gl::BindTexture(gl::TEXTURE_2D, id);
        gl::ActiveTexture(gl::TEXTURE0);
    }
    check_gl();
}

fn check_gl() {
    #[cfg(debug_assertions)]
    if let Some(error_message) = get_last_opengl_error() {
        panic!("OpenGL Error:\n{}", error_message);
    }
}

#[cfg(debug_assertions)]
fn get_last_opengl_error() -> Option<String> {
    let mut errors = Vec::new();

    unsafe {
        loop {
            let error_code = gl::GetError();
            if error_code == gl::NO_ERROR {
                break;
            }

            let error_message = match error_code {
                gl::INVALID_ENUM => "GL_INVALID_ENUM: An unacceptable value is specified for an enumerated argument.".to_string(),
                gl::INVALID_VALUE => "GL_INVALID_VALUE: A numeric argument is out of range.".to_string(),
                gl::INVALID_OPERATION => "GL_INVALID_OPERATION: The specified operation is not allowed in the current state.".to_string(),
                gl::STACK_OVERFLOW => "GL_STACK_OVERFLOW: This command would cause a stack overflow.".to_string(),
                gl::STACK_UNDERFLOW => "GL_STACK_UNDERFLOW: This command would cause a stack underflow.".to_string(),
                gl::OUT_OF_MEMORY => "GL_OUT_OF_MEMORY: There is not enough memory left to execute the command.".to_string(),
                gl::INVALID_FRAMEBUFFER_OPERATION => "GL_INVALID_FRAMEBUFFER_OPERATION: The framebuffer object is not complete.".to_string(),
                _ => format!("Unknown OpenGL error code: {}", error_code),
            };

            errors.push(error_message);
        }
    }

    if errors.is_empty() {
        None
    } else {
        Some(errors.join("\n"))
    }
}
