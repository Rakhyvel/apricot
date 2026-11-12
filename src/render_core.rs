//! This file contains the core rendering functionality that is shared between 2D and 3D rendering.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    f32::consts::PI,
    fmt::Debug,
};

use nalgebra_glm::{Mat4, Vec2, Vec3, Vec4};
use obj::{load_obj, Obj, TexturedVertex};

use crate::{
    error::Error,
    mesh_data::MeshData,
    opengl::{create_program, DeletionQueue, Fbo},
    shadow_map::ActiveLight,
    tri::Tri,
};

use super::{
    aabb::AABB,
    camera::{Camera, ProjectionKind},
    font::{Font, FontId, FontManager},
    opengl::{Buffer, Program, Texture, Uniform, Vao},
    rectangle::Rectangle,
};

const DEFAULT_FONT: &[u8] = include_bytes!("fonts/Not Jam Mono Clean 16.ttf");

pub(crate) enum DrawCommand {
    FillRect {
        rect: Rectangle,
        color: Vec4,
        scissor: Option<Rectangle>,
    },
    FillPolygon {
        center: Vec2,
        radius: f32,
        mesh_id: MeshId,
        rotation: f32,
        color: Vec4,
        scissor: Option<Rectangle>,
    },
    CopyTexture {
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: Vec4,
        scissor: Option<Rectangle>,
    },
}

pub struct RenderContext {
    // Updated by the user
    pub camera: RefCell<Camera>,
    pub program: RefCell<Option<ProgramId>>,
    pub font: Cell<FontId>,
    pub scissor: Cell<Option<Rectangle>>,

    // Managers
    mesh_manager: RefCell<ResourceManager<Mesh, MeshId>>,
    texture_manager: RefCell<ResourceManager<Texture, TextureId>>,
    program_manager: RefCell<ResourceManager<Program, ProgramId>>,
    font_manager: RefCell<FontManager>,
    pub(crate) poly_meshes: RefCell<HashMap<(u32, u32), MeshId>>,

    // Queues
    deletion_queue: RefCell<DeletionQueue>,
    pub(crate) draw_queue: RefCell<Vec<(i32, DrawCommand)>>,
    pub(crate) current_draw_layer: Cell<i32>,

    // Updated by the app
    pub(crate) active_light: Cell<Option<ActiveLight>>,
    pub int_screen_resolution: nalgebra_glm::I32Vec2,
    pub camera_2d: Camera,
    pub builtins: Builtins,
}

pub struct Builtins {
    pub program_2d: ProgramId,
    pub program_3d: ProgramId,
    pub program_3d_solid: ProgramId,
    pub program_shadow: ProgramId,
    pub program_line: ProgramId,
    pub white_texture: TextureId,
    pub font: FontId,
    pub quad: MeshId,
    pub cube: MeshId,
    pub plane: MeshId,
}

struct ResourceManager<Resource, Id: OpaqueId> {
    resources: Vec<Resource>,
    keys: HashMap<&'static str, Id>,
}

pub trait OpaqueId: Copy {
    fn new(id: usize) -> Self;
    fn as_usize(&self) -> usize;
}

/// Opaque type used by the mesh manager to associate meshes.
#[derive(Copy, Clone, Debug)]
pub struct MeshId(usize);

/// Opaque type used by the texture manager to associate textures.
#[derive(Copy, Clone, Debug)]
pub struct TextureId(usize);

/// Opaque type used by the program manager to associate programs.
#[derive(Copy, Clone, Debug)]
pub struct ProgramId(usize);

/// An actual model, with geometry, a position, scale, rotation, and texture.
pub struct ModelComponent {
    pub mesh_id: MeshId,
    pub texture_id: TextureId,
    position: nalgebra_glm::Vec3,
    scale: nalgebra_glm::Vec3,
    model_matrix: nalgebra_glm::Mat4,
    pub shown: bool,
    pub outlined: bool,
}

pub struct LinePathComponent {
    pub vao: Vao,
    pub vertices_buffer: Buffer<f32>,
    num_vertices: i32,

    pub width: f32,
    pub color: nalgebra_glm::Vec4,
    pub position: nalgebra_glm::Vec3,

    pub seam: f32,
    pub fade: bool,
    pub depth_test: bool,
}

