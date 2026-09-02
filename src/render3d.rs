use std::borrow::Borrow;

use crate::{
    high_precision::{self, WorldPosition},
    render_core::LinePathComponent,
};

use super::{
    bvh::BVH,
    opengl::*,
    render_core::{ModelComponent, RenderContext},
    shadow_map::DirectionalLightSource,
};

use hecs::{Entity, World};
use nalgebra_glm::Vec3;

impl RenderContext {
    pub fn render_3d_models_system(
        &self,
        world: &mut World,
        directional_light: &DirectionalLightSource,
        bvh: &BVH<Entity>,
        hp_camera: Option<&high_precision::Camera>,
        debug: bool,
    ) {
        // Set to "3d" program (defined by user)
        self.set_program_from_id(self.get_program_id_from_name("3d").unwrap());

        // Set the directional light direction
        let u_sun_dir = self.get_program_uniform("u_sun_dir").expect("erm lol");
        unsafe {
            gl::Uniform3f(
                u_sun_dir.id,
                directional_light.light_dir.x,
                directional_light.light_dir.y,
                directional_light.light_dir.z,
            );
        }

        // TODO: Not sure what goes here
        unsafe {
            gl::Viewport(
                0,
                0,
                self.int_screen_resolution.borrow().x,
                self.int_screen_resolution.borrow().y,
            );
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
            gl::Enable(gl::DEPTH_TEST);
            gl::StencilOp(gl::KEEP, gl::REPLACE, gl::REPLACE);
        }

        // TODO: Not sure what goes here
        let (light_view_matrix, light_proj_matrix) =
            directional_light.shadow_camera.view_proj_matrices();
        let u_light_matrix = Uniform::new(self.get_current_program_id(), "light_mvp").unwrap();
        let light_proj_view = light_proj_matrix * light_view_matrix;
        unsafe {
            gl::UniformMatrix4fv(
                u_light_matrix.id,
                1,
                gl::FALSE,
                &light_proj_view.columns(0, 4)[0],
            );
        }

        // Get the camera frustum, view matrix, and proj matrix
        let camera_frustum = &self.camera.borrow().frustum();
        let (view_matrix, proj_matrix) = self.camera.borrow().view_proj_matrices();

        // Render each model in the BVH that's inside the frustum
        for model_id in bvh.iter_frustum(camera_frustum, debug) {
            let model = world.get::<&mut ModelComponent>(model_id).unwrap();
            let mesh = self.get_mesh_from_id(model.mesh_id).unwrap();
            let texture = self.get_texture_from_id(model.texture_id).unwrap();

            // If a high precision camera and WorldPosition are both present, compute the model amtrix from
            // the camera-relative f64 offset cast to f32.
            // Otherwise, fallback to existing f32 model matrix
            let model_matrix = match hp_camera {
                Some(hp) => {
                    if let Ok(wp) = world.get::<&WorldPosition>(model_id) {
                        let relative_pos: Vec3 = nalgebra_glm::convert(wp.pos - hp.world_pos);
                        model.get_model_matrix_with_position(relative_pos)
                    } else {
                        model.get_model_matrix()
                    }
                }
                None => model.get_model_matrix(),
            };

            if model.outlined {
                unsafe {
                    gl::StencilFunc(gl::ALWAYS, 1, 0xFF);
                    gl::StencilMask(0xFF);
                }
            } else {
                unsafe {
                    gl::StencilMask(0x00);
                }
            }

            texture.activate(gl::TEXTURE0);
            texture.associate_uniform(self.get_current_program_id(), 0, "texture0");

            directional_light.activate_framebuffer(self.get_current_program_id());

            self.draw(mesh.borrow(), model_matrix, view_matrix, proj_matrix);
        }
        // println!("{:?}", rendered);
    }

    pub fn render_3d_outlines_system(&self, world: &mut World, bvh: &BVH<Entity>) {
        unsafe {
            gl::StencilFunc(gl::NOTEQUAL, 1, 0xFF);
            gl::StencilMask(0x00);
            gl::Enable(gl::STENCIL_TEST);
            gl::Disable(gl::DEPTH_TEST);
        }
        self.set_program_from_id(self.get_program_id_from_name("3d-solid").unwrap());
        let camera_frustum = &self.camera.borrow().frustum();

        let u_color = self.get_program_uniform("u_color").unwrap();
        unsafe {
            gl::Uniform4f(
                u_color.id,
                self.color.borrow().x,
                self.color.borrow().y,
                self.color.borrow().z,
                self.color.borrow().w,
            );
        }

        let (view_matrix, proj_matrix) = self.camera.borrow().view_proj_matrices();
        for model_id in bvh.iter_frustum(camera_frustum, false) {
            let mut model = world.get::<&mut ModelComponent>(model_id).unwrap();
            if !model.outlined {
                continue;
            }
            let mesh = self.get_mesh_from_id(model.mesh_id).unwrap();

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
        let (view_matrix, proj_matrix) = self.camera.borrow().view_proj_matrices();
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
