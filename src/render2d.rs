use std::borrow::Borrow;

use nalgebra_glm::{vec2, vec3, vec4, Mat4, Vec2, Vec4};

use crate::{
    mesh_data::MeshData,
    render_core::{Mesh, MeshId},
};

use super::{
    rectangle::Rectangle,
    render_core::{DrawCommand, RenderContext, TextureId},
};

pub struct NineSlice {
    pub texture: TextureId,
    pub border: f32,
}

impl RenderContext {
    fn draw_2d(
        &self,
        mesh_id: MeshId,
        model: Mat4,
        texture_id: TextureId,
        sprite_offset: Vec2,
        sprite_size: Vec2,
        color: Vec4,
    ) {
        self.set_program_from_id(self.builtins.program_2d);
        let texture = self
            .get_texture_from_id(texture_id)
            .expect("invalid TextureId");
        texture.activate(gl::TEXTURE0);
        texture.associate_uniform(self.get_current_program_id(), 0, "texture0");
        self.get_program_uniform("u_sprite_offset")
            .set_vec2(&sprite_offset);
        self.get_program_uniform("u_sprite_size")
            .set_vec2(&sprite_size);
        self.get_program_uniform("u_color").set_vec4(&color);

        let (view, proj) = self.camera_2d.view_proj_matrices();
        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(&mesh, model, view, proj);
    }

