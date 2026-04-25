//! This file contains the core rendering functionality that is shared between 2D and 3D rendering.

use std::{cell::RefCell, collections::HashMap, f32::consts::PI, fmt::Debug};

use nalgebra_glm::{Mat4, Vec3};
use obj::{load_obj, Obj, TexturedVertex};

use crate::{
    opengl::{DeletionQueue, Fbo, Shader},
    tri::Tri,
};

use super::{
    aabb::AABB,
    camera::{Camera, ProjectionKind},
    font::{Font, FontId, FontManager},
    opengl::{Buffer, Program, Texture, Uniform, Vao},
};

pub struct RenderContext {
    // Updated by the user
    pub camera: RefCell<Camera>,
    pub program: RefCell<Option<ProgramId>>,
    pub color: RefCell<nalgebra_glm::Vec4>,
    pub font: RefCell<Option<FontId>>,

    // Managers
    mesh_manager: RefCell<ResourceManager<Mesh, MeshId>>,
    texture_manager: RefCell<ResourceManager<Texture, TextureId>>,
    program_manager: RefCell<ResourceManager<Program, ProgramId>>,
    font_manager: RefCell<FontManager>,

    // Queues
    deletion_queue: RefCell<DeletionQueue>,

    // Updated by the app
    pub int_screen_resolution: nalgebra_glm::I32Vec2,
    pub camera_2d: Camera,
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
}

/// Stores the geometry of a mesh. Meshes are registered in the mesh manager, and can be potentially shared across
/// multiple models.
pub struct Mesh {
    geometry: Vec<GeometryData>,
    indices: Vec<u32>,
    aabb: AABB,
}

/// Actual geometry data for a mesh.
pub struct GeometryData {
    ibo: Buffer<u32>,
    vbo: Buffer<f32>,
    vao: Vao,
    vertex_data: Vec<f32>,
}

pub enum GeometryDataIndex {
    Vertex = 0,
    Normal = 1,
    Texture = 2,
    Color = 3,
}

