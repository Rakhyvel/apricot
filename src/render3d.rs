use std::borrow::Borrow;

use crate::render_core::LinePathComponent;

use super::{
    bvh::BVH,
    opengl::*,
    render_core::{ModelComponent, RenderContext},
    shadow_map::DirectionalLightSource,
};

use hecs::{Entity, World};

impl RenderContext {
    pub fn render_3d_models_system(
        &self,
        world: &mut World,
        directional_light: &DirectionalLightSource,
        bvh: &BVH<Entity>,
        debug: bool,
    ) {
        self.set_program_from_id(self.get_program_id_from_name("3d").unwrap());

        // let u_sun_dir = self.get_program_uniform("u_sun_dir").expect("erm lol");
        // unsafe {
        //     gl::Uniform3f(
        //         u_sun_dir.id,
        //         directional_light.light_dir.x,
        //         directional_light.light_dir.y,
        //         directional_light.light_dir.z,
        //     );
        // }

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
            gl::Clear(gl::COLOR_BUFFER_BIT | gl::DEPTH_BUFFER_BIT | gl::STENCIL_BUFFER_BIT);
        }

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

        let camera_frustum = &self.camera.borrow().frustum();

        let (view_matrix, proj_matrix) = self.camera.borrow().view_proj_matrices();
        for model_id in bvh.iter_frustum(camera_frustum, debug) {
            let model = world.get::<&mut ModelComponent>(model_id).unwrap();
            let mesh = self.get_mesh_from_id(model.mesh_id).unwrap();
            let texture = self.get_texture_from_id(model.texture_id).unwrap();
            let model_matrix = model.get_model_matrix();

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

    pub fn render_3d_line_paths(&self, world: &World) {
        unsafe {
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }
        let (view_matrix, proj_matrix) = self.camera.borrow().view_proj_matrices();
        for (_entity, line_path) in world.query::<&LinePathComponent>().iter() {
            self.draw_line_path(line_path, view_matrix, proj_matrix)
        }
    }
}
