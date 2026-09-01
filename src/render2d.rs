use std::borrow::Borrow;

use nalgebra_glm::{vec2, vec3, vec4, Mat4, Vec2, Vec4};

use crate::render_core::MeshId;

use super::{
    rectangle::Rectangle,
    render_core::{DrawCommand, RenderContext, TextureId},
};

pub struct NineSlice {
    pub texture: TextureId,
    pub border: f32,
}

impl RenderContext {
    pub fn render_nine_slice(&self, nine_slice: NineSlice, dest: Rectangle) {
        if dest.size.x < 2.0 * nine_slice.border || dest.size.y < 2.0 * nine_slice.border {
            panic!("Too small! {}", nine_slice.border)
        }

        let (nine_slice_width, nine_slice_height) = self
            .get_texture_from_id(nine_slice.texture)
            .unwrap()
            .get_dimensions()
            .unwrap();

        // TODO: Clean this up!
        // Coordinates for the texture destination
        let spritesheet_coords = [
            // top-left corner
            Rectangle::new(0.0, 0.0, nine_slice.border, nine_slice.border),
            // top edge
            Rectangle::new(
                nine_slice.border,
                0.0,
                nine_slice_width as f32 - 2.0 * nine_slice.border,
                nine_slice.border,
            ),
            // top-right corner
            Rectangle::new(
                nine_slice_width as f32 - nine_slice.border,
                0.0,
                nine_slice.border,
                nine_slice.border,
            ),
            // left edge
            Rectangle::new(
                0.0,
                nine_slice.border,
                nine_slice.border,
                nine_slice_height as f32 - 2.0 * nine_slice.border,
            ),
            // center
            Rectangle::new(
                nine_slice.border,
                nine_slice.border,
                nine_slice_width as f32 - 2.0 * nine_slice.border,
                nine_slice_height as f32 - 2.0 * nine_slice.border,
            ),
            // right edge
            Rectangle::new(
                nine_slice_width as f32 - nine_slice.border,
                nine_slice.border,
                nine_slice.border,
                nine_slice_height as f32 - 2.0 * nine_slice.border,
            ),
            // bottom-left corner
            Rectangle::new(
                0.0,
                nine_slice_height as f32 - nine_slice.border,
                nine_slice.border,
                nine_slice.border,
            ),
            // bottom edge
            Rectangle::new(
                nine_slice.border,
                nine_slice_height as f32 - nine_slice.border,
                nine_slice_width as f32 - 2.0 * nine_slice.border,
                nine_slice.border,
            ),
            // bottom-right corner
            Rectangle::new(
                nine_slice_width as f32 - nine_slice.border,
                nine_slice_height as f32 - nine_slice.border,
                nine_slice.border,
                nine_slice.border,
            ),
        ];

        let dest_coords = [
            // top-left corner
            Rectangle::new(dest.pos.x, dest.pos.y, nine_slice.border, nine_slice.border),
            // top edge
            Rectangle::new(
                dest.pos.x + nine_slice.border,
                dest.pos.y,
                dest.size.x - 2.0 * nine_slice.border,
                nine_slice.border,
            ),
            // top-right corner
            Rectangle::new(
                dest.pos.x + dest.size.x - nine_slice.border,
                dest.pos.y,
                nine_slice.border,
                nine_slice.border,
            ),
            // left edge
            Rectangle::new(
                dest.pos.x,
                dest.pos.y + nine_slice.border,
                nine_slice.border,
                dest.size.y - 2.0 * nine_slice.border,
            ),
            // center
            Rectangle::new(
                dest.pos.x + nine_slice.border,
                dest.pos.y + nine_slice.border,
                dest.size.x - 2.0 * nine_slice.border,
                dest.size.y - 2.0 * nine_slice.border,
            ),
            // right edge
            Rectangle::new(
                dest.pos.x + dest.size.x - nine_slice.border,
                dest.pos.y + nine_slice.border,
                nine_slice.border,
                dest.size.y - 2.0 * nine_slice.border,
            ),
            // bottom-left corner
            Rectangle::new(
                dest.pos.x,
                dest.pos.y + dest.size.y - nine_slice.border,
                nine_slice.border,
                nine_slice.border,
            ),
            // bottom edge
            Rectangle::new(
                dest.pos.x + nine_slice.border,
                dest.pos.y + dest.size.y - nine_slice.border,
                dest.size.x - 2.0 * nine_slice.border,
                nine_slice.border,
            ),
            // bottom-right corner
            Rectangle::new(
                dest.pos.x + dest.size.x - nine_slice.border,
                dest.pos.y + dest.size.y - nine_slice.border,
                nine_slice.border,
                nine_slice.border,
            ),
        ];

        for i in 0..9 {
            let dest = dest_coords[i];
            let texture_dest = spritesheet_coords[i];

            self.copy_texture(
                dest,
                nine_slice.texture,
                texture_dest,
                &vec4(1.0, 1.0, 1.0, 1.0),
            );
        }
    }

