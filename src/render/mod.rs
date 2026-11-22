//! Rendering. `RenderContext` is what scenes draw through, and it holds everything drawn with

pub mod assets;
pub mod canvas;
pub mod draw3d;
pub mod lighting;
pub mod lines;
pub mod target;
pub mod text;

use std::{
    cell::{Cell, Ref},
    path::Path,
};

use nalgebra_glm::{I32Vec2, Vec2, Vec3};

use crate::{
    camera::{Camera, ViewProj, Viewport},
    error::Error,
    geometry::{aabb::AABB, frustum::Frustum, ray::Ray},
    mesh::MeshData,
    opengl::{Program, Texture},
};
use assets::{Assets, Builtins, FontId, Mesh, MeshId, ProgramId, TextureId};
use canvas::Queue2d;
use lighting::Lighting;
use text::Font;

/// What scenes draw through. The app makes one, and scenes reach it as `app.renderer`
pub struct RenderContext {
    pub(crate) assets: Assets,
    camera: Cell<Camera>,
    screen_size: Cell<I32Vec2>,
    pub(crate) lighting: Lighting,
    pub(crate) queue_2d: Queue2d,
    pub builtins: Builtins,
}

impl RenderContext {
    pub(crate) fn new() -> Result<Self, Error> {
        let assets = Assets::new()?;
        let builtins = Builtins::new(&assets)?;
        Ok(Self {
            camera: Cell::default(),
            screen_size: Cell::default(),
            lighting: Lighting::new(),
            queue_2d: Queue2d::new(builtins.font),
            assets,
            builtins,
        })
    }
}

/// The camera, and the window it looks through
impl RenderContext {
    pub fn set_camera(&self, camera: Camera) {
        self.camera.set(camera);
    }

    /// A copy of the current camera
    pub fn camera(&self) -> Camera {
        self.camera.get()
    }

    /// The window's size, in pixels
    pub fn screen_size(&self) -> I32Vec2 {
        self.screen_size.get()
    }

    pub(crate) fn set_screen_size(&self, size: I32Vec2) {
        self.screen_size.set(size);
    }

    /// The window as a viewport, for converting between the world and the screen
    pub fn viewport(&self) -> Viewport {
        let size = self.screen_size();
        Viewport::new(size.x as f32, size.y as f32)
    }

    /// The window's aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        self.viewport().aspect_ratio()
    }

    /// The current camera's view and projection matrices, fitted to the window
    pub fn camera_view_proj(&self) -> ViewProj {
        self.camera().view_proj(self.aspect_ratio())
    }

    /// The current camera's view frustum, fitted to the window
    pub fn camera_frustum(&self) -> Frustum {
        self.camera().frustum(self.aspect_ratio())
    }

    /// Roughly how many pixels across the radius of a sphere at `center` looks on screen
    pub fn projected_radius_px(&self, center: Vec3, radius: f32) -> f32 {
        self.camera()
            .projected_radius_px(center, radius, self.viewport())
    }

    /// Where a world-space point lands on screen, in pixels. `None` if it's behind the camera
    pub fn world_to_screen(&self, p: Vec3) -> Option<Vec2> {
        self.camera().world_to_screen(p, self.viewport())
    }

    /// The ray from the camera through `pos` on screen, in pixels from the top left
    pub fn screen_ray(&self, pos: Vec2) -> Ray {
        self.camera().screen_ray(pos, self.viewport())
    }
}

/// Adding, getting, and freeing meshes, textures, shader programs, and fonts. Adding one under a
/// name that's taken replaces the one that has it, and its id keeps working
impl RenderContext {
    /// Add a mesh, optionally under a name. A name that's taken replaces that mesh
    pub fn add_mesh(&self, mesh: Mesh, name: Option<&'static str>) -> MeshId {
        self.assets.meshes.add(mesh, name)
    }

    /// Load a mesh from the bytes of a .obj file
    pub fn add_mesh_from_obj(
        &self,
        obj_file_data: &[u8],
        name: Option<&'static str>,
    ) -> Result<MeshId, Error> {
        self.assets.add_mesh_from_obj(obj_file_data, name)
    }

    /// Upload `data` to the GPU and add it as a mesh
    pub fn add_mesh_data(&self, data: MeshData, name: Option<&'static str>) -> MeshId {
        self.assets.add_mesh_data(data, name)
    }

    /// Swap the mesh behind `id` for new data, freeing the old one. Everything drawing `id` draws
    /// the new mesh from then on
    pub fn replace_mesh_data(&self, id: MeshId, data: MeshData) {
        self.assets.replace_mesh_data(id, data);
    }

    /// Free a mesh. Its id stops working, so anything still drawing it will panic. Panics if
    /// it was already unloaded
    #[track_caller]
    pub fn unload_mesh(&self, id: MeshId) {
        self.assets.meshes.take(id, "unload_mesh");
    }