/// Stores the geometry of a mesh. Meshes are registered in the mesh manager, and can be potentially shared across
/// multiple models.
pub struct Mesh {
    vao: Vao,
    vbos: Vec<Buffer<f32>>,
    ibo: Buffer<u32>,
    positions: Vec<f32>,
    indices: Vec<u32>,
    aabb: AABB,
}

impl RenderContext {
    pub fn new() -> Result<Self, Error> {
        fn builtin(vert: &'static str, frag: &'static str) -> Program {
            create_program(vert, frag).expect("apricot built-in shader failed to compile")
        }

        let mut programs = ResourceManager::new();
        let mut textures = ResourceManager::new();
        let mut meshes = ResourceManager::new();

        let builtins = Builtins {
            program_2d: programs.add(
                builtin(
                    include_str!("shaders/2d.vert"),
                    include_str!("shaders/2d.frag"),
                ),
                Some("2d"),
            ),
            program_3d: programs.add(
                builtin(
                    include_str!("shaders/3d.vert"),
                    include_str!("shaders/3d.frag"),
                ),
                Some("3d"),
            ),
            program_3d_solid: programs.add(
                builtin(
                    include_str!("shaders/3d.vert"),
                    include_str!("shaders/solid-color.frag"),
                ),
                Some("3d-solid"),
            ),
            program_shadow: programs.add(
                builtin(
                    include_str!("shaders/shadow.vert"),
                    include_str!("shaders/shadow.frag"),
                ),
                Some("shadow"),
            ),
            program_line: programs.add(
                builtin(
                    include_str!("shaders/line.vert"),
                    include_str!("shaders/line.frag"),
                ),
                Some("line"),
            ),
            white_texture: {
                let white = Texture::new();
                white.set_pixels(1, 1, &[255; 4]);
                textures.add(white, Some("white"))
            },
            font: FontId::new(0),
            quad: meshes.add(Mesh::from_data(&MeshData::quad()), Some("quad")),
            cube: meshes.add(Mesh::from_data(&MeshData::cube()), Some("cube")),
            plane: meshes.add(Mesh::from_data(&MeshData::plane()), Some("plane")),
        };

        let mut renderer = Self {
            camera: RefCell::new(Camera::default()),
            program: RefCell::new(None),
            font: Cell::new(FontId::new(0)), // replaced by the default font below
            scissor: Cell::new(None),

            mesh_manager: RefCell::new(meshes),
            texture_manager: RefCell::new(textures),
            program_manager: RefCell::new(programs),
            font_manager: RefCell::new(FontManager::new()?),
            poly_meshes: RefCell::new(HashMap::new()),

            deletion_queue: RefCell::new(DeletionQueue::new()),
            draw_queue: RefCell::new(Vec::new()),
            current_draw_layer: Cell::new(0),

            active_light: Cell::new(None),
            int_screen_resolution: nalgebra_glm::I32Vec2::new(0, 0),
            camera_2d: Camera::new(
                nalgebra_glm::vec3(0.0, 0.0, 0.0),
                nalgebra_glm::vec3(0.0, 0.0, 1.0),
                nalgebra_glm::vec3(0.0, 1.0, 0.0),
                ProjectionKind::Orthographic {
                    left: -1.0,
                    right: 1.0,
                    bottom: -1.0,
                    top: 1.0,
                    near: 0.1,
                    far: 10.0,
                },
                4.0 / 3.0,
            ),
            builtins,
        };

        renderer.builtins.font = renderer.add_font_from_bytes(
            DEFAULT_FONT,
            "default",
            16,
            sdl2::ttf::FontStyle::NORMAL,
        )?;
        renderer.set_font(renderer.builtins.font);
        Ok(renderer)
    }

    pub fn set_camera(&self, camera: Camera) {
        *self.camera.borrow_mut() = camera
    }

    pub fn set_program(&self, name: Option<&'static str>) {
        let manager = self.program_manager.borrow();
        if let Some(name) = name {
            let program_id = manager
                .get_id_from_name(name)
                .unwrap_or_else(|| panic!("no program named {name:?}"));
            *self.program.borrow_mut() = Some(program_id);
            let program = manager.get_from_id(program_id).expect("invalid ProgramId");
            program.set();
        } else {
            *self.program.borrow_mut() = None;
            unsafe {
                gl::UseProgram(0);
            }
        }
    }

    pub fn set_program_from_id(&self, program_id: ProgramId) {
        let manager = self.program_manager.borrow();
        *self.program.borrow_mut() = Some(program_id);
        let program = manager.get_from_id(program_id).expect("invalid ProgramId");
        program.set();
    }

