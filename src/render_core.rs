//! This file contains the core rendering functionality that is shared between 2D and 3D rendering.

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    fmt::Debug,
    path::Path,
};

use nalgebra_glm::{vec2, vec4, Mat4, Vec2, Vec3, Vec4};

use crate::{
    error::Error,
    frustum::Frustum,
    mesh_data::MeshData,
    opengl::{self, create_program, DrawOptions, Primitive, RawMesh},
    registry::{Handle, Registry},
    shadow_map::ActiveLight,
    tri::Tri,
};

use super::{
    aabb::AABB,
    camera::{Camera, ProjectionKind},
    font::{Font, FontId, FontManager},
    opengl::{Program, Texture, Uniform},
    ray::Ray,
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
        /// Half-size in pixels along the mesh's X and Y. For a polygon both are just its radius
        scale: Vec2,
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
    /// Nested clip rects. The top one clips both drawing and `App::mouse_over`
    pub(crate) clip_stack: RefCell<Vec<Rectangle>>,

    // Managers
    mesh_manager: RefCell<Registry<Mesh>>,
    texture_manager: RefCell<Registry<Texture>>,
    program_manager: RefCell<Registry<Program>>,
    font_manager: RefCell<FontManager>,
    pub(crate) poly_meshes: RefCell<HashMap<(u32, u32), MeshId>>,

    // Queues
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

/// Refers to a mesh added to the renderer
pub type MeshId = Handle<Mesh>;

/// Refers to a texture added to the renderer
pub type TextureId = Handle<Texture>;

/// Refers to a shader program added to the renderer
pub type ProgramId = Handle<Program>;

/// How a `LinePath` looks
#[derive(Clone, Copy, Debug)]
pub struct LineStyle {
    /// Thickness in pixels. Most drivers only support 1
    pub width: f32,
    pub color: Vec4,
    /// Whether the line fades out behind the seam, like a trail
    pub fade: bool,
    /// Where along the path the fade starts, from 0 (the first point) to 1 (the last)
    pub seam: f32,
    /// Whether 3D geometry in front of the line hides it
    pub depth_test: bool,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: nalgebra_glm::vec4(0.7, 0.9, 0.9, 0.9),
            fade: true,
            seam: 0.0,
            depth_test: true,
        }
    }
}

/// A 3D line through a list of points
pub struct LinePath {
    mesh: RawMesh,
    pub style: LineStyle,
}

/// Stores the geometry of a mesh. Meshes are registered in the mesh manager, and can be potentially shared across
/// multiple models.
pub struct Mesh {
    raw: RawMesh,
    positions: Vec<f32>,
    indices: Vec<u32>,
    aabb: AABB,
}

impl RenderContext {
    pub fn new() -> Result<Self, Error> {
        fn builtin(vert: &'static str, frag: &'static str) -> Program {
            create_program(vert, frag).expect("apricot built-in shader failed to compile")
        }

        let mut programs = Registry::new();
        let mut textures = Registry::new();
        let mut meshes = Registry::new();

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
            font: FontId::placeholder(0),
            quad: meshes.add(Mesh::from_data(MeshData::quad()), Some("quad")),
            cube: meshes.add(Mesh::from_data(MeshData::cube()), Some("cube")),
            plane: meshes.add(Mesh::from_data(MeshData::plane()), Some("plane")),
        };

        let mut renderer = Self {
            camera: RefCell::new(Camera::default()),
            program: RefCell::new(None),
            font: Cell::new(FontId::placeholder(0)), // replaced by the default font below
            clip_stack: RefCell::new(Vec::new()),

            mesh_manager: RefCell::new(meshes),
            texture_manager: RefCell::new(textures),
            program_manager: RefCell::new(programs),
            font_manager: RefCell::new(FontManager::new()?),
            poly_meshes: RefCell::new(HashMap::new()),

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

    /// The window's aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        let res = self.int_screen_resolution;
        if res.y == 0 {
            1.0
        } else {
            res.x as f32 / res.y as f32
        }
    }

    /// The current camera's view and projection matrices, fitted to the window
    pub fn camera_view_proj(&self) -> (Mat4, Mat4) {
        self.camera.borrow().view_proj_matrices(self.aspect_ratio())
    }

