//! A set of 3D models with frustum culling, keyed with custom ids

use std::{collections::HashMap, hash::Hash};

use nalgebra_glm::{vec4, Mat4, Vec4};

use crate::{
    aabb::AABB,
    bvh::{BVHNodeId, BVH},
    opengl::{DrawOptions, Stencil},
    ray::Ray,
    render_core::{MeshId, RenderContext, TextureId},
    shadow_map::ShadowPass,
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
    node: BVHNodeId,
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
        self.remove(key);
        let aabb = renderer
            .get_mesh_aabb(model.mesh)
            .transformed(&model.transform);
        let node = self.bvh.insert(key, aabb);
        self.entries.insert(key, Entry { model, node, aabb });
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

    /// The model under `key`, to change its texture, tint, outline, or shadow. Changing its
    /// transform here doesn't move it for culling or picking
    pub fn get_mut(&mut self, key: K) -> Option<&mut Model> {
        self.entries.get_mut(&key).map(|e| &mut e.model)
    }

    /// Move, rotate, or scale the model under `key`. Does nothing if there isn't one
    pub fn set_transform(&mut self, renderer: &RenderContext, key: K, transform: Mat4) {
        let Some(entry) = self.entries.get_mut(&key) else {
            return;
        };
        let aabb = renderer
            .get_mesh_aabb(entry.model.mesh)
            .transformed(&transform);
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
        self.bvh
            .iter_ray(ray)
            .filter_map(|key| {
                let model = &self.entries[&key].model;
                let to_local = model.transform.try_inverse()?;

                // Move the ray into the model's space instead of moving every triangle out
                let o = ray.origin();
                let d = ray.dir();
                let local_ray = Ray::new(
                    (to_local * vec4(o.x, o.y, o.z, 1.0)).xyz(),
                    (to_local * vec4(d.x, d.y, d.z, 0.0)).xyz(),
                );

                let mesh = renderer
                    .get_mesh_from_id(model.mesh)
                    .expect("invalid MeshId");
                let t = mesh
                    .triangles()
                    .filter_map(|tri| tri.raycast(&local_ray, 0.0..f32::INFINITY))
                    .map(|hit| hit.t)
                    .min_by(f32::total_cmp)?;

                // Back to world space, so differently scaled models compare fairly
                let p = local_ray.at(t);
                let hit = (model.transform * vec4(p.x, p.y, p.z, 1.0)).xyz();
                Some((key, nalgebra_glm::distance(&o, &hit)))
            })
            .min_by(|a, b| a.1.total_cmp(&b.1))
    }

    /// Draw every model the camera can see, lit by the last shadow pass's light
    pub fn draw(&self, renderer: &RenderContext) {
        for key in self.bvh.iter_frustum(&renderer.camera_frustum(), false) {
            let model = &self.entries[&key].model;
            // Outlined models mark the stencil buffer for draw_outlines
            let options = DrawOptions {
                stencil: if model.outlined {
                    Stencil::Mark
                } else {
                    Stencil::Off
                },
                ..Default::default()
            };
            renderer.draw_mesh_textured_with(
                model.mesh,
                model.texture,
                model.transform,
                model.tint,
                &options,
            );
        }
    }

    /// Draw every model the light can see that casts a shadow into a shadow pass
    pub fn draw_shadows(&self, pass: &ShadowPass) {
        for key in self.bvh.iter_frustum(&pass.frustum(), false) {
            let model = &self.entries[&key].model;
            if model.casts_shadow {
                pass.draw(model.mesh, model.transform);
            }
        }
    }

    /// Draw a solid `color` outline around every outlined model. Call it after `draw`
    pub fn draw_outlines(&self, renderer: &RenderContext, color: Vec4) {
        // Only draw where the model itself isn't
        let before = DrawOptions {
            stencil: Stencil::OutsideMarks,
            depth_test: false,
            ..Default::default()
        }
        .apply();
        renderer.set_program_from_id(renderer.builtins.program_3d_solid);
        renderer.get_program_uniform("u_color").set_vec4(&color);
        let (view, proj) = renderer.camera_view_proj();

        for key in self.bvh.iter_frustum(&renderer.camera_frustum(), false) {
            let model = &self.entries[&key].model;
            if !model.outlined {
                continue;
            }
            let mesh = renderer
                .get_mesh_from_id(model.mesh)
                .expect("invalid MeshId");
            // A slightly bigger copy, so only the rim pokes out past the stencil
            let bigger = nalgebra_glm::scale(&model.transform, &nalgebra_glm::vec3(1.2, 1.2, 1.2));
            renderer.draw(&mesh, bigger, view, proj);
        }

        before.restore();
    }
}
