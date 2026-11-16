//! Shaders, programs, and their uniforms

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    ffi::{CStr, CString},
    ptr::{null, null_mut},
};

use gl::types::{GLchar, GLint, GLuint};
use nalgebra_glm::{Mat4, Vec2, Vec3, Vec4};

use super::check_gl;
use crate::error::{Error, ShaderStage};

/// An OpenGL Shader
pub struct Shader {
    id: GLuint,
}

impl Shader {
    /// Compile an OpenGL shader from GLSL souce
    pub fn from_source(source: &CStr, stage: ShaderStage) -> Result<Self, Error> {
        let kind = match stage {
            ShaderStage::Vertex => gl::VERTEX_SHADER,
            ShaderStage::Fragment => gl::FRAGMENT_SHADER,
        };
        let id = unsafe { gl::CreateShader(kind) };
        check_gl();

        unsafe {
            gl::ShaderSource(id, 1, &source.as_ptr(), null());
        }
        check_gl();
        unsafe {
            gl::CompileShader(id);
        }
        check_gl();

        let mut success: GLint = 1;
        unsafe {
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
        }
        check_gl();

        if success == 0 {
            // Error occured!
            let mut len: GLint = 0;
            unsafe { gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len) }
            check_gl();

            let error = create_whitespace_cstring_with_len(len as usize);

            unsafe {
                gl::GetShaderInfoLog(id, len, null_mut(), error.as_ptr() as *mut GLchar);
            }
            check_gl();

            return Err(Error::ShaderCompile {
                stage,
                log: error.to_string_lossy().into_owned(),
            });
        }

        Ok(Shader { id })
    }

    /// Retrieve the OpenGL ID of this shader
    pub(crate) fn id(&self) -> GLuint {
        self.id
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
    }
}

/// An OpenGL program
pub struct Program {
    id: GLuint,
    locations: RefCell<HashMap<String, GLint>>,
}

impl Program {
    /// The uniform called `name`. Its location is only looked up the first time
    pub fn uniform(&self, name: &str) -> Uniform {
        if let Some(&id) = self.locations.borrow().get(name) {
            return Uniform {
                program: self.id,
                id,
            };
        }
        let uniform = Uniform::new(self.id, name);
        self.locations
            .borrow_mut()
            .insert(name.to_string(), uniform.id);
        uniform
    }

    fn from_shaders(shaders: &[Shader]) -> Result<Self, Error> {
        let id = unsafe { gl::CreateProgram() };
        check_gl();

        for shader in shaders {
            unsafe {
                gl::AttachShader(id, shader.id());
            }
            check_gl();
        }

        unsafe {
            gl::LinkProgram(id);
        }
        check_gl();

        let mut success: GLint = 1;
        unsafe {
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
        }
        check_gl();

        if success == 0 {
            // An error occured
            let mut len: GLint = 0;
            unsafe {
                gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut len);
            }
            check_gl();

            let error = create_whitespace_cstring_with_len(len as usize);

            unsafe {
                gl::GetProgramInfoLog(id, len, null_mut(), error.as_ptr() as *mut GLchar);
            }
            check_gl();

            return Err(Error::ShaderLink(error.to_string_lossy().into_owned()));
        }

        for shader in shaders {
            unsafe {
                gl::DetachShader(id, shader.id());
            }
            check_gl();
        }

        Ok(Program {
            id,
            locations: RefCell::new(HashMap::new()),
        })
    }

    /// Draw with this program from now on
    pub(crate) fn bind(&self) {
        bind_program(self.id);
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        if BOUND_PROGRAM.with(Cell::get) == self.id {
            bind_program(0);
        }
        unsafe {
            gl::DeleteProgram(self.id);
        }
    }
}

thread_local! {
    // The program GL is drawing with right now, so binding it again can be skipped
    static BOUND_PROGRAM: Cell<GLuint> = const { Cell::new(0) };
}

fn bind_program(id: GLuint) {
    if BOUND_PROGRAM.with(Cell::get) != id {
        unsafe { gl::UseProgram(id) };
        BOUND_PROGRAM.with(|bound| bound.set(id));
    }
}

fn create_whitespace_cstring_with_len(len: usize) -> CString {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);
    buffer.extend([b' '].iter().cycle().take(len));
    unsafe { CString::from_vec_unchecked(buffer) }
}

/// `source` as a C string, or an error if it has a NUL byte in it
fn shader_source(source: &str, stage: ShaderStage) -> Result<CString, Error> {
    CString::new(source).map_err(|_| Error::ShaderSourceNul { stage })
}

/// Create a program with a vert and frag shader
pub fn create_program(vert_data: &'static str, frag_data: &'static str) -> Result<Program, Error> {
    let vert_shader = Shader::from_source(
        &shader_source(vert_data, ShaderStage::Vertex)?,
        ShaderStage::Vertex,
    )?;

    let frag_shader = Shader::from_source(
        &shader_source(frag_data, ShaderStage::Fragment)?,
        ShaderStage::Fragment,
    )?;

    let shader_program = Program::from_shaders(&[vert_shader, frag_shader])?;

    Ok(shader_program)
}

/// An OpenGL Uniform location
pub struct Uniform {
    program: GLuint,
    id: GLint,
}

impl Uniform {
    // Lookup `name` in `program`. If the program doesn't use it, `id` is -1 and the setters do nothing.
    pub fn new(program: u32, name: &str) -> Self {
        let cname: CString = CString::new(name).expect("uniform name contains a null byte");
        let id: GLint = unsafe { gl::GetUniformLocation(program, cname.as_ptr()) };
        check_gl();
        Uniform { program, id }
    }

    // GL sets uniforms on whichever program is bound, so bind ours first
    pub fn set_mat4(&self, m: &Mat4) {
        bind_program(self.program);
        unsafe { gl::UniformMatrix4fv(self.id, 1, gl::FALSE, m.as_ptr()) }
    }
    pub fn set_vec4(&self, v: &Vec4) {
        bind_program(self.program);
        unsafe { gl::Uniform4f(self.id, v.x, v.y, v.z, v.w) }
    }
    pub fn set_vec3(&self, v: &Vec3) {
        bind_program(self.program);
        unsafe { gl::Uniform3f(self.id, v.x, v.y, v.z) }
    }
    pub fn set_vec2(&self, v: &Vec2) {
        bind_program(self.program);
        unsafe { gl::Uniform2f(self.id, v.x, v.y) }
    }
    pub fn set_f32(&self, x: f32) {
        bind_program(self.program);
        unsafe { gl::Uniform1f(self.id, x) }
    }
    pub fn set_i32(&self, x: i32) {
        bind_program(self.program);
        unsafe { gl::Uniform1i(self.id, x) }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_with_a_nul_byte_is_its_own_error() {
        let error = shader_source("void main() {\0}", ShaderStage::Vertex).unwrap_err();
        assert!(matches!(
            error,
            Error::ShaderSourceNul {
                stage: ShaderStage::Vertex
            }
        ));
        assert!(shader_source("void main() {}", ShaderStage::Fragment).is_ok());
    }
}
