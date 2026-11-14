//! The meshes, textures, programs, and fonts the renderer holds, and adding, getting, and freeing
//! them

use std::{cell::RefCell, path::Path};

use crate::{
    aabb::AABB,
    error::Error,
    font::{Atlas, Font, FontId, FontManager},
    mesh_data::MeshData,
    opengl::{create_program, Program, Texture},
    registry::Registry,
    render_core::{Mesh, MeshId, ProgramId, RenderContext, TextureId},
};

pub(crate) struct Resources {
    meshes: RefCell<Registry<Mesh>>,
    textures: RefCell<Registry<Texture>>,
    programs: RefCell<Registry<Program>>,
    fonts: RefCell<FontManager>,
}

impl Resources {
    pub(crate) fn new(
        meshes: Registry<Mesh>,
        textures: Registry<Texture>,
        programs: Registry<Program>,
        fonts: FontManager,
    ) -> Self {
        Self {
            meshes: RefCell::new(meshes),
            textures: RefCell::new(textures),
            programs: RefCell::new(programs),
            fonts: RefCell::new(fonts),
        }
    }
}

impl RenderContext {
    pub fn add_mesh(&self, mesh: Mesh, name: Option<&'static str>) -> MeshId {
        self.resources.meshes.borrow_mut().add(mesh, name)
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
        self.resources.meshes.borrow_mut().remove(id);
    }

    /// Free a texture. Its id stops working, so anything still drawing it will panic
    pub fn unload_texture(&self, id: TextureId) {
        self.resources.textures.borrow_mut().remove(id);
    }

    /// Free a font. If it was the current font, text goes back to the default font
    pub fn unload_font(&self, id: FontId) {
        let Some(font) = self.resources.fonts.borrow_mut().remove(id) else {
            return;
        };
        self.unload_texture(font.cache_texture);
        if self.get_current_font_id() == id {
            self.set_font(self.builtins.font);
        }
    }

    /// Swap the mesh behind `id` for new data, freeing the old one. Everything drawing `id` draws
    /// the new mesh from then on
    pub fn replace_mesh_data(&self, id: MeshId, data: MeshData) {
        let mut meshes = self.resources.meshes.borrow_mut();
        *meshes.get_mut(id).expect("invalid MeshId") = Mesh::from_data(data);
    }

    pub fn add_texture(&self, texture: Texture, name: Option<&'static str>) -> TextureId {
        self.resources.textures.borrow_mut().add(texture, name)
    }

    /// Load a PNG into a texture
    pub fn add_texture_from_png(
        &self,
        path: impl AsRef<Path>,
        name: Option<&'static str>,
    ) -> Result<TextureId, Error> {
        Ok(self
            .resources
            .textures
            .borrow_mut()
            .add(Texture::from_png(path)?, name))
    }

    pub fn add_program(&self, program: Program, name: Option<&'static str>) -> ProgramId {
        let mut manager = self.resources.programs.borrow_mut();
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
        let atlas = self
            .resources
            .fonts
            .borrow()
            .rasterize_file(path, name, size, style)?;
        Ok(self.add_font_atlas(atlas, name))
    }

    pub fn add_font_from_bytes(
        &self,
        bytes: &[u8],
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        let atlas = self
            .resources
            .fonts
            .borrow()
            .rasterize_bytes(bytes, name, size, style)?;
        Ok(self.add_font_atlas(atlas, name))
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
        std::cell::Ref::filter_map(self.resources.meshes.borrow(), |m| m.get(id)).ok()
    }

    pub fn get_texture_from_id(&self, id: TextureId) -> Option<std::cell::Ref<'_, Texture>> {
        std::cell::Ref::filter_map(self.resources.textures.borrow(), |m| m.get(id)).ok()
    }

    pub fn get_program_from_id(&self, id: ProgramId) -> Option<std::cell::Ref<'_, Program>> {
        std::cell::Ref::filter_map(self.resources.programs.borrow(), |m| m.get(id)).ok()
    }

    pub fn get_font_from_id(&self, id: FontId) -> Option<std::cell::Ref<'_, Font>> {
        let manager = self.resources.fonts.borrow();
        std::cell::Ref::filter_map(manager, |m| m.get_font_from_id(id)).ok()
    }

    pub fn get_mesh_id_from_name(&self, name: &'static str) -> Option<MeshId> {
        self.resources.meshes.borrow().lookup(name)
    }

    pub fn get_texture_id_from_name(&self, name: &'static str) -> Option<TextureId> {
        self.resources.textures.borrow().lookup(name)
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
        self.resources.programs.borrow().lookup(name)
    }

    pub fn get_font_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.resources.fonts.borrow().get_id_from_name(name)
    }

    pub fn get_mesh_aabb(&self, mesh_id: MeshId) -> AABB {
        self.resources
            .meshes
            .borrow()
            .get(mesh_id)
            .expect("invalid MeshId")
            .aabb
    }

    fn add_font_atlas(&self, atlas: Atlas, name: &'static str) -> FontId {
        let font = atlas.into_font(|image| self.add_texture(Texture::from_surface(image), None));
        self.resources.fonts.borrow_mut().add(font, name)
    }
}