    /// The mesh `id` refers to. Panics if it's been unloaded
    #[track_caller]
    pub fn get_mesh_from_id(&self, id: MeshId) -> Ref<'_, Mesh> {
        self.assets.meshes.require(id, "get_mesh_from_id")
    }

    /// The mesh added under `name`
    pub fn get_mesh(&self, name: &'static str) -> Option<Ref<'_, Mesh>> {
        self.assets.meshes.get_named(name)
    }

    /// The id of the mesh added under `name`
    pub fn get_mesh_id_from_name(&self, name: &'static str) -> Option<MeshId> {
        self.assets.meshes.id_of(name)
    }

    /// The box around a mesh, in its own space
    pub fn get_mesh_aabb(&self, mesh_id: MeshId) -> AABB {
        self.assets.mesh_aabb(mesh_id)
    }

    /// Add a texture, optionally under a name. A name that's taken replaces that texture
    pub fn add_texture(&self, texture: Texture, name: Option<&'static str>) -> TextureId {
        self.assets.textures.add(texture, name)
    }

    /// Load a PNG into a texture
    pub fn add_texture_from_png(
        &self,
        path: impl AsRef<Path>,
        name: Option<&'static str>,
    ) -> Result<TextureId, Error> {
        self.assets.add_texture_from_png(path, name)
    }

    /// Make a texture from tightly packed RGBA bytes, `w * h * 4` of them
    pub fn create_texture_rgba(&self, w: u32, h: u32, bytes: &[u8]) -> TextureId {
        self.assets.create_texture_rgba(w, h, bytes)
    }

    /// Replace a texture's pixels. Cheap to call every frame as long as the size stays the same
    pub fn update_texture_rgba(&self, id: TextureId, w: u32, h: u32, bytes: &[u8]) {
        self.assets.update_texture_rgba(id, w, h, bytes);
    }

    /// Free a texture. Its id stops working, so anything still drawing it will panic. Panics if
    /// it was already unloaded
    #[track_caller]
    pub fn unload_texture(&self, id: TextureId) {
        self.assets.textures.take(id, "unload_texture");
    }

    /// The texture `id` refers to. Panics if it's been unloaded
    #[track_caller]
    pub fn get_texture_from_id(&self, id: TextureId) -> Ref<'_, Texture> {
        self.assets.textures.require(id, "get_texture_from_id")
    }

    /// The texture added under `name`
    pub fn get_texture(&self, name: &'static str) -> Option<Ref<'_, Texture>> {
        self.assets.textures.get_named(name)
    }

    /// The id of the texture added under `name`
    pub fn get_texture_id_from_name(&self, name: &'static str) -> Option<TextureId> {
        self.assets.textures.id_of(name)
    }

    /// Add a shader program, optionally under a name. A name that's taken replaces that program,
    /// which is how to swap in your own version of a built-in shader
    pub fn add_program(&self, program: Program, name: Option<&'static str>) -> ProgramId {
        self.assets.programs.add(program, name)
    }

    /// Compile a vertex and fragment shader into a program, and name it
    pub fn add_program_from_source(
        &self,
        vert: &'static str,
        frag: &'static str,
        name: Option<&'static str>,
    ) -> Result<ProgramId, Error> {
        self.assets.add_program_from_source(vert, frag, name)
    }

    /// The program `id` refers to. Panics if it's been unloaded
    #[track_caller]
    pub fn get_program_from_id(&self, id: ProgramId) -> Ref<'_, Program> {
        self.assets.programs.require(id, "get_program_from_id")
    }

    /// The program added under `name`
    pub fn get_program(&self, name: &'static str) -> Option<Ref<'_, Program>> {
        self.assets.programs.get_named(name)
    }

    /// The id of the program added under `name`
    pub fn get_program_id_from_name(&self, name: &'static str) -> Option<ProgramId> {
        self.assets.programs.id_of(name)
    }

    /// Load a .ttf or .otf font. A name that's taken replaces that font
    pub fn add_font(
        &self,
        path: impl AsRef<Path>,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        self.assets.add_font(path, name, size, style)
    }

    /// Load a .ttf or .otf font from its bytes. A name that's taken replaces that font
    pub fn add_font_from_bytes(
        &self,
        bytes: &[u8],
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        self.assets.add_font_from_bytes(bytes, name, size, style)
    }

    /// Free a font. If it was the current font, text goes back to the default font. Panics if it
    /// was already unloaded
    #[track_caller]
    pub fn unload_font(&self, id: FontId) {
        self.assets.unload_font(id);
    }

    /// The font `id` refers to. Panics if it's been unloaded
    #[track_caller]
    pub fn get_font_from_id(&self, id: FontId) -> Ref<'_, Font> {
        self.assets.fonts.require(id, "get_font_from_id")
    }

    /// The id of the font added under `name`
    pub fn get_font_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.assets.fonts.id_of(name)
    }
}
