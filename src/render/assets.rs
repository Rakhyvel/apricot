//! The meshes, textures, shader programs, and fonts the renderer holds, and the ids that refer to
//! them

use std::{
    cell::{Ref, RefCell},
    path::Path,
};

use crate::{
    error::Error,
    font::{Atlas, Font, FontRasterizer},
    geometry::aabb::AABB,
    mesh::{MeshData, MeshShape},
    opengl::{create_program, Primitive, Program, RawMesh, Texture},
    registry::{Handle, Registry},
};

/// The font text uses until another is picked
pub(crate) const DEFAULT_FONT: &[u8] = include_bytes!("../fonts/Not Jam Mono Clean 16.ttf");

/// Refers to a mesh added to the renderer
pub type MeshId = Handle<Mesh>;

/// Refers to a texture added to the renderer
pub type TextureId = Handle<Texture>;

/// Refers to a shader program added to the renderer
pub type ProgramId = Handle<Program>;

/// Refers to a font added to the renderer
pub type FontId = Handle<Font>;

/// The assets apricot adds itself. They're named too, so adding your own asset under the same name
/// replaces one, like a shader. The names are "2d", "3d", "3d-solid", "shadow" and "line" for the
/// programs, "white" for the texture, "default" for the font, and "quad", "cube" and "plane" for
/// the meshes
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

/// A mesh uploaded to the GPU, with its shape kept on the CPU for picking and culling
pub struct Mesh {
    raw: RawMesh,
    shape: MeshShape,
}

impl Mesh {
    /// Upload `data` to the GPU. Keeps its shape on the CPU, but the normals and UVs only live on
    /// the GPU
    pub fn from_data(data: MeshData) -> Self {
        // The layout has to match the slots the shaders read
        let raw = RawMesh::with_indices(
            &data.interleaved(),
            &data.indices,
            &MeshData::LAYOUT,
            Primitive::Triangles,
        );
        Mesh {
            raw,
            shape: MeshShape::from(data),
        }
    }

    /// Upload a mesh straight from the bytes of a .obj file
    pub fn from_obj(obj_file_data: &[u8]) -> Result<Self, Error> {
        Ok(Self::from_data(MeshData::from_obj(obj_file_data)?))
    }

    /// The mesh's positions and triangles, for picking and culling
    pub fn shape(&self) -> &MeshShape {
        &self.shape
    }

    pub(crate) fn draw(&self) {
        self.raw.draw();
    }
}

/// Every asset of one kind, behind ids. Looking one up lends it out, which holds the whole store,
/// so adding or removing an asset of the same kind while holding one panics
pub(crate) struct Store<T> {
    registry: RefCell<Registry<T>>,
}

impl<T> Store<T> {
    pub(crate) fn new() -> Self {
        Self {
            registry: RefCell::new(Registry::new()),
        }
    }

    /// Add an asset, optionally under a name. A name that's taken replaces the asset that has it,
    /// and that asset's id keeps working
    pub(crate) fn add(&self, item: T, name: Option<&'static str>) -> Handle<T> {
        let mut registry = self.registry.borrow_mut();
        match name {
            Some(name) => registry.add_or_replace(item, name),
            None => registry.add(item, None),
        }
    }

    /// Take the asset out, giving it back
    pub(crate) fn remove(&self, id: Handle<T>) -> Option<T> {
        self.registry.borrow_mut().remove(id)
    }

    /// Put `item` where `id` points, giving back what was there. `None` if `id` isn't loaded
    pub(crate) fn replace(&self, id: Handle<T>, item: T) -> Option<T> {
        let mut registry = self.registry.borrow_mut();
        registry
            .get_mut(id)
            .map(|slot| std::mem::replace(slot, item))
    }

