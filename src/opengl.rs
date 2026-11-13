//! This module contains OpenGL objects.

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

/// An OpenGL Shader
pub struct Shader {
    id: GLuint,
}

impl Shader {
    /// Compile an OpenGL shader from GLSL souce
    pub fn from_source(source: &CStr, kind: GLenum) -> Result<Self, Error> {
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

            return Err(Error::ShaderCompile {
                stage: "idk",
                log: error.to_string_lossy().into_owned(),
            });
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
    }
}

// TODO: Rename OpenGlProgram, and rename ProgramId to Program
#[derive(Default)]
/// An OpenGL program
pub struct Program {
    id: GLuint,
    locations: RefCell<HashMap<String, GLint>>,
}

impl Program {
    /// The uniform called `name`. Its location is only looked up the first time
    pub fn uniform(&self, name: &str) -> Uniform {
        if let Some(&id) = self.locations.borrow().get(name) {
            return Uniform { id };
        }
        let uniform = Uniform::new(self.id, name);
        self.locations
            .borrow_mut()
            .insert(name.to_string(), uniform.id);
        uniform
    }

    fn from_shaders(shaders: &[Shader]) -> Result<Self, Error> {
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

            return Err(Error::ShaderLink(error.to_string_lossy().into_owned()));
        }

        for shader in shaders {
            unsafe {
                gl::DetachShader(id, shader.id());
            }
            print_any_errors();
        }

        Ok(Program {
            id,
            locations: RefCell::new(HashMap::new()),
        })
    }

    /// Tell OpenGL to use this program
    pub fn set(&self) {
        unsafe {
            gl::UseProgram(self.id);
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
    }
}

fn create_whitespace_cstring_with_len(len: usize) -> CString {
    let mut buffer: Vec<u8> = Vec::with_capacity(len + 1);
    buffer.extend([b' '].iter().cycle().take(len));
    unsafe { CString::from_vec_unchecked(buffer) }
}

/// Create a program with a vert and frag shader
pub fn create_program(vert_data: &'static str, frag_data: &'static str) -> Result<Program, Error> {
    let vert_shader = Shader::from_source(
        &CString::new(vert_data).map_err(|e| Error::ShaderCompile {
            stage: "vert",
            log: format!("{e}"),
        })?,
        gl::VERTEX_SHADER,
    )?;

    let frag_shader = Shader::from_source(
        &CString::new(frag_data).map_err(|e| Error::ShaderCompile {
            stage: "frag",
            log: format!("{e}"),
        })?,
        gl::FRAGMENT_SHADER,
    )?;

    let shader_program = Program::from_shaders(&[vert_shader, frag_shader])?;

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
        print_any_errors();
    }

    /// Bind the buffer in OpenGL
    pub fn bind(&self) {
        unsafe {
            gl::BindBuffer(self.target, self.id);
        }
        print_any_errors();
    }

    pub fn unbind(&self) {
        unsafe {
            gl::BindBuffer(self.target, 0);
        }
        print_any_errors();
    }
}

impl<T> Drop for Buffer<T> {
    fn drop(&mut self) {
        if self.id != 0 {
            eprintln!(
                "WARNING: Buffer {} dropped without being queued for deletion!",
                self.id
            );
        }
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

    /// Bind this VAO. All attributes or index state recorded into it becomes active.
    pub fn bind(&self) {
        unsafe {
            gl::BindVertexArray(self.id);
        }
        print_any_errors();
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
        print_any_errors();
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
        print_any_errors();
    }
}

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

    pub fn queue_deletion(&mut self, renderer: &crate::render_core::RenderContext) {
        renderer.queue_vao_deletion(&mut self.vao);
        renderer.queue_buffer_deletion(&mut self.vbo);
        if let Some(ibo) = &mut self.ibo {
            renderer.queue_buffer_deletion(ibo);
        }
    }

    pub(crate) fn queue_into(&mut self, queue: &mut DeletionQueue) {
        queue.queue_vao(&mut self.vao);
        queue.queue_buffer(&mut self.vbo);
        if let Some(ibo) = &mut self.ibo {
            queue.queue_buffer(ibo);
        }
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
        print_any_errors();
        self.vao.unbind();
    }
}

/// GL settings for a draw. Whatever they change gets put back afterward
#[derive(Debug, Clone, Copy)]
pub struct DrawOptions {
    /// Whether things in front hide things behind
    pub depth_test: bool,
    /// Whether this draw hides things drawn after it
    pub depth_write: bool,
    /// Whether triangles facing away aren't drawn
    pub culling: bool,
    /// Whether see-through colors mix with what's behind them
    pub blending: bool,
    /// Whether the vertex shader sets each point's size with `gl_PointSize`
    pub shader_point_size: bool,
    /// How thick lines are, in pixels. Most drivers only support 1
    pub line_width: f32,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            culling: true,
            blending: true,
            shader_point_size: false,
            line_width: 1.0,
        }
    }
}

