use std::borrow::Borrow;

use crate::{
    high_precision::{self, WorldPosition},
    render_core::LinePathComponent,
};

use super::{
    bvh::BVH,
    render_core::{ModelComponent, RenderContext},
};

use hecs::{Entity, World};
use nalgebra_glm::{vec4, Vec3, Vec4};

impl RenderContext {
    /// Draw every model in `bvh` that the camera can see, lit by the last shadow pass's light.
    /// Entities with a `WorldPosition` are placed relative to `hp_camera`, if there is one
    pub fn render_3d_models_system(
        &self,
        world: &World,
        bvh: &BVH<Entity>,
        hp_camera: Option<&high_precision::Camera>,
        debug: bool,
    ) {
        // Outlined models mark the stencil buffer for render_3d_outlines_system
        unsafe { gl::StencilOp(gl::KEEP, gl::REPLACE, gl::REPLACE) };

        for model_id in bvh.iter_frustum(&self.camera_frustum(), debug) {
            let model = world
                .get::<&ModelComponent>(model_id)
                .expect("entity in the BVH has no ModelComponent");

            // Use the camera-relative f64 position when there is one, so far-away things don't jitter
            let model_matrix = match (hp_camera, world.get::<&WorldPosition>(model_id)) {
                (Some(hp), Ok(wp)) => {
                    let relative_pos: Vec3 = nalgebra_glm::convert(wp.pos - hp.world_pos);
                    model.get_model_matrix_with_position(relative_pos)
                }
                _ => model.get_model_matrix(),
            };

            unsafe {
                if model.outlined {
                    gl::StencilFunc(gl::ALWAYS, 1, 0xFF);
                    gl::StencilMask(0xFF);
                } else {
                    gl::StencilMask(0x00);
                }
            }

            self.draw_mesh_textured(
                model.mesh_id,
                model.texture_id,
                model_matrix,
                vec4(1.0, 1.0, 1.0, 1.0),
            );
        }
    }

    pub fn render_3d_outlines_system(&self, world: &mut World, bvh: &BVH<Entity>, color: Vec4) {
        unsafe {
            gl::StencilFunc(gl::NOTEQUAL, 1, 0xFF);
            gl::StencilMask(0x00);
            gl::Enable(gl::STENCIL_TEST);
            gl::Disable(gl::DEPTH_TEST);
        }
        self.set_program_from_id(self.builtins.program_3d_solid);
        let camera_frustum = &self.camera_frustum();

        self.get_program_uniform("u_color").set_vec4(&color);

        let (view_matrix, proj_matrix) = self.camera_view_proj();
        for model_id in bvh.iter_frustum(camera_frustum, false) {
            let mut model = world
                .get::<&mut ModelComponent>(model_id)
                .expect("entity in the BVH has no ModelComponent");
            if !model.outlined {
                continue;
            }
            let mesh = self
                .get_mesh_from_id(model.mesh_id)
                .expect("invalid MeshId");

            let old_scale = model.get_scale();
            model.set_scale(old_scale * 1.2);
            let model_matrix = model.get_model_matrix();
            model.set_scale(old_scale);

            self.draw(mesh.borrow(), model_matrix, view_matrix, proj_matrix);
        }

        unsafe {
            gl::StencilMask(0xFF);
            gl::Enable(gl::STENCIL_TEST);
            gl::StencilFunc(gl::ALWAYS, 1, 0xFF);
        }
    }

    pub fn render_3d_line_paths(&self, world: &World, hp_camera: Option<&high_precision::Camera>) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
            gl::Disable(gl::MULTISAMPLE);
        }
        let (view_matrix, proj_matrix) = self.camera_view_proj();
        for (entity, line_path) in world.query::<&LinePathComponent>().iter() {
            let position = match hp_camera {
                Some(hp) => {
                    if let Ok(wp) = world.get::<&WorldPosition>(entity) {
                        nalgebra_glm::convert(wp.pos - hp.world_pos)
                    } else {
                        line_path.position
                    }
                }
                None => line_path.position,
            };
            self.draw_line_path_at(line_path, position, view_matrix, proj_matrix)
        }
        unsafe {
            gl::Enable(gl::MULTISAMPLE);
        }
    }
}