    pub fn render_nine_slice(&self, nine_slice: NineSlice, dest: Rectangle) {
        // If `dest` is smaller than two borders, shrink the drawn corners to fit
        let border = nine_slice
            .border
            .min(dest.size.x / 2.0)
            .min(dest.size.y / 2.0);

        let (nine_slice_width, nine_slice_height) = self
            .get_texture_from_id(nine_slice.texture)
            .expect("invalid TextureId")
            .get_dimensions()
            .expect("texture has no dimensions");

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
            Rectangle::new(dest.pos.x, dest.pos.y, border, border),
            // top edge
            Rectangle::new(
                dest.pos.x + border,
                dest.pos.y,
                dest.size.x - 2.0 * border,
                border,
            ),
            // top-right corner
            Rectangle::new(
                dest.pos.x + dest.size.x - border,
                dest.pos.y,
                border,
                border,
            ),
            // left edge
            Rectangle::new(
                dest.pos.x,
                dest.pos.y + border,
                border,
                dest.size.y - 2.0 * border,
            ),
            // center
            Rectangle::new(
                dest.pos.x + border,
                dest.pos.y + border,
                dest.size.x - 2.0 * border,
                dest.size.y - 2.0 * border,
            ),
            // right edge
            Rectangle::new(
                dest.pos.x + dest.size.x - border,
                dest.pos.y + border,
                border,
                dest.size.y - 2.0 * border,
            ),
            // bottom-left corner
            Rectangle::new(
                dest.pos.x,
                dest.pos.y + dest.size.y - border,
                border,
                border,
            ),
            // bottom edge
            Rectangle::new(
                dest.pos.x + border,
                dest.pos.y + dest.size.y - border,
                dest.size.x - 2.0 * border,
                border,
            ),
            // bottom-right corner
            Rectangle::new(
                dest.pos.x + dest.size.x - border,
                dest.pos.y + dest.size.y - border,
                border,
                border,
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

    pub fn draw_text(&self, pos: Vec2, text: &str, color: Vec4) {
        self.get_current_font().draw(pos, text, self, color);
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

        let (w, h) = self
            .get_texture_from_id(texture_id)
            .expect("invalid TextureId")
            .get_dimensions()
            .expect("texture has no dimensions");
        let sprite_offset = vec2(texture_dest.pos.x / w as f32, texture_dest.pos.y / h as f32);
        let sprite_size = vec2(
            texture_dest.size.x / w as f32,
            texture_dest.size.y / h as f32,
        );

        self.draw_2d(
            self.builtins.quad,
            model_matrix,
            texture_id,
            sprite_offset,
            sprite_size,
            *color_mod,
        );
    }

    pub fn fill_rect(&self, dest: Rectangle, color: Vec4) {
        self.draw_queue.borrow_mut().push((
            self.current_draw_layer.get(),
            DrawCommand::FillRect {
                rect: dest,
                color,
                scissor: self.scissor.get(),
            },
        ));
    }

    pub fn fill_polygon(
        &self,
        mesh_id: MeshId,
        center: Vec2,
        radius: f32,
        rotation: f32,
        color: Vec4,
    ) {
        self.queue_polygon(mesh_id, center, radius, rotation, color);
    }

    fn queue_polygon(
        &self,
        mesh_id: MeshId,
        center: Vec2,
        radius: f32,
        rotation: f32,
        color: Vec4,
    ) {
        self.draw_queue.borrow_mut().push((
            self.current_draw_layer.get(),
            DrawCommand::FillPolygon {
                center,
                radius,
                mesh_id,
                rotation,
                color,
                scissor: self.scissor.get(),
            },
        ));
    }

    /// Filled regular polygon. `rotation` 0 puts the first vertex at +X.
    pub fn draw_poly(&self, center: Vec2, sides: u32, radius: f32, rotation: f32, color: Vec4) {
        self.queue_polygon(self.poly_mesh(sides, 0.0), center, radius, rotation, color);
    }

    /// Draws the outline of a polygon. `thickness` grows inward from `radius`
    pub fn draw_poly_lines(
        &self,
        center: Vec2,
        sides: u32,
        radius: f32,
        rotation: f32,
        thickness: f32,
        color: Vec4,
    ) {
        let inner = ((radius - thickness) / radius).max(0.0);
        self.queue_polygon(
            self.poly_mesh(sides, inner),
            center,
            radius,
            rotation,
            color,
        );
    }

    /// Circular ring between two radii
    pub fn draw_ring(&self, center: Vec2, inner_radius: f32, outer_radius: f32, color: Vec4) {
        let inner = (inner_radius / outer_radius).clamp(0.0, 1.0);
        self.queue_polygon(self.poly_mesh(64, inner), center, outer_radius, 0.0, color);
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

        self.draw_2d(
            self.builtins.quad,
            model_matrix,
            self.builtins.white_texture,
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            color,
        );
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

        let mut model_matrix = nalgebra_glm::translate(
            &nalgebra_glm::one(),
            &vec3(
                1.0 - 2.0 * center.x / res.x as f32,
                1.0 - 2.0 * center.y / res.y as f32,
                3.0,
            ),
        );
        model_matrix = nalgebra_glm::scale(
            &model_matrix,
            &vec3(2.0 / res.x as f32, 2.0 / res.y as f32, 1.0),
        );
        model_matrix = nalgebra_glm::rotate_z(&model_matrix, rotation);
        model_matrix = nalgebra_glm::scale(&model_matrix, &vec3(radius, radius, 0.1));

        self.draw_2d(
            mesh_id,
            model_matrix,
            self.builtins.white_texture,
            vec2(0.0, 0.0),
            vec2(1.0, 1.0),
            color,
        );
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

    pub fn draw_rect(&self, rect: Rectangle, thickness: f32, color: Vec4) {
        // Top
        self.fill_rect(
            Rectangle {
                pos: rect.pos,
                size: vec2(rect.size.x, thickness),
            },
            color,
        );

        // Bottom
        self.fill_rect(
            Rectangle {
                pos: vec2(rect.pos.x, rect.pos.y + rect.size.y - thickness),
                size: vec2(rect.size.x, thickness),
            },
            color,
        );

        // Left (inner height only, corners handled by top/bottom)
        self.fill_rect(
            Rectangle {
                pos: vec2(rect.pos.x, rect.pos.y + thickness),
                size: vec2(thickness, rect.size.y - thickness * 2.0),
            },
            color,
        );

        // Right
        self.fill_rect(
            Rectangle {
                pos: vec2(rect.pos.x + rect.size.x - thickness, rect.pos.y + thickness),
                size: vec2(thickness, rect.size.y - thickness * 2.0),
            },
            color,
        );
    }

    fn poly_mesh(&self, sides: u32, inner: f32) -> MeshId {
        let sides = sides.max(3);
        let inner_key = (inner.clamp(0.0, 1.0) * 1024.0).round() as u32;
        if let Some(&id) = self.poly_meshes.borrow().get(&(sides, inner_key)) {
            return id;
        }
        let data = if inner_key == 0 {
            MeshData::ngon(sides)
        } else {
            MeshData::ngon_ring(sides, inner_key as f32 / 1024.0)
        };
        let id = self.add_mesh(Mesh::from_data(&data), None);
        self.poly_meshes.borrow_mut().insert((sides, inner_key), id);
        id
    }
}