    pub fn set_font(&self, font: FontId) {
        self.font.set(font);
    }

    pub fn set_scissor(&self, scissor: Option<Rectangle>) {
        self.scissor.set(scissor)
    }

    pub fn get_current_font_id(&self) -> FontId {
        self.font.get()
    }

    pub fn get_current_font(&self) -> std::cell::Ref<'_, Font> {
        self.get_font_from_id(self.font.get())
            .expect("invalid FontId")
    }

    pub fn add_mesh(&self, mesh: Mesh, name: Option<&'static str>) -> MeshId {
        self.mesh_manager.borrow_mut().add(mesh, name)
    }

    pub fn add_mesh_from_obj(
        &self,
        obj_file_data: &[u8],
        name: Option<&'static str>,
    ) -> Result<MeshId, Error> {
        Ok(self.add_mesh(Mesh::from_obj(obj_file_data)?, name))
    }

    pub fn add_mesh_from_verts(
        &self,
        indices: Vec<u32>,
        datas: Vec<&Vec<f32>>,
        name: Option<&'static str>,
    ) -> MeshId {
        self.add_mesh(Mesh::new(indices, datas), name)
    }

    pub fn add_texture(&self, texture: Texture, name: Option<&'static str>) -> TextureId {
        self.texture_manager.borrow_mut().add(texture, name)
    }

    pub fn add_texture_from_png(
        &self,
        texture_filename: &'static str,
        name: Option<&'static str>,
    ) -> Result<TextureId, Error> {
        Ok(self
            .texture_manager
            .borrow_mut()
            .add(Texture::from_png(texture_filename)?, name))
    }

    pub fn add_program(&self, program: Program, name: Option<&'static str>) -> ProgramId {
        let mut manager = self.program_manager.borrow_mut();
        match name {
            Some(name) => manager.add_or_replace(program, name),
            None => manager.add(program, None),
        }
    }

    pub fn add_font(
        &self,
        path: &'static str,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        self.font_manager
            .borrow_mut()
            .add_font(path, name, size, style, self)
    }

    pub fn add_font_from_bytes(
        &self,
        bytes: &[u8],
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        self.font_manager
            .borrow_mut()
            .add_font_from_bytes(bytes, name, size, style, self)
    }

    pub fn get_mesh(&self, name: &'static str) -> Option<std::cell::Ref<'_, Mesh>> {
        if let Some(id) = self.get_mesh_id_from_name(name) {
            self.get_mesh_from_id(id)
        } else {
            None
        }
    }

    pub fn get_texture(&self, name: &'static str) -> Option<std::cell::Ref<'_, Texture>> {
        if let Some(id) = self.get_texture_id_from_name(name) {
            self.get_texture_from_id(id)
        } else {
            None
        }
    }

    pub fn get_program(&self, name: &'static str) -> Option<std::cell::Ref<'_, Program>> {
        if let Some(id) = self.get_program_id_from_name(name) {
            self.get_program_from_id(id)
        } else {
            None
        }
    }

    pub fn get_mesh_from_id(&self, id: MeshId) -> Option<std::cell::Ref<'_, Mesh>> {
        std::cell::Ref::filter_map(self.mesh_manager.borrow(), |m| m.get_from_id(id)).ok()
    }

    pub fn get_texture_from_id(&self, id: TextureId) -> Option<std::cell::Ref<'_, Texture>> {
        std::cell::Ref::filter_map(self.texture_manager.borrow(), |m| m.get_from_id(id)).ok()
    }

    pub fn get_program_from_id(&self, id: ProgramId) -> Option<std::cell::Ref<'_, Program>> {
        std::cell::Ref::filter_map(self.program_manager.borrow(), |m| m.get_from_id(id)).ok()
    }

    pub fn get_font_from_id(&self, id: FontId) -> Option<std::cell::Ref<'_, Font>> {
        let manager = self.font_manager.borrow();
        std::cell::Ref::filter_map(manager, |m| m.get_font_from_id(id)).ok()
    }

    pub fn get_mesh_id_from_name(&self, name: &'static str) -> Option<MeshId> {
        self.mesh_manager.borrow().get_id_from_name(name)
    }

    pub fn get_texture_id_from_name(&self, name: &'static str) -> Option<TextureId> {
        self.texture_manager.borrow().get_id_from_name(name)
    }

    pub fn create_texture_rgba(&self, w: u32, h: u32, bytes: &[u8]) -> TextureId {
        debug_assert_eq!(
            bytes.len(),
            (w * h * 4) as usize,
            "rgba buffer size mismatch"
        );
        let texture = Texture::new();
        texture.set_pixels(w as i32, h as i32, bytes);
        self.add_texture(texture, None)
    }

    pub fn update_texture_rgba(&self, id: TextureId, w: u32, h: u32, bytes: &[u8]) {
        debug_assert_eq!(
            bytes.len(),
            (w * h * 4) as usize,
            "rgba buffer size mismatch"
        );
        if let Some(texture) = self.get_texture_from_id(id) {
            texture.set_pixels(w as i32, h as i32, bytes);
        }
    }

    pub fn get_program_id_from_name(&self, name: &'static str) -> Option<ProgramId> {
        self.program_manager.borrow().get_id_from_name(name)
    }

    pub fn get_font_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.font_manager.borrow().get_id_from_name(name)
    }

    pub fn get_mesh_aabb(&self, mesh_id: MeshId) -> AABB {
        self.mesh_manager
            .borrow()
            .get_from_id(mesh_id)
            .expect("invalid MeshId")
            .aabb
    }

    pub fn get_model_aabb(&self, model: &ModelComponent) -> AABB {
        self.mesh_manager
            .borrow()
            .get_from_id(model.mesh_id)
            .expect("invalid MeshId")
            .aabb
            .scale(model.scale)
            .translate(model.position)
    }

    pub fn get_current_program_id(&self) -> u32 {
        match *self.program.borrow() {
            Some(id) => self
                .get_program_from_id(id)
                .expect("invalid ProgramId")
                .id(),
            None => 0,
        }
    }

    pub fn get_program_uniform(&self, uniform_name: &str) -> Uniform {
        Uniform::new(self.get_current_program_id(), uniform_name)
    }

    pub fn queue_vao_deletion(&self, vao: &mut Vao) {
        self.deletion_queue.borrow_mut().queue_vao(vao);
    }

    pub fn queue_buffer_deletion<T>(&self, buffer: &mut Buffer<T>) {
        self.deletion_queue.borrow_mut().queue_buffer(buffer);
    }

    pub fn queue_texture_deletion(&self, texture: &mut Texture) {
        self.deletion_queue.borrow_mut().queue_texture(texture);
    }

    pub fn queue_fbo_deletion(&self, fbo: &mut Fbo) {
        self.deletion_queue.borrow_mut().queue_fbo(fbo);
    }

    pub fn flush_deletion_queue(&self) {
        self.deletion_queue.borrow_mut().flush();
    }

    pub fn clear(&self, color: Vec4) {
        unsafe {
            gl::ClearColor(color.x, color.y, color.z, color.w);
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }
    }

    pub fn draw(
        &self,
        mesh: &Mesh,
        model_matrix: nalgebra_glm::Mat4,
        view_matrix: nalgebra_glm::Mat4,
        proj_matrix: nalgebra_glm::Mat4,
    ) {
        self.get_program_uniform("u_model_matrix")
            .set_mat4(&model_matrix);
        self.get_program_uniform("u_view_matrix")
            .set_mat4(&view_matrix);
        self.get_program_uniform("u_proj_matrix")
            .set_mat4(&proj_matrix);

        unsafe {
            // Setup geometry for rendering
            mesh.vao.bind();

            // Make the render call!
            gl::DrawElements(
                gl::TRIANGLES,
                mesh.indices.len() as i32,
                gl::UNSIGNED_INT,
                std::ptr::null(),
            );

            // Unbind the VAO
            mesh.vao.unbind();
        }
    }

    /// Draw `mesh_id` with the built-in 3D shader from the current camera, tinted `color`.
    pub fn draw_mesh(&self, mesh_id: MeshId, model: Mat4, color: Vec4) {
        unsafe {
            gl::Enable(gl::DEPTH_TEST); // the 2D pass turns this off
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
        }
        self.set_program_from_id(self.builtins.program_3d);
        self.get_program_uniform("u_color").set_vec4(&color);
        self.get_program_uniform("shadow_map").set_i32(1);

        match self.active_light.get() {
            Some(light) => {
                self.get_program_uniform("u_sun_dir")
                    .set_vec3(&light.light_dir);
                self.get_program_uniform("light_mvp")
                    .set_mat4(&light.light_proj_view);
                self.get_program_uniform("u_shadows").set_i32(1);
                self.get_program_uniform("shadow_map").set_i32(1); // texture unit 1
                self.get_program_uniform("u_shadow_texel")
                    .set_f32(light.texel_size);
                unsafe {
                    gl::ActiveTexture(gl::TEXTURE1);
                    gl::BindTexture(gl::TEXTURE_2D, light.depth_map);
                    gl::ActiveTexture(gl::TEXTURE0);
                }
            }
            None => {
                self.get_program_uniform("u_sun_dir")
                    .set_vec3(&Vec3::zeros());
                self.get_program_uniform("u_shadows").set_i32(0);
            }
        }

        let white = self
            .get_texture_from_id(self.builtins.white_texture)
            .expect("builtin white texture missing");
        white.activate(gl::TEXTURE0);
        white.associate_uniform(self.get_current_program_id(), 0, "texture0");

        let (view, proj) = self.camera.borrow().view_proj_matrices();
        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(&mesh, model, view, proj);
    }

    pub fn draw_line_path_at(
        &self,
        line_path: &LinePathComponent,
        position: Vec3,
        view_matrix: nalgebra_glm::Mat4,
        proj_matrix: nalgebra_glm::Mat4,
    ) {
        self.set_program_from_id(self.builtins.program_line);
        unsafe {
            if line_path.depth_test {
                gl::Enable(gl::DEPTH_TEST);
                gl::DepthFunc(gl::LEQUAL);
            } else {
                gl::Disable(gl::DEPTH_TEST);
            }
            gl::DepthMask(gl::FALSE);
            gl::LineWidth(line_path.width);
        }

        // Set uniforms
        let model_matrix = nalgebra_glm::translate(&nalgebra_glm::one(), &position);
        self.get_program_uniform("u_model_matrix")
            .set_mat4(&model_matrix);
        self.get_program_uniform("u_view_matrix")
            .set_mat4(&view_matrix);
        self.get_program_uniform("u_proj_matrix")
            .set_mat4(&proj_matrix);
        self.get_program_uniform("u_color")
            .set_vec4(&line_path.color);
        self.get_program_uniform("u_seam").set_f32(line_path.seam);
        self.get_program_uniform("u_fade")
            .set_i32(line_path.fade as i32);
        self.get_program_uniform("u_num_vertices")
            .set_i32(line_path.num_vertices);

        unsafe {
            line_path.vao.bind();
            gl::DrawArrays(gl::LINE_STRIP, 0, line_path.num_vertices);
            line_path.vao.unbind();

            gl::DepthMask(gl::TRUE);
            gl::DepthFunc(gl::LESS);
        }
    }
}

