//! A set of 3D models with frustum culling, keyed with custom ids

use std::{collections::HashMap, hash::Hash};

use nalgebra_glm::{Mat4, Vec4};

use crate::{
    geometry::aabb::AABB,
    geometry::bvh::{NodeId, BVH},
    geometry::ray::Ray,
    render::assets::{MeshId, TextureId},
    render::lighting::ShadowPass,
    render::RenderContext,
};

/// A mesh with a texture, placed in the world
#[derive(Clone, Copy, Debug)]
pub struct Model {
    pub mesh: MeshId,
    pub texture: TextureId,
    /// Position, rotation, and scale
    pub transform: Mat4,
    /// Multiplied with the texture
    pub tint: Vec4,
    /// Whether `ModelSet::draw_outlines` draws an outline around it
    pub outlined: bool,
    /// Whether `ModelSet::draw_shadows` draws it, so it casts a shadow
    pub casts_shadow: bool,
}

impl Model {
    /// A model at `transform` with a white tint and no outline, that casts a shadow
    pub fn new(mesh: MeshId, texture: TextureId, transform: Mat4) -> Self {
        Self {
            mesh,
            texture,
            transform,
            tint: Vec4::repeat(1.0),
            outlined: false,
            casts_shadow: true,
        }
    }
}

struct Entry {
    model: Model,
    node: NodeId,
    /// The box around the mesh, in its own space
    mesh_aabb: AABB,
    /// The box around the model, where it is in the world
    aabb: AABB,
}

/// A set of models keyed by your own ids (an ECS entity, an index, ...). Only the models the
/// camera can see get drawn
pub struct ModelSet<K: Copy + Eq + Hash> {
    entries: HashMap<K, Entry>,
    bvh: BVH<K>,
}

impl<K: Copy + Eq + Hash> Default for ModelSet<K> {
    fn default() -> Self {
        Self::new()
    }
}