    /// The current camera's view frustum, fitted to the window
    pub fn camera_frustum(&self) -> Frustum {
        self.camera.borrow().frustum(self.aspect_ratio())
    }

    /// Roughly how many pixels across the radius of a sphere at `center` looks on screen
    pub fn projected_radius_px(&self, center: Vec3, radius: f32) -> f32 {
        let height = self.int_screen_resolution.y as f32;
        self.camera
            .borrow()
            .projected_radius_px(center, radius, height)
    }

    /// Where a world-space point lands on screen, in pixels. `None` if it's behind the camera
    pub fn world_to_screen(&self, p: Vec3) -> Option<Vec2> {
        let (view, proj) = self.camera_view_proj();
        let clip = proj * view * vec4(p.x, p.y, p.z, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.xyz() / clip.w;
        let res = self.int_screen_resolution;
        Some(vec2(
            (ndc.x + 1.0) / 2.0 * res.x as f32,
            (1.0 - ndc.y) / 2.0 * res.y as f32,
        ))
    }

    /// A ray from the camera through a pixel on screen
    pub fn screen_ray(&self, pos: Vec2) -> Ray {
        let res = self.int_screen_resolution;
        self.camera
            .borrow()
            .get_ray(pos.x, pos.y, res.x as f32, res.y as f32)
    }

    pub fn set_program(&self, name: Option<&'static str>) {
        let manager = self.program_manager.borrow();
        if let Some(name) = name {
            let program_id = manager
                .lookup(name)
                .unwrap_or_else(|| panic!("no program named {name:?}"));
            *self.program.borrow_mut() = Some(program_id);
            let program = manager.get(program_id).expect("invalid ProgramId");
            program.set();
        } else {
            *self.program.borrow_mut() = None;
            Program::unbind();
        }
    }

    pub fn set_program_from_id(&self, program_id: ProgramId) {
        let manager = self.program_manager.borrow();
        *self.program.borrow_mut() = Some(program_id);
        let program = manager.get(program_id).expect("invalid ProgramId");
        program.set();
    }

    pub fn set_font(&self, font: FontId) {
        self.font.set(font);
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

    /// Upload `data` to the GPU and add it as a mesh
    pub fn add_mesh_data(&self, data: MeshData, name: Option<&'static str>) -> MeshId {
        self.add_mesh(Mesh::from_data(data), name)
    }

    /// Free a mesh. Its id stops working, so anything still drawing it will panic
    pub fn unload_mesh(&self, id: MeshId) {
        self.mesh_manager.borrow_mut().remove(id);
    }

    /// Free a texture. Its id stops working, so anything still drawing it will panic
    pub fn unload_texture(&self, id: TextureId) {
        self.texture_manager.borrow_mut().remove(id);
    }

    /// Free a font. If it was the current font, text goes back to the default font
    pub fn unload_font(&self, id: FontId) {
        let Some(font) = self.font_manager.borrow_mut().remove(id) else {
            return;
        };
        self.unload_texture(font.cache_texture);
        if self.font.get() == id {
            self.font.set(self.builtins.font);
        }
    }

    /// Swap the mesh behind `id` for new data, freeing the old one. Everything drawing `id` draws
    /// the new mesh from then on
    pub fn replace_mesh_data(&self, id: MeshId, data: MeshData) {
        let mut meshes = self.mesh_manager.borrow_mut();
        *meshes.get_mut(id).expect("invalid MeshId") = Mesh::from_data(data);
    }

    pub fn add_texture(&self, texture: Texture, name: Option<&'static str>) -> TextureId {
        self.texture_manager.borrow_mut().add(texture, name)
    }

    /// Load a PNG into a texture
    pub fn add_texture_from_png(
        &self,
        path: impl AsRef<Path>,
        name: Option<&'static str>,
    ) -> Result<TextureId, Error> {
        Ok(self
            .texture_manager
            .borrow_mut()
            .add(Texture::from_png(path)?, name))
    }

    pub fn add_program(&self, program: Program, name: Option<&'static str>) -> ProgramId {
        let mut manager = self.program_manager.borrow_mut();
        match name {
            Some(name) => manager.add_or_replace(program, name),
            None => manager.add(program, None),
        }
    }

    /// Compile a vertex and fragment shader into a program, and name it
    pub fn add_program_from_source(
        &self,
        vert: &'static str,
        frag: &'static str,
        name: Option<&'static str>,
    ) -> Result<ProgramId, Error> {
        Ok(self.add_program(create_program(vert, frag)?, name))
    }

    /// Load a .ttf or .otf font
    pub fn add_font(
        &self,
        path: impl AsRef<Path>,
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
        std::cell::Ref::filter_map(self.mesh_manager.borrow(), |m| m.get(id)).ok()
    }

    pub fn get_texture_from_id(&self, id: TextureId) -> Option<std::cell::Ref<'_, Texture>> {
        std::cell::Ref::filter_map(self.texture_manager.borrow(), |m| m.get(id)).ok()
    }

    pub fn get_program_from_id(&self, id: ProgramId) -> Option<std::cell::Ref<'_, Program>> {
        std::cell::Ref::filter_map(self.program_manager.borrow(), |m| m.get(id)).ok()
    }