impl Drop for RenderContext {
    fn drop(&mut self) {
        let mut deletion_queue = self.deletion_queue.borrow_mut();

        self.mesh_manager
            .borrow_mut()
            .queue_all(&mut deletion_queue);
        self.texture_manager
            .borrow_mut()
            .queue_all(&mut deletion_queue);
        deletion_queue.flush();
    }
}

impl<Resource, Id: OpaqueId + Debug> ResourceManager<Resource, Id> {
    pub fn new() -> Self {
        Self {
            resources: vec![],
            keys: HashMap::new(),
        }
    }

    pub fn add(&mut self, res: Resource, name: Option<&'static str>) -> Id {
        let id = Id::new(self.resources.len());
        self.resources.push(res);
        if let Some(name) = name {
            self.keys.insert(name, id);
        }
        id
    }

    pub fn add_or_replace(&mut self, res: Resource, name: &'static str) -> Id {
        match self.keys.get(name).copied() {
            Some(id) => {
                self.resources[id.as_usize()] = res;
                id
            }
            None => self.add(res, Some(name)),
        }
    }

    pub fn get_from_id(&self, id: Id) -> Option<&Resource> {
        self.resources.get(id.as_usize())
    }

    pub fn get_id_from_name(&self, name: &'static str) -> Option<Id> {
        self.keys.get(name).copied()
    }
}