    pub fn draw_text(&self, pos: Vec2, text: &str) {
        let font = self.get_font_from_id(self.font.borrow().unwrap()).unwrap();
        font.draw(pos, text, self);
    }

    /// Set the current draw layer. Draw calls on higher layers render on top of lower layers,
    /// regardless of call order. Layer 0 is the default. Resets to 0 after flush_2d_queue.
    pub fn set_draw_layer(&self, layer: i32) {
        self.current_draw_layer.set(layer);
    }

    pub fn copy_texture(
        &self,
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: &Vec4,
    ) {
        self.draw_queue.borrow_mut().push((
            self.current_draw_layer.get(),
            DrawCommand::CopyTexture {
                dest,
                texture_id,
                texture_dest,
                color_mod: *color_mod,
                scissor: self.scissor.get(),
            },
        ));
    }

    fn copy_texture_immediate(
        &self,
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: &Vec4,
    ) {
        let res = self.int_screen_resolution.borrow();
        unsafe {
            gl::Viewport(0, 0, res.x, res.y);
            gl::Disable(gl::DEPTH_TEST); // Disable depth test for 2D rendering
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

        self.set_program_from_id(self.get_program_id_from_name("2d").unwrap());

        let (view_matrix, proj_matrix) = self.camera_2d.view_proj_matrices();
        let model_matrix: Mat4 = nalgebra_glm::scale(
            &nalgebra_glm::translate(
                &nalgebra_glm::one(),
                &vec3(
                    1.0 - 2.0 * dest.pos.x.floor() / res.x as f32 - dest.size.x / res.x as f32,
                    1.0 - 2.0 * dest.pos.y.floor() / res.y as f32 - dest.size.y / res.y as f32,
                    3.0,
                ),
            ),
            &vec3(dest.size.x / res.x as f32, dest.size.y / res.y as f32, 0.1),
        );

        let texture = self.get_texture_from_id(texture_id).unwrap();
        let (texture_width, texture_height) = texture.get_dimensions().unwrap();
        texture.activate(gl::TEXTURE0);
        texture.associate_uniform(self.get_current_program_id(), 0, "texture0");
        let u_sprite_offset = self.get_program_uniform("u_sprite_offset").unwrap();
        unsafe {
            gl::Uniform2f(
                u_sprite_offset.id,
                texture_dest.pos.x / texture_width as f32,
                texture_dest.pos.y / texture_height as f32,
            );
        }
        let u_sprite_size = self.get_program_uniform("u_sprite_size").unwrap();
        unsafe {
            gl::Uniform2f(
                u_sprite_size.id,
                texture_dest.size.x / texture_width as f32,
                texture_dest.size.y / texture_height as f32,
            );
        }
        let u_color_mod = self.get_program_uniform("u_color_mod").unwrap();
        unsafe {
            gl::Uniform4f(
                u_color_mod.id,
                color_mod.x,
                color_mod.y,
                color_mod.z,
                color_mod.w,
            );
        }

        let quad_mesh = self
            .get_mesh_from_id(self.get_mesh_id_from_name("quad-xy").unwrap())
            .unwrap();
        self.draw(quad_mesh.borrow(), model_matrix, view_matrix, proj_matrix);
    }

    pub fn fill_rect(&self, dest: Rectangle) {
        self.draw_queue.borrow_mut().push((
            self.current_draw_layer.get(),
            DrawCommand::FillRect {
                rect: dest,
                color: *self.color.borrow(),
                scissor: self.scissor.get(),
            },
        ));
    }

    pub fn fill_polygon(&self, mesh_id: MeshId, center: Vec2, radius: f32, rotation: f32) {
        self.draw_queue.borrow_mut().push((
            self.current_draw_layer.get(),
            DrawCommand::FillPolygon {
                center,
                radius,
                mesh_id,
                rotation,
                color: *self.color.borrow(),
                scissor: self.scissor.get(),
            },
        ));
    }

    fn fill_rect_immediate(&self, dest: Rectangle, color: Vec4) {
        let res = self.int_screen_resolution.borrow();
        unsafe {
            gl::Viewport(0, 0, res.x, res.y);
            gl::Disable(gl::DEPTH_TEST);
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

        self.set_program_from_id(self.get_program_id_from_name("2d-solid").unwrap());

        let u_color = self.get_program_uniform("u_color").unwrap();
        unsafe {
            gl::Uniform4f(u_color.id, color.x, color.y, color.z, color.w);
        }

        let (view_matrix, proj_matrix) = self.camera_2d.view_proj_matrices();
        let model_matrix: Mat4 = nalgebra_glm::scale(
            &nalgebra_glm::translate(
                &nalgebra_glm::one(),
                &vec3(
                    1.0 - 2.0 * dest.pos.x / res.x as f32 - dest.size.x / res.x as f32,
                    1.0 - 2.0 * dest.pos.y / res.y as f32 - dest.size.y / res.y as f32,
                    3.0,
                ),
            ),
            &vec3(dest.size.x / res.x as f32, dest.size.y / res.y as f32, 0.1),
        );

        let quad_mesh = self
            .get_mesh_from_id(self.get_mesh_id_from_name("quad-xy").unwrap())
            .unwrap();
        self.draw(quad_mesh.borrow(), model_matrix, view_matrix, proj_matrix);
    }

    fn fill_polygon_immediate(
        &self,
        center: Vec2,
        radius: f32,
        mesh_id: MeshId,
        rotation: f32,
        color: Vec4,
    ) {
        let res = self.int_screen_resolution.borrow();
        unsafe {
            gl::Viewport(0, 0, res.x, res.y);
            gl::Disable(gl::DEPTH_TEST);
            gl::Disable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

        self.set_program_from_id(self.get_program_id_from_name("2d-solid").unwrap());

        let u_color = self.get_program_uniform("u_color").unwrap();
        unsafe {
            gl::Uniform4f(u_color.id, color.x, color.y, color.z, color.w);
        }

        let (view_matrix, proj_matrix) = self.camera_2d.view_proj_matrices();
        let mut m = nalgebra_glm::translate(
            &nalgebra_glm::one(),
            &vec3(
                1.0 - 2.0 * center.x / res.x as f32,
                1.0 - 2.0 * center.y / res.y as f32,
                3.0,
            ),
        );
        m = nalgebra_glm::scale(&m, &vec3(2.0 / res.x as f32, 2.0 / res.y as f32, 1.0));
        m = nalgebra_glm::rotate_z(&m, rotation);
        m = nalgebra_glm::scale(&m, &vec3(radius, radius, 0.1));

        let quad_mesh = self.get_mesh_from_id(mesh_id).unwrap();
        self.draw(quad_mesh.borrow(), m, view_matrix, proj_matrix);
    }

    /// Execute all queued 2D draw commands in layer order. Called once per frame after scene.render().
    pub fn flush_2d_queue(&self) {
        let commands: Vec<(i32, DrawCommand)> = {
            let mut queue = self.draw_queue.borrow_mut();
            queue.sort_by_key(|(layer, _)| *layer);
            queue.drain(..).collect()
        };
        for (_, cmd) in commands {
            match cmd {
                DrawCommand::FillRect {
                    rect,
                    color,
                    scissor,
                } => {
                    self.apply_scissor(scissor);
                    self.fill_rect_immediate(rect, color)
                }
                DrawCommand::FillPolygon {
                    center,
                    radius,
                    mesh_id,
                    rotation,
                    color,
                    scissor,
                } => {
                    self.apply_scissor(scissor);
                    self.fill_polygon_immediate(center, radius, mesh_id, rotation, color)
                }
                DrawCommand::CopyTexture {
                    dest,
                    texture_id,
                    texture_dest,
                    color_mod,
                    scissor,
                } => {
                    self.apply_scissor(scissor);
                    self.copy_texture_immediate(dest, texture_id, texture_dest, &color_mod);
                }
            }
        }
        self.current_draw_layer.set(0);
    }

    pub fn apply_scissor(&self, scissor: Option<Rectangle>) {
        let res = self.int_screen_resolution.borrow();
        match scissor {
            Some(r) => unsafe {
                gl::Enable(gl::SCISSOR_TEST);
                gl::Scissor(
                    r.pos.x as i32,
                    res.y - (r.pos.y + r.size.y) as i32,
                    r.size.x as i32,
                    r.size.y as i32,
                );
            },
            None => unsafe {
                gl::Disable(gl::SCISSOR_TEST);
            },
        }
    }

    pub fn draw_rect(&self, rect: Rectangle, thickness: f32) {
        // Top
        self.fill_rect(Rectangle {
            pos: rect.pos,
            size: vec2(rect.size.x, thickness),
        });

        // Bottom
        self.fill_rect(Rectangle {
            pos: vec2(rect.pos.x, rect.pos.y + rect.size.y - thickness),
            size: vec2(rect.size.x, thickness),
        });

        // Left (inner height only, corners handled by top/bottom)
        self.fill_rect(Rectangle {
            pos: vec2(rect.pos.x, rect.pos.y + thickness),
            size: vec2(thickness, rect.size.y - thickness * 2.0),
        });

        // Right
        self.fill_rect(Rectangle {
            pos: vec2(rect.pos.x + rect.size.x - thickness, rect.pos.y + thickness),
            size: vec2(thickness, rect.size.y - thickness * 2.0),
        });
    }
}
