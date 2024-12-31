//! This module contains OpenGL objects.

use std::{
    ffi::{CStr, CString},
    marker::PhantomData,
    path::Path,
    ptr::{null, null_mut},
};

use gl::{
    types::{GLchar, GLenum, GLint, GLuint},
    UseProgram,
};

use image::{EncodableLayout, ImageError};

/// An OpenGL Shader
pub struct Shader {
    id: GLuint,
}

impl Shader {
    /// Compile an OpenGL shader from GLSL souce
    pub fn from_source(source: &CStr, kind: GLenum) -> Result<Self, String> {
        let id = unsafe { gl::CreateShader(kind) };
        print_any_errors();

        unsafe {
            gl::ShaderSource(id, 1, &source.as_ptr(), null());
        }
        print_any_errors();
        unsafe {
            gl::CompileShader(id);
        }
        print_any_errors();

        let mut success: GLint = 1;
        unsafe {
            gl::GetShaderiv(id, gl::COMPILE_STATUS, &mut success);
        }
        print_any_errors();

        if success == 0 {
            // Error occured!
            let mut len: GLint = 0;
            unsafe { gl::GetShaderiv(id, gl::INFO_LOG_LENGTH, &mut len) }
            print_any_errors();

            let error = create_whitespace_cstring_with_len(len as usize);

            unsafe {
                gl::GetShaderInfoLog(id, len, null_mut(), error.as_ptr() as *mut GLchar);
            }
            print_any_errors();

            return Err(error.to_string_lossy().into_owned());
        }

        Ok(Shader { id })
    }

    /// Retrieve the OpenGL ID of this shader
    pub fn id(&self) -> GLuint {
        self.id
    }
}

impl Drop for Shader {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteShader(self.id);
        }
        print_any_errors();
    }
}

// TODO: Rename OpenGlProgram, and rename ProgramId to Program
#[derive(Default)]
/// An OpenGL program
pub struct Program {
    id: GLuint,
}

impl Program {
    fn from_shaders(shaders: &[Shader]) -> Result<Self, String> {
        let id = unsafe { gl::CreateProgram() };
        print_any_errors();

        for shader in shaders {
            unsafe {
                gl::AttachShader(id, shader.id());
            }
            print_any_errors();
        }

        unsafe {
            gl::LinkProgram(id);
        }
        print_any_errors();

        let mut success: GLint = 1;
        unsafe {
            gl::GetProgramiv(id, gl::LINK_STATUS, &mut success);
        }
        print_any_errors();

        if success == 0 {
            // An error occured
            let mut len: GLint = 0;
            unsafe {
                gl::GetProgramiv(id, gl::INFO_LOG_LENGTH, &mut len);
            }
            print_any_errors();

            let error = create_whitespace_cstring_with_len(len as usize);

            unsafe {
                gl::GetProgramInfoLog(id, len, null_mut(), error.as_ptr() as *mut GLchar);
            }
            print_any_errors();

            return Err(error.to_string_lossy().into_owned());
        }

        for shader in shaders {
            unsafe {
                gl::DetachShader(id, shader.id());
            }
            print_any_errors();
        }

        Ok(Program { id })
    }

    /// Tell OpenGL to use this program
    pub fn set(&self) {
        unsafe {
            UseProgram(self.id);
        }
    }

    /// Retrieve the OpenGL ID of this program
    pub fn id(&self) -> GLuint {
        self.id
    }
}

impl Drop for Program {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteProgram(self.id);
        }
        print_any_errors();
    }
}

fn create_whitespace_cstring_with_len(len: usize) -> CString {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);
    buffer.extend([b' '].iter().cycle().take(len));
    unsafe { CString::from_vec_unchecked(buffer) }
}

/// Create a program with a vert and frag shader
pub fn create_program(
    vert_data: &'static str,
    frag_data: &'static str,
) -> Result<Program, &'static str> {
    let vert_shader = Shader::from_source(
        &CString::new(vert_data).unwrap(), // TODO: Load this at runtime
        gl::VERTEX_SHADER,
    )
    .unwrap();
    let frag_shader = Shader::from_source(
        &CString::new(frag_data).unwrap(), // TODO: Load this at runtime
        gl::FRAGMENT_SHADER,
    )
    .unwrap();

    let shader_program = Program::from_shaders(&[vert_shader, frag_shader]).unwrap();

    Ok(shader_program)
}