impl<K: Copy + Eq + Hash> ModelSet<K> {
    /// An empty set
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
            bvh: BVH::new(),
        }
    }

    /// Add a model under `key`, replacing whatever model was there before
    pub fn insert(&mut self, renderer: &RenderContext, key: K, model: Model) {
        self.insert_with_box(key, model, renderer.get_mesh_aabb(model.mesh));
    }

    /// Add a model under `key` whose mesh fits in `mesh_aabb`, in the mesh's own space,
    /// replacing whatever model was there before
    pub fn insert_with_box(&mut self, key: K, model: Model, mesh_aabb: AABB) {
        self.remove(key);
        let aabb = mesh_aabb.transformed(&model.transform);
        let node = self.bvh.insert(key, aabb);
        self.entries.insert(
            key,
            Entry {
                model,
                node,
                mesh_aabb,
                aabb,
            },
        );
    }

    /// Remove the model under `key`, giving it back if there was one
    pub fn remove(&mut self, key: K) -> Option<Model> {
        let entry = self.entries.remove(&key)?;
        self.bvh.remove(entry.node);
        Some(entry.model)
    }

    /// The model under `key`
    pub fn get(&self, key: K) -> Option<&Model> {
        self.entries.get(&key).map(|e| &e.model)
    }

    /// Whether the model under `key` gets an outline from `draw_outlines`. Does nothing if there
    /// isn't one
    pub fn set_outlined(&mut self, key: K, outlined: bool) {
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.model.outlined = outlined;
        }
    }

    /// Move, rotate, or scale the model under `key`. Does nothing if there isn't one
    pub fn set_transform(&mut self, key: K, transform: Mat4) {
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };
        let aabb = entry.mesh_aabb.transformed(&transform);
        let displacement = aabb.center() - entry.aabb.center();
        self.bvh.move_obj(entry.node, &aabb, &displacement);
        entry.model.transform = transform;
        entry.aabb = aabb;
    }

    /// How many models are in the set
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the set has no models
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Every key and model, in no particular order
    pub fn iter(&self) -> impl Iterator<Item = (K, &Model)> {
        self.entries.iter().map(|(k, e)| (*k, &e.model))
    }

    /// Keys of the models whose bounding boxes `ray` passes through, in no particular order.
    /// The boxes are loose, so this includes some models the ray misses
    pub fn iter_ray<'a>(&'a self, ray: &'a Ray) -> impl Iterator<Item = K> + 'a {
        self.bvh.iter_ray(ray)
    }

    /// The model `ray` hits first, and how far along the ray the hit is.
    pub fn raycast(&self, renderer: &RenderContext, ray: &Ray) -> Option<(K, f32)> {
        self.raycast_with(ray, |mesh, transform| {
            renderer
                .assets
                .meshes
                .require(mesh, "ModelSet::raycast")
                .shape()
                .raycast(ray, transform)
        })
    }

    /// The model `ray` hits first, and how far along the ray the hit is. `hit` says how far along
    /// `ray` a mesh placed by a transform gets hit, if it does
    pub fn raycast_with(
        &self,
        ray: &Ray,
        hit: impl Fn(MeshId, &Mat4) -> Option<f32>,
    ) -> Option<(K, f32)> {
        self.bvh
            .iter_ray(ray)
            .filter_map(|key| {
                let model = &self.entries[&key].model;
                Some((key, hit(model.mesh, &model.transform)?))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Draw every model the camera can see, lit by the last shadow pass's light
    pub fn draw(&self, renderer: &RenderContext) {
        for key in self.bvh.iter_frustum(&renderer.camera_frustum()) {
            let model = &self.entries[&key].model;
            if model.outlined {
                // Marks where it lands, for draw_outlines
                renderer.draw_mesh_textured_marked(
                    model.mesh,
                    model.texture,
                    model.transform,
                    model.tint,
                );
            } else {
                renderer.draw_mesh_textured(model.mesh, model.texture, model.transform, model.tint);
            }
        }
    }

    /// Draw every model the light can see that casts a shadow into a shadow pass
    pub fn draw_shadows(&self, pass: &ShadowPass) {
        for key in self.bvh.iter_frustum(&pass.frustum()) {
            let model = &self.entries[&key].model;
            if model.casts_shadow {
                pass.draw(model.mesh, model.transform);
            }
        }
    }

    /// Draw a solid `color` outline around every outlined model. Call it after `draw`
    pub fn draw_outlines(&self, renderer: &RenderContext, color: Vec4) {
        renderer.outline_pass(color, |pass| {
            for key in self.bvh.iter_frustum(&renderer.camera_frustum()) {
                let model = &self.entries[&key].model;
                if model.outlined {
                    pass.draw(model.mesh, model.transform);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{mesh::MeshData, mesh::MeshShape, registry::Handle};
    use nalgebra_glm::vec3;

    /// Two meshes, a unit cube (mesh 0) and a 1 by 1 flat square (mesh 1)
    fn shapes() -> [MeshShape; 2] {
        [
            MeshShape::from(MeshData::cube()),
            MeshShape::from(MeshData::plane()),
        ]
    }

    fn mesh(i: usize) -> MeshId {
        Handle::for_tests(i)
    }

    fn at(x: f32) -> Mat4 {
        nalgebra_glm::translation(&vec3(x, 0.0, 0.0))
    }

    /// Cubes keyed by name at `x` along the x axis
    fn set_of_cubes(cubes: &[(&'static str, f32)]) -> ModelSet<&'static str> {
        let mut set = ModelSet::new();
        for &(key, x) in cubes {
            let model = Model::new(mesh(0), Handle::for_tests(0), at(x));
            set.insert_with_box(key, model, shapes()[0].aabb());
        }
        set
    }

    /// Raycast against the real shapes
    fn pick(set: &ModelSet<&'static str>, ray: &Ray) -> Option<(&'static str, f32)> {
        let shapes = shapes();
        set.raycast_with(ray, |mesh, transform| {
            shapes[mesh.index_for_tests()].raycast(ray, transform)
        })
    }

    fn along_x(start: f32) -> Ray {
        Ray::new(vec3(start, 0.1, 0.2), vec3(1.0, 0.0, 0.0))
    }

    #[test]
    fn inserting_removing_and_looking_up() {
        let mut set = set_of_cubes(&[("a", 0.0), ("b", 5.0)]);
        assert_eq!(set.len(), 2);
        assert!(!set.is_empty());
        assert_eq!(set.get("a").unwrap().transform, at(0.0));
        assert!(set.get("c").is_none());

        // Inserting under a key that's taken replaces it
        set.insert_with_box(
            "a",
            Model::new(mesh(0), Handle::for_tests(0), at(9.0)),
            shapes()[0].aabb(),
        );
        assert_eq!(set.len(), 2);
        assert_eq!(set.get("a").unwrap().transform, at(9.0));

        set.set_outlined("b", true);
        assert!(set.get("b").unwrap().outlined);
        set.set_outlined("b", false);
        assert!(!set.get("b").unwrap().outlined);
        set.set_outlined("missing", true);

        assert_eq!(set.remove("a").unwrap().transform, at(9.0));
        assert!(set.remove("a").is_none());
        let keys: Vec<_> = set.iter().map(|(k, _)| k).collect();
        assert_eq!(keys, ["b"]);
        set.remove("b");
        assert!(set.is_empty());
    }

    #[test]
    fn raycast_picks_the_closest_model_it_hits() {
        let set = set_of_cubes(&[("near", 0.0), ("far", 5.0), ("off to the side", 20.0)]);
        let (key, distance) = pick(&set, &along_x(-10.0)).unwrap();
        assert_eq!(key, "near");
        assert!((distance - 9.5).abs() < 1e-4, "{distance}");

        // Starting past the near one, the far one is first
        let (key, distance) = pick(&set, &along_x(2.0)).unwrap();
        assert_eq!(key, "far");
        assert!((distance - 2.5).abs() < 1e-4, "{distance}");

        let up = Ray::new(vec3(0.0, 5.0, 0.0), vec3(0.0, 1.0, 0.0));
        assert_eq!(pick(&set, &up), None);
    }

    #[test]
    fn raycast_uses_each_models_own_mesh() {
        let mut set = ModelSet::new();
        // The flat square is only hit from above, the cube from anywhere
        let square = Model::new(mesh(1), Handle::for_tests(0), at(0.0));
        set.insert_with_box("square", square, shapes()[1].aabb());
        let down = Ray::new(vec3(0.1, 5.0, 0.2), vec3(0.0, -1.0, 0.0));
        let (key, distance) = pick(&set, &down).unwrap();
        assert_eq!(key, "square");
        assert!((distance - 5.0).abs() < 1e-4, "{distance}");
        assert_eq!(
            pick(&set, &along_x(-10.0)),
            None,
            "edge on, it's too thin to hit"
        );
    }

    #[test]
    fn moving_a_model_moves_where_it_gets_picked() {
        let mut set = set_of_cubes(&[("a", 0.0)]);
        set.set_transform("a", at(30.0));
        assert_eq!(set.get("a").unwrap().transform, at(30.0));
        let (key, distance) = pick(&set, &along_x(-10.0)).unwrap();
        assert_eq!(key, "a");
        assert!((distance - 39.5).abs() < 1e-4, "{distance}");
        // Moving a key that isn't there does nothing
        set.set_transform("missing", at(0.0));
        assert_eq!(set.len(), 1);
    }

    #[test]
    fn moving_stretches_the_box_ahead_by_twice_the_move() {
        let mut set = set_of_cubes(&[("a", 5.0)]);
        set.set_transform("a", at(15.0));
        // The cube now reaches x = 15.5, plus a little padding and 2 times the 10 it moved
        let down = |x: f32| Ray::new(vec3(x, 5.0, 0.0), vec3(0.0, -1.0, 0.0));
        assert!(set.iter_ray(&down(30.0)).any(|k| k == "a"));
        assert_eq!(set.iter_ray(&down(40.0)).count(), 0);
    }
}