impl<Resource, Id: OpaqueId> Default for ResourceManager<Resource, Id> {
    fn default() -> Self {
        Self {
            resources: vec![],
            keys: HashMap::new(),
        }
    }
}

impl ResourceManager<Mesh, MeshId> {
    pub fn queue_all(&mut self, deletion_queue: &mut DeletionQueue) {
        for mesh in self.resources.iter_mut() {
            for vbo in mesh.vbos.iter_mut() {
                deletion_queue.queue_buffer(vbo);
            }
            deletion_queue.queue_buffer(&mut mesh.ibo);
            deletion_queue.queue_vao(&mut mesh.vao);
        }
    }
}

impl ResourceManager<Texture, TextureId> {
    pub fn queue_all(&mut self, deletion_queue: &mut DeletionQueue) {
        for texture in self.resources.iter_mut() {
            deletion_queue.queue_texture(texture);
        }
    }
}

impl OpaqueId for MeshId {
    fn new(id: usize) -> Self {
        MeshId(id)
    }

    fn as_usize(&self) -> usize {
        self.0
    }
}

impl OpaqueId for TextureId {
    fn new(id: usize) -> Self {
        TextureId(id)
    }

    fn as_usize(&self) -> usize {
        self.0
    }
}

impl OpaqueId for ProgramId {
    fn new(id: usize) -> Self {
        ProgramId(id)
    }