/// OpenGL Vertex Buffer Object. Contains vertex data given as input to the vertex shader.
pub struct Buffer<T> {
    pub id: GLuint,
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
        print_any_errors();
        Buffer::<T> {
            id,
            target,
            phantom: PhantomData::<T>::default(),
        }
    }

    /// Set the buffer's data
    pub fn set_data(&self, data: &Vec<T>) {
        self.bind();
        unsafe {
            gl::BufferData(
                self.target,
                (data.len() * std::mem::size_of::<T>()) as gl::types::GLsizeiptr,
                data.as_ptr() as *const gl::types::GLvoid,
                gl::STATIC_DRAW,
            );
        }
        print_any_errors();
    }

    /// Bind the buffer in OpenGL
    pub fn bind(&self) {
        unsafe {
            gl::BindBuffer(self.target, self.id);
        }
        print_any_errors();
    }

    fn unbind(&self) {
        unsafe {
            gl::BindBuffer(self.target, 0);
        }
        print_any_errors();
    }

    fn delete(&self) {
        unsafe {
            gl::DeleteBuffers(1, &self.id);
        }
        print_any_errors();
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        self.unbind();
        self.delete();
    }
}

/// OpenGL Vertex Array Object
pub struct Vao {
    pub id: GLuint,
}

impl Vao {
    /// Create a new VAO
    pub fn gen() -> Self {
        let mut id: GLuint = 0;
        unsafe {
            gl::GenVertexArrays(1, &mut id);
        }
        print_any_errors();
        Vao { id }
    }

    /// Bind and use this VAO
    pub fn set(&self, loc: u32) {
        self.bind(loc);
        self.setup(loc);
    }

    /// Enable this VAO
    pub fn enable(&self, loc: u32) {
        unsafe {
            gl::EnableVertexAttribArray(loc);
        }
        print_any_errors();
        self.setup(loc);
    }

    fn bind(&self, loc: u32) {
        unsafe {
            gl::EnableVertexAttribArray(loc);
            gl::BindVertexArray(self.id);
        }
        print_any_errors();
    }

    fn setup(&self, loc: u32) {
        unsafe {
            gl::VertexAttribPointer(
                loc,
                3,
                gl::FLOAT,
                gl::FALSE,
                (3 * std::mem::size_of::<f32>()) as GLint,
                null(),
            );
        }
        print_any_errors();
    }

    fn unbind(&self) {
        unsafe {
            gl::BindVertexArray(0);
        }
        print_any_errors();
    }

    fn delete(&self) {
        unsafe {
            gl::DeleteVertexArrays(1, &self.id);
        }
        print_any_errors();
    }
}

impl Drop for Vao {
    fn drop(&mut self) {
        self.unbind();
        self.delete();
    }
}

/// An OpenGL Uniform location
pub struct Uniform {
    pub id: GLint,
}

impl Uniform {
    // Create a new OpenGL uniform
    pub fn new(program: u32, name: &str) -> Result<Self, &'static str> {
        let cname: CString = CString::new(name).expect("CString::new failed");
        let location: GLint = unsafe { gl::GetUniformLocation(program, cname.as_ptr()) };
        print_any_errors();
        if location == -1 {
            return Err("Couldn't get a uniform location");
        }
        Ok(Uniform { id: location })
    }
}

// TODO: Rename OpenGlTexture, and rename TextureId to Texture
#[derive(Clone)]
/// An OpenGL Texture
pub struct Texture {
    pub id: GLuint,
}

impl Texture {
    /// Create a new blank OpenGL texture
    pub fn new() -> Self {
        let mut id: GLuint = 0;
        unsafe { gl::GenTextures(1, &mut id) }
        print_any_errors();
        Self { id }
    }

    /// Create an OpenGL texture from a PNG
    pub fn from_png(texture_filename: &'static str) -> Self {
        let texture = Texture::new();
        let path = Path::new(texture_filename);
        texture.load(&path).unwrap();
        texture
    }