    pub fn get_font_from_id(&self, id: FontId) -> Option<std::cell::Ref<'_, Font>> {
        let manager = self.font_manager.borrow();
        std::cell::Ref::filter_map(manager, |m| m.get_font_from_id(id)).ok()
    }

    pub fn get_mesh_id_from_name(&self, name: &'static str) -> Option<MeshId> {
        self.mesh_manager.borrow().lookup(name)
    }

    pub fn get_texture_id_from_name(&self, name: &'static str) -> Option<TextureId> {
        self.texture_manager.borrow().lookup(name)
    }

    /// Make a texture from tightly packed RGBA bytes, `w * h * 4` of them
    pub fn create_texture_rgba(&self, w: u32, h: u32, bytes: &[u8]) -> TextureId {
        let texture = Texture::new();
        texture.set_pixels(w as i32, h as i32, bytes);
        self.add_texture(texture, None)
    }

    /// Replace a texture's pixels. Cheap to call every frame as long as the size stays the same
    pub fn update_texture_rgba(&self, id: TextureId, w: u32, h: u32, bytes: &[u8]) {
        self.get_texture_from_id(id)
            .expect("invalid TextureId")
            .set_pixels(w as i32, h as i32, bytes);
    }

    pub fn get_program_id_from_name(&self, name: &'static str) -> Option<ProgramId> {
        self.program_manager.borrow().lookup(name)
    }

    pub fn get_font_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.font_manager.borrow().get_id_from_name(name)
    }

    pub fn get_mesh_aabb(&self, mesh_id: MeshId) -> AABB {
        self.mesh_manager
            .borrow()
            .get(mesh_id)
            .expect("invalid MeshId")
            .aabb
    }

    /// The uniform called `uniform_name` in the current program. Does nothing when set if there's
    /// no current program, or it has no such uniform
    pub fn get_program_uniform(&self, uniform_name: &str) -> Uniform {
        match self
            .program
            .borrow()
            .and_then(|id| self.get_program_from_id(id))
        {
            Some(program) => program.uniform(uniform_name),
            None => Uniform { id: -1 },
        }
    }

    pub fn clear(&self, color: Vec4) {
        opengl::clear(color);
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

        mesh.raw.draw();
    }

    /// Draw `mesh_id` with the built-in 3D shader from the current camera, in a solid `color`
    pub fn draw_mesh(&self, mesh_id: MeshId, model: Mat4, color: Vec4) {
        self.draw_mesh_textured(mesh_id, self.builtins.white_texture, model, color);
    }

    /// Draw `mesh_id` with a texture, tinted by `tint`. A white tint leaves the texture as is
    pub fn draw_mesh_textured(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
    ) {
        self.draw_mesh_textured_with(mesh_id, texture_id, model, tint, &DrawOptions::default());
    }

    /// Draw `mesh_id` with a texture and custom GL `options`
    pub(crate) fn draw_mesh_textured_with(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
        options: &DrawOptions,
    ) {
        let before = options.apply();
        self.set_program_from_id(self.builtins.program_3d);
        self.get_program_uniform("u_color").set_vec4(&tint);
        self.get_program_uniform("shadow_map").set_i32(1);

        match self.active_light.get() {
            Some(light) => {
                self.get_program_uniform("u_sun_dir")
                    .set_vec3(&light.light_dir);
                self.get_program_uniform("light_mvp")
                    .set_mat4(&light.light_proj_view);
                self.get_program_uniform("u_shadows").set_i32(1);
                self.get_program_uniform("u_shadow_texel")
                    .set_f32(light.texel_size);
                opengl::bind_texture_id(1, light.depth_map);
            }
            None => {
                self.get_program_uniform("u_sun_dir")
                    .set_vec3(&Vec3::zeros());
                self.get_program_uniform("u_shadows").set_i32(0);
            }
        }

        let texture = self
            .get_texture_from_id(texture_id)
            .expect("invalid TextureId");
        texture.activate(0);
        self.get_program_uniform("texture0").set_i32(0);

        let (view, proj) = self.camera_view_proj();
        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(&mesh, model, view, proj);
        before.restore();
    }

    /// Draw a line path from the current camera, moved by `transform`
    pub fn draw_line_path(&self, path: &LinePath, transform: Mat4) {
        let style = &path.style;
        self.set_program_from_id(self.builtins.program_line);

        let (view, proj) = self.camera_view_proj();
        self.get_program_uniform("u_model_matrix")
            .set_mat4(&transform);
        self.get_program_uniform("u_view_matrix").set_mat4(&view);
        self.get_program_uniform("u_proj_matrix").set_mat4(&proj);
        self.get_program_uniform("u_color").set_vec4(&style.color);
        self.get_program_uniform("u_seam").set_f32(style.seam);
        self.get_program_uniform("u_fade")
            .set_i32(style.fade as i32);
        self.get_program_uniform("u_num_vertices")
            .set_i32(path.mesh.vertex_count() as i32);

        let options = DrawOptions {
            depth_test: style.depth_test,
            depth_write: false, // lines shouldn't hide what's drawn after them
            line_width: style.width,
            ..Default::default()
        };
        self.draw_raw(&path.mesh, &options);
    }

    /// Draw a `RawMesh` with the current program and `options`
    pub fn draw_raw(&self, mesh: &RawMesh, options: &DrawOptions) {
        let before = options.apply();
        mesh.draw();
        before.restore();
    }
}