    fn as_usize(&self) -> usize {
        self.0
    }
}

impl ModelComponent {
    pub fn new(
        mesh_id: MeshId,
        texture_id: TextureId,
        position: nalgebra_glm::Vec3,
        scale: nalgebra_glm::Vec3,
    ) -> Self {
        Self {
            mesh_id,
            texture_id,
            position,
            scale,
            model_matrix: Self::construct_model_matrix(&position, &scale),
            shown: true,
            outlined: false,
        }
    }

    pub fn set_position(&mut self, position: nalgebra_glm::Vec3) {
        self.position = position;
        self.regen_model_matrix();
    }

    pub fn get_position(&self) -> nalgebra_glm::Vec3 {
        self.position
    }

    pub fn set_scale(&mut self, scale: nalgebra_glm::Vec3) {
        self.scale = scale;
        self.regen_model_matrix();
    }

    pub fn get_scale(&self) -> nalgebra_glm::Vec3 {
        self.scale
    }

    pub fn get_model_matrix(&self) -> nalgebra_glm::Mat4 {
        self.model_matrix
    }

    pub fn get_model_matrix_with_position(&self, position: Vec3) -> Mat4 {
        nalgebra_glm::scale(
            &nalgebra_glm::translate(&nalgebra_glm::one(), &position),
            &self.scale,
        )
    }

    fn regen_model_matrix(&mut self) {
        self.model_matrix = Self::construct_model_matrix(&self.position, &self.scale);
    }

    fn construct_model_matrix(
        position: &nalgebra_glm::Vec3,
        scale: &nalgebra_glm::Vec3,
    ) -> nalgebra_glm::Mat4 {
        nalgebra_glm::scale(
            &nalgebra_glm::translate(&nalgebra_glm::one(), position),
            scale,
        )
    }
}

impl LinePathComponent {
    pub fn new(vertices: Vec<f32>) -> Self {
        let vao = Vao::gen();
        let vertices_buffer: Buffer<f32> = Buffer::gen(gl::ARRAY_BUFFER);
        vertices_buffer.set_data(&vertices);

        vao.attrib(0, &vertices_buffer, 3, 3, 0);

        vao.unbind();
        vertices_buffer.unbind();

        // Generate vertices for the elliptical orbit
        let num_vertices = vertices.len() as i32 / 3; // 3 components per vertex (x,y,z)

        Self {
            vao,
            vertices_buffer,
            num_vertices,
            width: 1.0,
            color: nalgebra_glm::vec4(0.7, 0.9, 0.9, 0.9),
            position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            seam: 0.0,
            fade: true,
            depth_test: true,
        }
    }

    pub fn queue_deletion(&mut self, renderer: &RenderContext) {
        renderer.queue_vao_deletion(&mut self.vao);
        renderer.queue_buffer_deletion(&mut self.vertices_buffer);
    }

    pub fn from_orbit(semi_major_axis: f32, eccentricity: f32, segments: i32) -> Self {
        let vertices = Self::generate_orbit_vertices(semi_major_axis, eccentricity, segments);
        Self::new(vertices)
    }