impl RenderContext {
    pub fn new() -> Self {
        Self {
            camera: RefCell::new(Camera::default()),
            program: RefCell::new(None),
            color: RefCell::new(nalgebra_glm::vec4(0.0, 0.0, 0.0, 1.0)),
            font: RefCell::new(None),

            mesh_manager: RefCell::new(ResourceManager::new()),
            texture_manager: RefCell::new(ResourceManager::new()),
            program_manager: RefCell::new(ResourceManager::new()),
            font_manager: RefCell::new(FontManager::new()),

            deletion_queue: RefCell::new(DeletionQueue::new()),

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
            ),
        }
    }

    pub fn set_camera(&self, camera: Camera) {
        *self.camera.borrow_mut() = camera
    }

    pub fn set_program(&self, name: Option<&'static str>) {
        let manager = self.program_manager.borrow();
        if let Some(name) = name {
            let program_id = manager.get_id_from_name(name).unwrap();
            *self.program.borrow_mut() = Some(program_id);
            let program = manager.get_from_id(program_id).unwrap();
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
        let program = manager.get_from_id(program_id).unwrap();
        program.set();
    }

    pub fn set_color(&self, color: nalgebra_glm::Vec4) {
        let mut color_ref = self.color.borrow_mut();
        *color_ref = color;
    }

    pub fn set_font(&self, font: FontId) {
        *self.font.borrow_mut() = Some(font);
    }

    pub fn get_current_font_id(&self) -> Option<FontId> {
        *self.font.borrow()
    }

    pub fn get_current_font(&self) -> Option<std::cell::Ref<'_, Font>> {
        let id = self.get_current_font_id()?;
        let manager = self.font_manager.borrow();
        std::cell::Ref::filter_map(manager, |m| m.get_font_from_id(id)).ok()
    }

    pub fn add_mesh(&self, mesh: Mesh, name: Option<&'static str>) -> MeshId {
        self.mesh_manager.borrow_mut().add(mesh, name)
    }

    pub fn add_mesh_from_obj(&self, obj_file_data: &[u8], name: Option<&'static str>) -> MeshId {
        self.add_mesh(Mesh::from_obj(obj_file_data), name)
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
    ) -> TextureId {
        self.texture_manager
            .borrow_mut()
            .add(Texture::from_png(texture_filename), name)
    }

    pub fn add_program(&self, program: Program, name: Option<&'static str>) -> ProgramId {
        let retval = self.program_manager.borrow_mut().add(program, name);
        retval
    }

    pub fn add_font(
        &self,
        path: &'static str,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> FontId {
        self.font_manager
            .borrow_mut()
            .add_font(path, name, size, style, self)
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
        let manager = self.mesh_manager.borrow();
        if let Some(_mesh) = manager.get_from_id(id) {
            // Map the Ref<MeshManager> to Ref<Mesh>
            Some(std::cell::Ref::map(manager, |m| m.get_from_id(id).unwrap()))
        } else {
            None
        }
    }

    pub fn get_texture_from_id(&self, id: TextureId) -> Option<std::cell::Ref<'_, Texture>> {
        let manager = self.texture_manager.borrow();
        if let Some(_texture) = manager.get_from_id(id) {
            // Map the Ref<TextureManager> to Ref<Texture>
            Some(std::cell::Ref::map(manager, |m| m.get_from_id(id).unwrap()))
        } else {
            None
        }
    }

    pub fn get_program_from_id(&self, id: ProgramId) -> Option<std::cell::Ref<'_, Program>> {
        let manager = self.program_manager.borrow();
        if let Some(_program) = manager.get_from_id(id) {
            // Map the Ref<ProgramManager> to Ref<Program>
            Some(std::cell::Ref::map(manager, |m| m.get_from_id(id).unwrap()))
        } else {
            None
        }
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
            .unwrap()
            .aabb
    }

    pub fn get_model_aabb(&self, model: &ModelComponent) -> AABB {
        self.mesh_manager
            .borrow()
            .get_from_id(model.mesh_id)
            .unwrap()
            .aabb
            .scale(model.scale)
            .translate(model.position)
    }

    pub fn get_current_program_id(&self) -> u32 {
        if self.program.borrow().is_some() {
            let program = self
                .get_program_from_id(self.program.borrow().unwrap())
                .unwrap();
            program.id()
        } else {
            0
        }
    }

    pub fn get_program_uniform(&self, uniform_name: &str) -> Result<Uniform, &'static str> {
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

    pub fn clear(&self) {
        let color_ref = self.color.borrow_mut();
        unsafe {
            gl::ClearColor(color_ref.x, color_ref.y, color_ref.z, color_ref.w);
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
        let u_model_matrix: Uniform = self.get_program_uniform("u_model_matrix").unwrap();
        let u_view_matrix = self.get_program_uniform("u_view_matrix").unwrap();
        let u_proj_matrix = self.get_program_uniform("u_proj_matrix").unwrap();
        unsafe {
            gl::UniformMatrix4fv(
                u_model_matrix.id,
                1,
                gl::FALSE,
                &model_matrix.columns(0, 4)[0],
            );
            gl::UniformMatrix4fv(
                u_view_matrix.id,
                1,
                gl::FALSE,
                &view_matrix.columns(0, 4)[0],
            );
            gl::UniformMatrix4fv(
                u_proj_matrix.id,
                1,
                gl::FALSE,
                &proj_matrix.columns(0, 4)[0],
            );

            // Setup geometry for rendering
            for i in 0..mesh.geometry.len() {
                mesh.geometry[i].vbo.bind();
                mesh.geometry[i].ibo.bind();
                mesh.geometry[i].vao.enable(i as u32);
            }

            // Make the render call!
            gl::DrawElements(
                gl::TRIANGLES,
                mesh.indices.len() as i32,
                gl::UNSIGNED_INT,
                std::ptr::null(),
            );

            // Unbind all buffers
            for i in 0..mesh.geometry.len() {
                mesh.geometry[i].vbo.unbind();
                mesh.geometry[i].ibo.unbind();
            }
        }
    }

    pub fn draw_line_path_at(
        &self,
        line_path: &LinePathComponent,
        position: Vec3,
        view_matrix: nalgebra_glm::Mat4,
        proj_matrix: nalgebra_glm::Mat4,
    ) {
        self.set_program(Some("line"));
        unsafe {
            gl::Enable(gl::DEPTH_TEST);
            gl::LineWidth(line_path.width);

            // Set uniforms
            let model_matrix = nalgebra_glm::translate(&nalgebra_glm::one(), &position);

            let u_model_matrix = self.get_program_uniform("model").unwrap();
            let u_view_matrix = self.get_program_uniform("view").unwrap();
            let u_proj_matrix = self.get_program_uniform("projection").unwrap();
            let u_color = self.get_program_uniform("u_color").unwrap();
            let u_seam = self.get_program_uniform("u_seam").unwrap();
            let u_num_vertices = self.get_program_uniform("u_num_vertices").unwrap();
            gl::UniformMatrix4fv(
                u_model_matrix.id,
                1,
                gl::FALSE,
                &model_matrix.columns(0, 4)[0],
            );
            gl::UniformMatrix4fv(
                u_view_matrix.id,
                1,
                gl::FALSE,
                &view_matrix.columns(0, 4)[0],
            );
            gl::UniformMatrix4fv(
                u_proj_matrix.id,
                1,
                gl::FALSE,
                &proj_matrix.columns(0, 4)[0],
            );
            gl::Uniform4f(
                u_color.id,
                line_path.color.x,
                line_path.color.y,
                line_path.color.z,
                line_path.color.w,
            );
            gl::Uniform1f(u_seam.id, line_path.seam);
            gl::Uniform1i(u_num_vertices.id, line_path.num_vertices);

            line_path.vertices_buffer.bind();
            line_path.vao.enable(0);
            gl::DrawArrays(gl::LINE_STRIP, 0, line_path.num_vertices);
            line_path.vertices_buffer.unbind();
        }
    }
}