impl LinePath {
    /// A line through `points`, in order
    pub fn new(points: &[Vec3], style: LineStyle) -> Self {
        Self {
            mesh: RawMesh::new(&flatten(points), &[3], Primitive::LineStrip),
            style,
        }
    }

    /// Replace the points the line goes through. Cheaper than making a new `LinePath`
    pub fn set_points(&mut self, points: &[Vec3]) {
        self.mesh.set_vertices(&flatten(points));
    }
}

impl Mesh {
    /// Upload `data` to the GPU. Keeps the positions and indices on the CPU for bounding boxes and
    /// raycasts, but the normals and UVs only live on the GPU
    pub fn from_data(data: MeshData) -> Self {
        // Slots match the shaders: 0 = position, 1 = normal, 2 = uv
        let raw = RawMesh::with_indices(
            &data.interleaved(),
            &data.indices,
            &[3, 3, 3],
            Primitive::Triangles,
        );

        let aabb = AABB::from_points(
            data.positions
                .chunks(3)
                .map(|p| nalgebra_glm::vec3(p[0], p[1], p[2])),
        );

        Mesh {
            raw,
            positions: data.positions,
            indices: data.indices,
            aabb,
        }
    }

    /// Upload a mesh straight from the bytes of a .obj file
    pub fn from_obj(obj_file_data: &[u8]) -> Result<Self, Error> {
        Ok(Self::from_data(MeshData::from_obj(obj_file_data)?))
    }

    /// Every triangle, in the mesh's own space
    pub fn triangles(&self) -> impl Iterator<Item = Tri> + '_ {
        crate::mesh_data::triangles(&self.positions, &self.indices)
    }

    pub fn positions(&self) -> &[f32] {
        &self.positions
    }

    pub fn indices(&self) -> &Vec<u32> {
        &self.indices
    }
}

fn flatten(points: &[Vec3]) -> Vec<f32> {
    points.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}