impl DrawOptions {
    /// Switch GL over to these options, giving back the settings to restore afterward
    pub(crate) fn apply(&self) -> DrawOptions {
        let before = DrawOptions::current();
        self.set();
        before
    }

    /// Put these options back, like after `apply`
    pub(crate) fn restore(&self) {
        self.set();
    }

    fn current() -> DrawOptions {
        let mut depth_write: gl::types::GLboolean = gl::TRUE;
        let mut line_width: f32 = 1.0;
        unsafe {
            gl::GetBooleanv(gl::DEPTH_WRITEMASK, &mut depth_write);
            gl::GetFloatv(gl::LINE_WIDTH, &mut line_width);
            DrawOptions {
                depth_test: gl::IsEnabled(gl::DEPTH_TEST) == gl::TRUE,
                depth_write: depth_write == gl::TRUE,
                culling: gl::IsEnabled(gl::CULL_FACE) == gl::TRUE,
                blending: gl::IsEnabled(gl::BLEND) == gl::TRUE,
                shader_point_size: gl::IsEnabled(gl::PROGRAM_POINT_SIZE) == gl::TRUE,
                line_width,
            }
        }
    }

    fn set(&self) {
        let toggle = |cap: GLenum, on: bool| unsafe {
            if on {
                gl::Enable(cap)
            } else {
                gl::Disable(cap)
            }
        };
        toggle(gl::DEPTH_TEST, self.depth_test);
        toggle(gl::CULL_FACE, self.culling);
        toggle(gl::BLEND, self.blending);
        toggle(gl::PROGRAM_POINT_SIZE, self.shader_point_size);
        unsafe {
            gl::DepthMask(if self.depth_write {
                gl::TRUE
            } else {
                gl::FALSE
            });
            gl::LineWidth(self.line_width);
        }
        print_any_errors();
    }
}

impl Drop for Vao {
    fn drop(&mut self) {
        if self.id != 0 {
            eprintln!(
                "WARNING: Vao {} dropped without being queued for deletion!",
                self.id
            );
        }
    }
}

pub struct DeletionQueue {
    vaos: Vec<GLuint>,
    buffers: Vec<GLuint>,
    textures: Vec<GLuint>,
    programs: Vec<GLuint>,
    shaders: Vec<GLuint>,
    fbos: Vec<GLuint>,
}

impl DeletionQueue {
    pub fn new() -> Self {
        Self {
            vaos: Vec::new(),
            buffers: Vec::new(),
            textures: Vec::new(),
            programs: Vec::new(),
            shaders: Vec::new(),
            fbos: Vec::new(),
        }
    }

    pub fn queue_vao(&mut self, vao: &mut Vao) {
        self.vaos.push(vao.id);
        vao.id = 0;
    }

    pub fn queue_buffer<T>(&mut self, buffer: &mut Buffer<T>) {
        self.buffers.push(buffer.id);
        buffer.id = 0;
    }

    pub fn queue_texture(&mut self, texture: &mut Texture) {
        self.textures.push(texture.id);
        texture.id = 0;
    }

    pub fn queue_fbo(&mut self, fbo: &mut Fbo) {
        self.fbos.push(fbo.id);
        fbo.id = 0;
    }

    pub fn flush(&mut self) {
        unsafe {
            if !self.vaos.is_empty() {
                gl::DeleteVertexArrays(self.vaos.len() as i32, self.vaos.as_ptr());
                self.vaos.clear();
            }
            if !self.buffers.is_empty() {
                gl::DeleteBuffers(self.buffers.len() as i32, self.buffers.as_ptr());
                self.buffers.clear();
            }
            if !self.textures.is_empty() {
                gl::DeleteTextures(self.textures.len() as i32, self.textures.as_ptr());
                self.textures.clear();
            }
            if !self.programs.is_empty() {
                for id in self.programs.drain(..) {
                    gl::DeleteProgram(id);
                }
            }
            if !self.shaders.is_empty() {
                for id in self.shaders.drain(..) {
                    gl::DeleteShader(id);
                }
            }
            if !self.fbos.is_empty() {
                gl::DeleteFramebuffers(self.fbos.len() as i32, self.fbos.as_ptr());
                self.fbos.clear();
            }
        }
    }
}

impl Default for DeletionQueue {
    fn default() -> Self {
        Self::new()
    }
}

/// An OpenGL Uniform location
pub struct Uniform {
    pub id: GLint,
}