    /// Create an OpenGL texture from an SDL surface
    pub fn from_surface(surface: sdl2::surface::Surface) -> Self {
        let texture = Texture::new();
        unsafe {
            texture.bind();

            let width = surface.width() as i32;
            let height = surface.height() as i32;
            let format = match surface.pixel_format_enum() {
                sdl2::pixels::PixelFormatEnum::RGB24 => gl::RGB,
                sdl2::pixels::PixelFormatEnum::RGBA32 => gl::RGBA,
                sdl2::pixels::PixelFormatEnum::ARGB8888 => gl::BGRA,
                x => panic!("Lol! {:?}", x),
            };

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width,
                height,
                0,
                format,
                gl::UNSIGNED_BYTE,
                surface.without_lock().unwrap().as_ptr() as *const std::ffi::c_void,
            );
            print_any_errors();

            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            print_any_errors();
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            print_any_errors();
        }
        texture
    }

    /// Bind this texture
    pub fn bind(&self) {
        unsafe { gl::BindTexture(gl::TEXTURE_2D, self.id) }
        print_any_errors();
    }

    /// Load this texture into it's OpenGL slot
    pub fn load(&self, path: &Path) -> Result<(), ImageError> {
        self.bind();

        let img = image::open(path)?.into_rgba8();
        unsafe {
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_EDGE as GLint,
            );
            print_any_errors();
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_EDGE as GLint,
            );
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            print_any_errors();

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                img.width() as i32,
                img.height() as i32,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                img.as_bytes().as_ptr() as *const _,
            );
            print_any_errors();
            gl::GenerateMipmap(gl::TEXTURE_2D);
            print_any_errors();
        }
        Ok(())
    }

    /// Load this texture as a depth buffer
    pub fn load_depth_buffer(&self, width: i32, height: i32) {
        self.bind();

        unsafe {
            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::DEPTH_COMPONENT as GLint,
                width,
                height,
                0,
                gl::DEPTH_COMPONENT,
                gl::FLOAT,
                std::ptr::null(),
            );
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::NEAREST as GLint);
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::NEAREST as GLint);
            print_any_errors();
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_S,
                gl::CLAMP_TO_BORDER as GLint,
            );
            print_any_errors();
            gl::TexParameteri(
                gl::TEXTURE_2D,
                gl::TEXTURE_WRAP_T,
                gl::CLAMP_TO_BORDER as GLint,
            );
            print_any_errors();
            let border_color: [f32; 4] = [1.0, 1.0, 1.0, 1.0];
            gl::TexParameterfv(
                gl::TEXTURE_2D,
                gl::TEXTURE_BORDER_COLOR,
                border_color.as_ptr(),
            );
            print_any_errors();
        }
    }

    /// Method to call after binding
    pub fn post_bind(&self) {
        unsafe {
            gl::FramebufferTexture2D(
                gl::FRAMEBUFFER,
                gl::DEPTH_ATTACHMENT,
                gl::TEXTURE_2D,
                self.id,
                0,
            );
            print_any_errors();
            gl::DrawBuffer(gl::NONE);
            print_any_errors();
            gl::ReadBuffer(gl::NONE);
            print_any_errors();

            if gl::CheckFramebufferStatus(gl::FRAMEBUFFER) != gl::FRAMEBUFFER_COMPLETE {
                panic!("Framebuffer is not complete!");
            }
            print_any_errors();
        };
    }

    /// Activate this texture
    pub fn activate(&self, unit: GLuint) {
        unsafe {
            gl::ActiveTexture(unit);
            self.bind();
        }
        print_any_errors();
    }

    /// Associate this texture with a uniform name
    pub fn associate_uniform(&self, program_id: u32, unit: GLint, uniform_name: &str) {
        unsafe {
            let uniform = CString::new(uniform_name).unwrap();
            gl::Uniform1i(gl::GetUniformLocation(program_id, uniform.as_ptr()), unit)
        }
        print_any_errors();
    }

    /// Retrieve the width and height of this texture
    pub fn get_dimensions(&self) -> Option<(i32, i32)> {
        let mut width: GLint = 0;
        let mut height: GLint = 0;

        unsafe {
            gl::BindTexture(gl::TEXTURE_2D, self.id);
            print_any_errors();
            gl::GetTexLevelParameteriv(gl::TEXTURE_2D, 0, gl::TEXTURE_WIDTH, &mut width);
            print_any_errors();
            gl::GetTexLevelParameteriv(gl::TEXTURE_2D, 0, gl::TEXTURE_HEIGHT, &mut height);
            print_any_errors();
            gl::BindTexture(gl::TEXTURE_2D, 0);
            print_any_errors();
        }

        if width > 0 && height > 0 {
            Some((width, height))
        } else {
            None
        }
    }
}

impl Drop for Texture {
    fn drop(&mut self) {
        unsafe {
            gl::DeleteTextures(1, [self.id].as_ptr());
        }
        print_any_errors();
    }
}
impl Default for Texture {
    fn default() -> Self {
        Self { id: 0 }
    }
}

/// An OpenGL Frame Buffer Object
pub struct Fbo {
    pub id: GLuint,
}

impl Fbo {
    /// Create a new Frame Buffer Object
    pub fn new() -> Self {
        let mut id: GLuint = 0;
        unsafe {
            gl::GenFramebuffers(1, &mut id);
        }
        print_any_errors();
        Self { id }
    }

    /// Bind this FBO
    pub fn bind(&self) {
        unsafe {
            gl::BindFramebuffer(gl::FRAMEBUFFER, self.id);
        }
        print_any_errors();
    }

    /// Unbind this FBO
    pub fn unbind(&self) {
        unsafe { gl::BindFramebuffer(gl::FRAMEBUFFER, 0) }
        print_any_errors();
    }
}

impl Default for Fbo {
    fn default() -> Self {
        Self { id: 0 }
    }
}

fn print_any_errors() {
    if let Some(error_message) = get_last_opengl_error() {
        panic!("OpenGL Error:\n{}", error_message);
    }
}

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