impl Default for RenderContext {
    fn default() -> Self {
        Self::new()
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
            for geom in mesh.geometry.iter_mut() {
                deletion_queue.queue_buffer(&mut geom.ibo);
                deletion_queue.queue_buffer(&mut geom.vbo);
                deletion_queue.queue_vao(&mut geom.vao);
            }
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
        vao.set(0);

        // Generate vertices for the elliptical orbit
        let num_vertices = vertices.len() as i32 / 3; // 3 components per vertex (x,y,z)

        vertices_buffer.unbind();

        Self {
            vao,
            vertices_buffer,
            num_vertices,
            width: 1.0,
            color: nalgebra_glm::vec4(0.7, 0.9, 0.9, 0.9),
            position: nalgebra_glm::vec3(0.0, 0.0, 0.0),
            seam: 0.0,
        }
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
        let geometry: Vec<GeometryData> = datas
            .iter()
            .map(|data| GeometryData {
                ibo: Buffer::<u32>::gen(gl::ELEMENT_ARRAY_BUFFER),
                vao: Vao::gen(),
                vbo: Buffer::<f32>::gen(gl::ARRAY_BUFFER),
                vertex_data: data.to_vec(),
            })
            .collect();

        // Allocate VRAM buffers (this is slow!)
        for i in 0..geometry.len() {
            geometry[i].vbo.set_data(&geometry[i].vertex_data);
            geometry[i].ibo.set_data(&indices);
            geometry[i].vao.set(i as u32)
        }

        // Unbind all buffers
        for i in 0..geometry.len() {
            geometry[i].vbo.unbind();
            geometry[i].ibo.unbind();
        }

        let aabb = AABB::from_points(
            geometry[GeometryDataIndex::Vertex as usize]
                .vertex_data
                .chunks(3)
                .map(|p| nalgebra_glm::vec3(p[0], p[1], p[2])),
        );

        Mesh {
            geometry,
            indices,
            aabb,
        }
    }

    pub fn from_obj(obj_file_data: &[u8]) -> Self {
        let obj: Obj<TexturedVertex> = load_obj(&obj_file_data[..]).unwrap();
        let vb: Vec<TexturedVertex> = obj.vertices;

        let indices = vec_u32_from_vec_u16(&obj.indices);
        let vertices = flatten_positions(&vb);
        let normals = flatten_normals(&vb);
        let uv = flatten_uv(&vb);

        let data = vec![&vertices, &normals, &uv];

        Self::new(indices, data)
    }

    pub fn triangles(&self) -> Vec<Tri> {
        let verts = &self.geometry[GeometryDataIndex::Vertex as usize].vertex_data;
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

    pub fn geometry(&self) -> &Vec<GeometryData> {
        &self.geometry
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }
}

impl GeometryData {
    pub fn vertex_data(&self) -> &Vec<f32> {
        &self.vertex_data
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