    /// The asset `id` refers to. `None` if it's been removed
    pub(crate) fn get(&self, id: Handle<T>) -> Option<Ref<'_, T>> {
        Ref::filter_map(self.registry.borrow(), |registry| registry.get(id)).ok()
    }

    /// The asset added under `name`
    pub(crate) fn get_named(&self, name: &str) -> Option<Ref<'_, T>> {
        self.get(self.id_of(name)?)
    }

    /// The id of the asset added under `name`
    pub(crate) fn id_of(&self, name: &str) -> Option<Handle<T>> {
        self.registry.borrow().lookup(name)
    }
}

/// Every asset the renderer holds
pub(crate) struct Assets {
    pub(crate) meshes: Store<Mesh>,
    pub(crate) textures: Store<Texture>,
    pub(crate) programs: Store<Program>,
    pub(crate) fonts: Store<Font>,
    rasterizer: FontRasterizer,
}

impl Assets {
    pub(crate) fn new() -> Result<Self, Error> {
        Ok(Self {
            meshes: Store::new(),
            textures: Store::new(),
            programs: Store::new(),
            fonts: Store::new(),
            rasterizer: FontRasterizer::new()?,
        })
    }

    pub(crate) fn add_mesh_data(&self, data: MeshData, name: Option<&'static str>) -> MeshId {
        self.meshes.add(Mesh::from_data(data), name)
    }

    pub(crate) fn add_mesh_from_obj(
        &self,
        obj_file_data: &[u8],
        name: Option<&'static str>,
    ) -> Result<MeshId, Error> {
        Ok(self.meshes.add(Mesh::from_obj(obj_file_data)?, name))
    }

    pub(crate) fn replace_mesh_data(&self, id: MeshId, data: MeshData) {
        let replaced = self.meshes.replace(id, Mesh::from_data(data));
        assert!(
            replaced.is_some(),
            "replace_mesh_data: {id:?} isn't a loaded mesh"
        );
    }

    pub(crate) fn mesh_aabb(&self, id: MeshId) -> AABB {
        let mesh = self
            .meshes
            .get(id)
            .unwrap_or_else(|| panic!("get_mesh_aabb: {id:?} isn't a loaded mesh"));
        mesh.shape().aabb()
    }

    pub(crate) fn create_texture_rgba(&self, w: u32, h: u32, bytes: &[u8]) -> TextureId {
        let texture = Texture::new();
        texture.set_pixels(w as i32, h as i32, bytes);
        self.textures.add(texture, None)
    }

    pub(crate) fn update_texture_rgba(&self, id: TextureId, w: u32, h: u32, bytes: &[u8]) {
        let texture = self
            .textures
            .get(id)
            .unwrap_or_else(|| panic!("update_texture_rgba: {id:?} isn't a loaded texture"));
        texture.set_pixels(w as i32, h as i32, bytes);
    }

    pub(crate) fn add_texture_from_png(
        &self,
        path: impl AsRef<Path>,
        name: Option<&'static str>,
    ) -> Result<TextureId, Error> {
        Ok(self.textures.add(Texture::from_png(path)?, name))
    }

    pub(crate) fn add_program_from_source(
        &self,
        vert: &'static str,
        frag: &'static str,
        name: Option<&'static str>,
    ) -> Result<ProgramId, Error> {
        Ok(self.programs.add(create_program(vert, frag)?, name))
    }

    pub(crate) fn add_font(
        &self,
        path: impl AsRef<Path>,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        let atlas = self.rasterizer.rasterize_file(path, name, size, style)?;
        Ok(self.add_font_atlas(atlas, name))
    }

