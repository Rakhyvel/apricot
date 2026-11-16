//! This file contains the core rendering functionality that is shared between 2D and 3D rendering.

use std::{cell::Ref, fmt::Debug, path::Path};

use nalgebra_glm::{Mat4, Vec3, Vec4};

use crate::{
    camera::ViewProj,
    error::Error,
    font::Font,
    geometry::aabb::AABB,
    mesh::MeshData,
    opengl::{self, DrawOptions, Primitive, RawMesh},
    render::assets::{Assets, Builtins, FontId, Mesh, MeshId, ProgramId, TextureId},
};

use super::{
    opengl::{Program, Texture, Uniform},
    render2d::Queue2d,
    view::View,
};

pub struct RenderContext {
    pub(crate) assets: Assets,
    pub(crate) view: View,
    pub(crate) queue_2d: Queue2d,
    pub builtins: Builtins,
}

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

impl RenderContext {
    pub fn new() -> Result<Self, Error> {
        let assets = Assets::new()?;
        let builtins = Builtins::new(&assets)?;
        Ok(Self {
            view: View::default(),
            queue_2d: Queue2d::new(builtins.font),
            assets,
            builtins,
        })
    }

    /// The uniform called `name` in `program`. Setting it works any time, whatever's being drawn
    pub fn uniform(&self, program: ProgramId, name: &str) -> Uniform {
        self.get_program_from_id(program)
            .expect("invalid ProgramId")
            .uniform(name)
    }

    fn bind(&self, program: ProgramId) {
        self.get_program_from_id(program)
            .expect("invalid ProgramId")
            .bind();
    }

    pub fn clear(&self, color: Vec4) {
        opengl::clear(color);
    }

    /// Draw `mesh` with `program`, which gets the model, view, and projection matrices
    pub fn draw(
        &self,
        program: ProgramId,
        mesh: &Mesh,
        model_matrix: nalgebra_glm::Mat4,
        view_proj: &ViewProj,
    ) {
        self.uniform(program, "u_model_matrix")
            .set_mat4(&model_matrix);
        self.uniform(program, "u_view_matrix")
            .set_mat4(&view_proj.view);
        self.uniform(program, "u_proj_matrix")
            .set_mat4(&view_proj.proj);

        self.bind(program);
        mesh.draw();
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
        let program = self.builtins.program_3d;
        self.uniform(program, "u_color").set_vec4(&tint);
        self.uniform(program, "shadow_map").set_i32(1);

        match self.light() {
            Some(light) => {
                self.uniform(program, "u_sun_dir")
                    .set_vec3(&light.light_dir);
                self.uniform(program, "light_mvp")
                    .set_mat4(&light.light_proj_view);
                self.uniform(program, "u_shadows")
                    .set_i32(light.shadows as i32);
                self.uniform(program, "u_shadow_texel")
                    .set_f32(light.texel_size);
                if light.shadows {
                    opengl::bind_texture_id(1, light.depth_map);
                }
            }
            None => {
                self.uniform(program, "u_sun_dir").set_vec3(&Vec3::zeros());
                self.uniform(program, "u_shadows").set_i32(0);
            }
        }

        let texture = self
            .get_texture_from_id(texture_id)
            .expect("invalid TextureId");
        texture.activate(0);
        self.uniform(program, "texture0").set_i32(0);

        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(program, &mesh, model, &self.camera_view_proj());
        before.restore();
    }

    /// Draw a line path from the current camera, moved by `transform`
    pub fn draw_line_path(&self, path: &LinePath, transform: Mat4) {
        let style = &path.style;
        let program = self.builtins.program_line;

        let view_proj = self.camera_view_proj();
        self.uniform(program, "u_model_matrix").set_mat4(&transform);
        self.uniform(program, "u_view_matrix")
            .set_mat4(&view_proj.view);
        self.uniform(program, "u_proj_matrix")
            .set_mat4(&view_proj.proj);
        self.uniform(program, "u_color").set_vec4(&style.color);
        self.uniform(program, "u_seam").set_f32(style.seam);
        self.uniform(program, "u_fade").set_i32(style.fade as i32);
        self.uniform(program, "u_num_vertices")
            .set_i32(path.mesh.vertex_count() as i32);

        let options = DrawOptions {
            depth_test: style.depth_test,
            depth_write: false, // lines shouldn't hide what's drawn after them
            line_width: style.width,
            ..Default::default()
        };
        self.draw_raw(&path.mesh, program, &options);
    }

    /// Draw a `RawMesh` with `program` and `options`
    pub fn draw_raw(&self, mesh: &RawMesh, program: ProgramId, options: &DrawOptions) {
        let before = options.apply();
        self.bind(program);
        mesh.draw();
        before.restore();
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

    /// Free a mesh. Its id stops working, so anything still drawing it will panic
    pub fn unload_mesh(&self, id: MeshId) {
        self.assets.meshes.remove(id);
    }

    /// The mesh `id` refers to. `None` if it's been unloaded
    pub fn get_mesh_from_id(&self, id: MeshId) -> Option<Ref<'_, Mesh>> {
        self.assets.meshes.get(id)
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

    /// Free a texture. Its id stops working, so anything still drawing it will panic
    pub fn unload_texture(&self, id: TextureId) {
        self.assets.textures.remove(id);
    }

    /// The texture `id` refers to. `None` if it's been unloaded
    pub fn get_texture_from_id(&self, id: TextureId) -> Option<Ref<'_, Texture>> {
        self.assets.textures.get(id)
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

    /// The program `id` refers to. `None` if it's been unloaded
    pub fn get_program_from_id(&self, id: ProgramId) -> Option<Ref<'_, Program>> {
        self.assets.programs.get(id)
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

    /// Free a font. If it was the current font, text goes back to the default font
    pub fn unload_font(&self, id: FontId) {
        if self.assets.unload_font(id) && self.get_current_font_id() == id {
            self.set_font(self.builtins.font);
        }
    }

    /// The font `id` refers to. `None` if it's been unloaded
    pub fn get_font_from_id(&self, id: FontId) -> Option<Ref<'_, Font>> {
        self.assets.fonts.get(id)
    }

    /// The id of the font added under `name`
    pub fn get_font_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.assets.fonts.id_of(name)
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

fn flatten(points: &[Vec3]) -> Vec<f32> {
    points.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}
