use nalgebra_glm::{vec2, vec3, Mat4, Vec2, Vec4};

use crate::{
    mesh_data::MeshData,
    render_core::MeshId,
};

use super::{
    rectangle::Rectangle,
    render_core::{DrawCommand, RenderContext, TextureId},
};

/// How many sides circles and rings get. Enough to look round at any reasonable size
const CIRCLE_SIDES: u32 = 64;

/// A texture that can stretch to any size without its edges getting blurry or squished
#[derive(Clone, Copy)]
pub struct NineSlice {
    pub texture: TextureId,
    /// How wide the edges are, in texture pixels
    pub border: f32,
}

/// Cuts `outer` into a 3x3 grid with edges `border` wide. Goes left to right, top to bottom
fn nine_slice_rects(outer: Rectangle, border: f32) -> [Rectangle; 9] {
    let (x, y) = (outer.pos.x, outer.pos.y);
    let (w, h) = (outer.size.x, outer.size.y);
    let xs = [x, x + border, x + w - border];
    let ys = [y, y + border, y + h - border];
    let widths = [border, w - 2.0 * border, border];
    let heights = [border, h - 2.0 * border, border];
    std::array::from_fn(|i| {
        let (col, row) = (i % 3, i / 3);
        Rectangle::new(xs[col], ys[row], widths[col], heights[row])
    })
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

        let (view, proj) = self.camera_2d.view_proj_matrices(1.0);
        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(&mesh, model, view, proj);
    }

    /// Draws a nine-slice texture stretched over `dest`. The corners stay their real size, the
    /// edges stretch one way, and the middle stretches both ways. If `dest` is too small to fit
    /// two borders, the corners shrink to fit
    pub fn render_nine_slice(&self, nine_slice: NineSlice, dest: Rectangle, tint: Vec4) {
        let (w, h) = self
            .get_texture_from_id(nine_slice.texture)
            .expect("invalid TextureId")
            .get_dimensions()
            .expect("texture has no dimensions");
        let src = nine_slice_rects(
            Rectangle::new(0.0, 0.0, w as f32, h as f32),
            nine_slice.border,
        );

        let border = nine_slice
            .border
            .min(dest.size.x / 2.0)
            .min(dest.size.y / 2.0);
        let dest = nine_slice_rects(dest, border);

        for (dest, src) in dest.into_iter().zip(src) {
            self.copy_texture(dest, nine_slice.texture, src, &tint);
        }
    }

    /// Size of `text` in pixels if it were drawn with `draw_text`
    pub fn measure_text(&self, text: &str) -> Vec2 {
        self.get_current_font().measure(text)
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
                scissor: self.current_clip(),
            },
        ));
    }

    /// Turns a rectangle in pixels into a model matrix for the built-in quad. Both edges get
    /// snapped to whole pixels, so rectangles that share an edge always meet exactly.
    fn rect_model_matrix(&self, rect: Rectangle) -> Mat4 {
        let res = self.int_screen_resolution;
        let (res_x, res_y) = (res.x as f32, res.y as f32);
        let (x0, y0) = (rect.pos.x.floor(), rect.pos.y.floor());
        let (x1, y1) = (
            (rect.pos.x + rect.size.x).floor(),
            (rect.pos.y + rect.size.y).floor(),
        );
        let (w, h) = (x1 - x0, y1 - y0);
        nalgebra_glm::scale(
            &nalgebra_glm::translate(
                &nalgebra_glm::one(),
                &vec3(
                    1.0 - (2.0 * x0 + w) / res_x,
                    1.0 - (2.0 * y0 + h) / res_y,
                    3.0,
                ),
            ),
            &vec3(w / res_x, h / res_y, 0.1),
        )
    }

    fn copy_texture_immediate(
        &self,
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: &Vec4,
    ) {
        let model_matrix = self.rect_model_matrix(dest);

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
                scissor: self.current_clip(),
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
        self.queue_polygon(mesh_id, center, vec2(radius, radius), rotation, color);
    }

    fn queue_polygon(
        &self,
        mesh_id: MeshId,
        center: Vec2,
        scale: Vec2,
        rotation: f32,
        color: Vec4,
    ) {
        self.draw_queue.borrow_mut().push((
            self.current_draw_layer.get(),
            DrawCommand::FillPolygon {
                center,
                scale,
                mesh_id,
                rotation,
                color,
                scissor: self.current_clip(),
            },
        ));
    }

    /// Filled regular polygon. `rotation` 0 puts the first vertex at +X.
    pub fn draw_poly(&self, center: Vec2, sides: u32, radius: f32, rotation: f32, color: Vec4) {
        self.queue_polygon(
            self.poly_mesh(sides, 0.0),
            center,
            vec2(radius, radius),
            rotation,
            color,
        );
    }

    /// Draws a filled circle
    pub fn fill_circle(&self, center: Vec2, radius: f32, color: Vec4) {
        self.draw_poly(center, CIRCLE_SIDES, radius, 0.0, color);
    }

    /// Draws a line from `a` to `b`. The ends are cut off square, no rounded caps
    pub fn draw_line(&self, a: Vec2, b: Vec2, thickness: f32, color: Vec4) {
        let d = b - a;
        // The quad spans -1..1, so stretch it to half the line's length and thickness
        self.queue_polygon(
            self.builtins.quad,
            (a + b) / 2.0,
            vec2(d.norm() / 2.0, thickness / 2.0),
            d.y.atan2(d.x),
            color,
        );
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
            vec2(radius, radius),
            rotation,
            color,
        );
    }

    /// Circular ring between two radii
    pub fn draw_ring(&self, center: Vec2, inner_radius: f32, outer_radius: f32, color: Vec4) {
        let inner = (inner_radius / outer_radius).clamp(0.0, 1.0);
        self.queue_polygon(
            self.poly_mesh(CIRCLE_SIDES, inner),
            center,
            vec2(outer_radius, outer_radius),
            0.0,
            color,
        );
    }

    fn fill_rect_immediate(&self, dest: Rectangle, color: Vec4) {
        let model_matrix = self.rect_model_matrix(dest);

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
        scale: Vec2,
        mesh_id: MeshId,
        rotation: f32,
        color: Vec4,
    ) {
        let res = self.int_screen_resolution;

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
        model_matrix = nalgebra_glm::scale(&model_matrix, &vec3(scale.x, scale.y, 0.1));

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

        // Setup the GL context for drawing 2d stuff
        let res = self.int_screen_resolution;
        unsafe {
            gl::Viewport(0, 0, res.x, res.y);
            gl::Disable(gl::DEPTH_TEST);
            gl::Disable(gl::CULL_FACE);
            gl::Enable(gl::BLEND);
            gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        }

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
                    scale,
                    mesh_id,
                    rotation,
                    color,
                    scissor,
                } => {
                    self.apply_scissor(scissor);
                    self.fill_polygon_immediate(center, scale, mesh_id, rotation, color)
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

        debug_assert!(
            self.clip_stack.borrow().is_empty(),
            "push_clip without a matching pop_clip"
        );

        // Put things back the way you found them
        unsafe {
            gl::Disable(gl::SCISSOR_TEST);
            gl::Enable(gl::DEPTH_TEST);
            gl::Enable(gl::CULL_FACE);
        }
        self.current_draw_layer.set(0);
    }

    /// Push a rectangle to the clip stack
    pub fn push_clip(&self, r: Rectangle) {
        let mut stack = self.clip_stack.borrow_mut();
        let r = stack.last().map_or(r, |outer| outer.intersect(&r));
        stack.push(r);
    }

    /// Pop most recently pushed rectangle from the clip stack
    pub fn pop_clip(&self) {
        self.clip_stack.borrow_mut().pop();
    }

    /// The rectangle at the top of the clip stack
    pub fn current_clip(&self) -> Option<Rectangle> {
        self.clip_stack.borrow().last().copied()
    }

    fn apply_scissor(&self, scissor: Option<Rectangle>) {
        let res = self.int_screen_resolution;
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
        let id = self.add_mesh_data(data, None);
        self.poly_meshes.borrow_mut().insert((sides, inner_key), id);
        id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nine_slice_rects_tile_the_outer_rect() {
        let outer = Rectangle::new(10.0, 20.0, 100.0, 50.0);
        let rects = nine_slice_rects(outer, 8.0);

        // Corners are exactly border-sized and tucked into the outer corners
        for i in [0, 2, 6, 8] {
            assert_eq!((rects[i].size.x, rects[i].size.y), (8.0, 8.0));
        }
        assert_eq!((rects[0].pos.x, rects[0].pos.y), (10.0, 20.0));
        assert_eq!((rects[8].pos.x + 8.0, rects[8].pos.y + 8.0), (110.0, 70.0));

        // Every row and column butts up against the next with no gaps
        for row in 0..3 {
            let r = &rects[row * 3..row * 3 + 3];
            assert_eq!(r[0].pos.x + r[0].size.x, r[1].pos.x);
            assert_eq!(r[1].pos.x + r[1].size.x, r[2].pos.x);
        }
        for col in 0..3 {
            let (a, b, c) = (rects[col], rects[col + 3], rects[col + 6]);
            assert_eq!(a.pos.y + a.size.y, b.pos.y);
            assert_eq!(b.pos.y + b.size.y, c.pos.y);
        }

        // ...so the areas add up to the whole thing
        let area: f32 = rects.iter().map(|r| r.size.x * r.size.y).sum();
        assert_eq!(area, 100.0 * 50.0);
    }

    #[test]
    fn nine_slice_rects_collapse_middle_at_two_borders() {
        let rects = nine_slice_rects(Rectangle::new(0.0, 0.0, 16.0, 16.0), 8.0);
        assert_eq!(rects[4].size.x, 0.0);
        assert_eq!(rects[4].size.y, 0.0);
    }
}