    pub(crate) fn add_font_from_bytes(
        &self,
        bytes: &[u8],
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<FontId, Error> {
        let atlas = self.rasterizer.rasterize_bytes(bytes, name, size, style)?;
        Ok(self.add_font_atlas(atlas, name))
    }

    /// Add a finished font under `name`. If that replaces a font, the old font's atlas texture is
    /// freed too
    fn add_font_atlas(&self, atlas: Atlas, name: &'static str) -> FontId {
        let old_atlas = self.fonts.get_named(name).map(|font| font.cache_texture);
        let font = atlas.into_font(|image| self.textures.add(Texture::from_surface(image), None));
        let id = self.fonts.add(font, Some(name));
        if let Some(old_atlas) = old_atlas {
            self.textures.remove(old_atlas);
        }
        id
    }

    /// Free a font and its atlas texture. Gives back whether it was loaded
    pub(crate) fn unload_font(&self, id: FontId) -> bool {
        let Some(font) = self.fonts.remove(id) else {
            return false;
        };
        self.textures.remove(font.cache_texture);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, rc::Rc};

    /// Counts how many of these have been dropped, standing in for a GPU object that gets freed
    struct Tracked(&'static str, Rc<Cell<u32>>);

    impl Drop for Tracked {
        fn drop(&mut self) {
            self.1.set(self.1.get() + 1);
        }
    }

    fn named(store: &Store<String>, text: &str, name: &'static str) -> Handle<String> {
        store.add(text.to_string(), Some(name))
    }

    #[test]
    fn added_assets_are_found_by_id_and_by_name() {
        let store = Store::new();
        let a = named(&store, "apple", "a");
        let unnamed = store.add("banana".to_string(), None);
        assert_eq!(*store.get(a).unwrap(), "apple");
        assert_eq!(*store.get(unnamed).unwrap(), "banana");
        assert_eq!(*store.get_named("a").unwrap(), "apple");
        assert_eq!(store.id_of("a"), Some(a));
        assert!(store.get_named("b").is_none());
        assert!(store.id_of("b").is_none());
        assert_ne!(a, unnamed);
    }

    #[test]
    fn adding_under_a_taken_name_replaces_in_place() {
        let store = Store::new();
        let first = named(&store, "old", "x");
        let other = named(&store, "other", "y");
        let second = named(&store, "new", "x");
        assert_eq!(first, second, "the id keeps working");
        assert_eq!(*store.get(first).unwrap(), "new");
        assert_eq!(*store.get_named("x").unwrap(), "new");
        assert_eq!(*store.get(other).unwrap(), "other");
        assert_eq!(store.registry.borrow().len(), 2);
    }

    #[test]
    fn unnamed_assets_never_replace_each_other() {
        let store = Store::new();
        let a = store.add("a".to_string(), None);
        let b = store.add("b".to_string(), None);
        assert_ne!(a, b);
        assert_eq!(store.registry.borrow().len(), 2);
    }

    #[test]
    fn replacing_drops_the_old_asset_so_its_gpu_object_is_freed() {
        let drops = Rc::new(Cell::new(0));
        let store = Store::new();
        let make = |label| Tracked(label, drops.clone());
        let id = store.add(make("old"), Some("x"));
        assert_eq!(drops.get(), 0);
        assert_eq!(store.add(make("new"), Some("x")), id);
        assert_eq!(drops.get(), 1);
        assert_eq!(store.get(id).unwrap().0, "new");
    }

    #[test]
    fn removing_gives_the_asset_back_and_kills_its_id_and_name() {
        let store = Store::new();
        let id = named(&store, "apple", "a");
        assert_eq!(store.remove(id).as_deref(), Some("apple"));
        assert!(store.get(id).is_none());
        assert!(store.id_of("a").is_none());
        assert_eq!(store.remove(id), None);
    }

    #[test]
    fn replace_swaps_what_an_id_points_at_and_gives_back_the_old_one() {
        let store = Store::new();
        let id = named(&store, "old", "a");
        assert_eq!(store.replace(id, "new".to_string()).as_deref(), Some("old"));
        assert_eq!(*store.get(id).unwrap(), "new");
        assert_eq!(*store.get_named("a").unwrap(), "new");

        store.remove(id);
        assert_eq!(store.replace(id, "late".to_string()), None);
        assert!(store.get(id).is_none(), "a dead id stays dead");
    }
}