impl Uniform {
    // Lookup `name` in `program`. If the program doesn't use it, `id` is -1 and the setters do nothing.
    pub fn new(program: u32, name: &str) -> Self {
        let cname: CString = CString::new(name).expect("uniform name contains a null byte");
        let id: GLint = unsafe { gl::GetUniformLocation(program, cname.as_ptr()) };
        print_any_errors();
        Uniform { id }
    }

    pub fn set_mat4(&self, m: &Mat4) {
        unsafe { gl::UniformMatrix4fv(self.id, 1, gl::FALSE, m.as_ptr()) }
    }
    pub fn set_vec4(&self, v: &Vec4) {
        unsafe { gl::Uniform4f(self.id, v.x, v.y, v.z, v.w) }
    }
    pub fn set_vec3(&self, v: &Vec3) {
        unsafe { gl::Uniform3f(self.id, v.x, v.y, v.z) }
    }
    pub fn set_vec2(&self, v: &Vec2) {
        unsafe { gl::Uniform2f(self.id, v.x, v.y) }
    }
    pub fn set_f32(&self, x: f32) {
        unsafe { gl::Uniform1f(self.id, x) }
    }
    pub fn set_i32(&self, x: i32) {
        unsafe { gl::Uniform1i(self.id, x) }
    }
}

// TODO: Rename OpenGlTexture, and rename TextureId to Texture
/// An OpenGL Texture
#[derive(Default)]
pub struct Texture {
    pub id: GLuint,
    width: Cell<i32>,
    height: Cell<i32>,
}

impl Texture {
    /// Create a new blank OpenGL texture
    pub fn new() -> Self {
        let mut id: GLuint = 0;
        unsafe { gl::GenTextures(1, &mut id) }
        print_any_errors();
        Self {
            id,
            width: Cell::new(0),
            height: Cell::new(0),
        }
    }

    /// Create an OpenGL texture from a PNG
    pub fn from_png(path: impl AsRef<Path>) -> Result<Self, Error> {
        let texture = Texture::new();
        let path = path.as_ref();
        texture.load(path).map_err(|source| Error::Image {
            path: path.into(),
            source,
        })?;
        Ok(texture)
    }

    /// Create an OpenGL texture from an SDL surface
    pub fn from_surface(surface: sdl2::surface::Surface) -> Self {
        // Whatever format SDL gave us, upload it as RGBA
        let surface = surface
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .expect("out of memory converting surface to RGBA");
        let texture = Texture::new();
        unsafe {
            texture.bind();

            let width = surface.width() as i32;
            let height = surface.height() as i32;
            texture.set_size(width, height);

            gl::TexImage2D(
                gl::TEXTURE_2D,
                0,
                gl::RGBA as i32,
                width,
                height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                surface
                    .without_lock()
                    .expect("a converted surface never needs locking")
                    .as_ptr() as *const std::ffi::c_void,
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
            print_any_errors();
            return;
        }

        self.set_size(width, height);
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
                width,
                height,
                0,
                gl::RGBA,
                gl::UNSIGNED_BYTE,
                data.as_ptr() as *const _,
            );
            print_any_errors();
        }
    }

    /// Load this texture into it's OpenGL slot
    pub fn load(&self, path: &Path) -> Result<(), ImageError> {
        self.bind();

        let img = image::open(path)?.into_rgba8();
        self.set_pixels(img.width() as i32, img.height() as i32, img.as_bytes());
        unsafe {
            gl::GenerateMipmap(gl::TEXTURE_2D);
            print_any_errors();
        }
        Ok(())
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
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MIN_FILTER, gl::LINEAR as GLint);
            print_any_errors();
            gl::TexParameteri(gl::TEXTURE_2D, gl::TEXTURE_MAG_FILTER, gl::LINEAR as GLint);
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
            print_any_errors();
        }
    }

    /// Method to call after binding
    pub fn post_bind(&self) -> Result<(), Error> {
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

            let status = gl::CheckFramebufferStatus(gl::FRAMEBUFFER);
            if status != gl::FRAMEBUFFER_COMPLETE {
                return Err(Error::Framebuffer(status));
            }
            print_any_errors();
        };
        Ok(())
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
            let uniform = CString::new(uniform_name).expect("uniform name contains a null byte");
            gl::Uniform1i(gl::GetUniformLocation(program_id, uniform.as_ptr()), unit)
        }
        print_any_errors();
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

impl Drop for Texture {
    fn drop(&mut self) {
        if self.id != 0 {
            eprintln!(
                "WARNING: Texture {} dropped without being queued for deletion!",
                self.id
            );
        }
    }
}

/// An OpenGL Frame Buffer Object
#[derive(Default)]
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

impl Drop for Fbo {
    fn drop(&mut self) {
        if self.id != 0 {
            eprintln!(
                "WARNING: Fbo {} dropped without being queued for deletion!",
                self.id
            );
        }
    }
}

fn print_any_errors() {
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