    fn generate_orbit_vertices(semi_major_axis: f32, eccentricity: f32, segments: i32) -> Vec<f32> {
        let mut vertices = Vec::with_capacity((segments as usize) * 3);
        let semi_minor_axis = semi_major_axis * (1.0 - eccentricity * eccentricity).sqrt();

        // Ensure we generate the complete loop
        for i in 0..=segments {
            // Note: changed to 0..=segments to ensure closure
            let angle = (i as f32 * 2.0 * PI) / segments as f32;

            // Parametric equations for ellipse
            let x = semi_major_axis * angle.cos();
            let y = semi_minor_axis * angle.sin();
            let z = 0.0;

            vertices.push(x);
            vertices.push(y);
            vertices.push(z);
        }

        vertices
    }
}

impl Mesh {
    pub fn new(indices: Vec<u32>, datas: Vec<&Vec<f32>>) -> Self {
        assert!(
            !datas.is_empty(),
            "The `datas` parameter vec can't be empty"
        );

        let vao = Vao::gen();
        let ibo = Buffer::<u32>::gen(gl::ELEMENT_ARRAY_BUFFER);
        ibo.set_data(&indices);

        let vbos: Vec<Buffer<f32>> = datas
            .iter()
            .enumerate()
            .map(|(i, data)| {
                let vbo = Buffer::<f32>::gen(gl::ARRAY_BUFFER);
                vbo.set_data(data); // binds ARRAY_BUFFER to `vbo`
                vao.attrib(i as u32, &vbo, 3, 3, 0); // slot `i` reads from vbo
                vbo
            })
            .collect();

        vao.set_index_buffer(&ibo);

        // Unbind the VAO and any left over array buffer
        vao.unbind();
        unsafe {
            gl::BindBuffer(gl::ARRAY_BUFFER, 0);
        }

        let positions = datas[0].clone();
        let aabb = AABB::from_points(
            positions
                .chunks(3)
                .map(|p| nalgebra_glm::vec3(p[0], p[1], p[2])),
        );

        Mesh {
            vao,
            vbos,
            ibo,
            positions,
            indices,
            aabb,
        }
    }

    pub fn from_obj(obj_file_data: &[u8]) -> Result<Self, Error> {
        let obj: Obj<TexturedVertex> = load_obj(obj_file_data)?;
        let vb: Vec<TexturedVertex> = obj.vertices;

        let indices = vec_u32_from_vec_u16(&obj.indices);
        let vertices = flatten_positions(&vb);
        let normals = flatten_normals(&vb);
        let uv = flatten_uv(&vb);

        let data = vec![&vertices, &normals, &uv];

        Ok(Self::new(indices, data))
    }

    pub fn from_data(data: &MeshData) -> Self {
        Self::new(
            data.indices.clone(),
            vec![&data.positions, &data.normals, &data.uvs],
        )
    }

    pub fn triangles(&self) -> Vec<Tri> {
        let verts = &self.positions;
        self.indices
            .chunks(3)
            .map(|tri| {
                let (i0, i1, i2) = (tri[0] as usize, tri[1] as usize, tri[2] as usize);
                Tri::new(
                    nalgebra_glm::vec3(verts[i0 * 3], verts[i0 * 3 + 1], verts[i0 * 3 + 2]),
                    nalgebra_glm::vec3(verts[i1 * 3], verts[i1 * 3 + 1], verts[i1 * 3 + 2]),
                    nalgebra_glm::vec3(verts[i2 * 3], verts[i2 * 3 + 1], verts[i2 * 3 + 2]),
                )
            })
            .collect()
    }

    pub fn positions(&self) -> &[f32] {
        &self.positions
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }
}

fn flatten_positions(vertices: &Vec<TexturedVertex>) -> Vec<f32> {
    let mut retval = vec![];
    for vertex in vertices {
        retval.push(vertex.position[0]);
        retval.push(vertex.position[1]);
        retval.push(vertex.position[2]);
    }
    retval
}

fn flatten_normals(vertices: &Vec<TexturedVertex>) -> Vec<f32> {
    let mut retval = vec![];
    for vertex in vertices {
        retval.push(vertex.normal[0]);
        retval.push(vertex.normal[1]);
        retval.push(vertex.normal[2]);
    }
    retval
}

fn flatten_uv(vertices: &Vec<TexturedVertex>) -> Vec<f32> {
    let mut retval = vec![];
    for vertex in vertices {
        retval.push(vertex.texture[0]);
        retval.push(vertex.texture[1]);
        retval.push(vertex.texture[2]);
    }
    retval
}

fn vec_u32_from_vec_u16(input: &Vec<u16>) -> Vec<u32> {
    let mut retval = vec![];
    for x in input {
        retval.push(*x as u32);
    }
    retval
}
