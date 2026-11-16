//! Drawing a frame's recorded 2D commands

use nalgebra_glm::{vec2, vec3, I32Vec2, Mat4, Vec2, Vec4};

use super::command::{DrawCommand, PolygonShape, INNER_RADIUS_STEPS};
use crate::{
    camera::ViewProj,
    geometry::rectangle::Rectangle,
    mesh::MeshData,
    opengl::{self, DrawOptions, ScissorBox},
    render::assets::{MeshId, TextureId},
    render_core::RenderContext,
};

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
        let program = self.builtins.program_2d;
        let texture = self
            .get_texture_from_id(texture_id)
            .expect("invalid TextureId");
        texture.activate(0);
        self.uniform(program, "texture0").set_i32(0);
        self.uniform(program, "u_sprite_offset")
            .set_vec2(&sprite_offset);
        self.uniform(program, "u_sprite_size")
            .set_vec2(&sprite_size);
        self.uniform(program, "u_color").set_vec4(&color);

        let mesh = self.get_mesh_from_id(mesh_id).expect("invalid MeshId");
        self.draw(program, &mesh, model, &pixel_view_proj(self.screen_size()));
    }

    /// Turns a rectangle in pixels into a model matrix for the built-in quad. Both edges get
    /// snapped to whole pixels, so rectangles that share an edge always meet exactly.
    pub fn rect_model_matrix(&self, rect: Rectangle) -> Mat4 {
        quad_model(rect)
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
        let model_matrix = polygon_model(center, scale, rotation);

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
        let draws = self.queue_2d.take_draws();

        // Setup the GL context for drawing 2d stuff
        let res = self.screen_size();
        opengl::set_viewport(res.x, res.y);
        let before = DrawOptions {
            depth_test: false,
            culling: false,
            ..Default::default()
        }
        .apply();

        for draw in draws {
            opengl::set_scissor(draw.clip.map(|clip| scissor_box(clip, res.y)));
            match draw.command {
                DrawCommand::FillRect { rect, color } => self.fill_rect_immediate(rect, color),
                DrawCommand::FillPolygon {
                    center,
                    scale,
                    shape,
                    rotation,
                    color,
                } => {
                    let mesh_id = self.polygon_mesh(shape);
                    self.fill_polygon_immediate(center, scale, mesh_id, rotation, color)
                }
                DrawCommand::CopyTexture {
                    dest,
                    texture_id,
                    texture_dest,
                    color_mod,
                } => self.copy_texture_immediate(dest, texture_id, texture_dest, &color_mod),
            }
        }

        let balanced = self.queue_2d.end_frame();
        debug_assert!(balanced, "push_clip without a matching pop_clip");
        if !balanced {
            log::warn!("push_clip without a matching pop_clip this frame");
        }

        // Put things back the way you found them
        opengl::set_scissor(None);
        before.restore();
    }

    /// The mesh a polygon shape draws with, making it first if it's a regular polygon nobody has
    /// drawn yet
    fn polygon_mesh(&self, shape: PolygonShape) -> MeshId {
        match shape {
            PolygonShape::Mesh(id) => id,
            PolygonShape::Quad => self.builtins.quad,
            PolygonShape::Ngon { sides, inner_key } => self.ngon_mesh(sides, inner_key),
        }
    }

    fn ngon_mesh(&self, sides: u32, inner_key: u32) -> MeshId {
        if let Some(&id) = self.queue_2d.poly_meshes.borrow().get(&(sides, inner_key)) {
            return id;
        }
        let data = if inner_key == 0 {
            MeshData::ngon(sides)
        } else {
            MeshData::ngon_ring(sides, inner_key as f32 / INNER_RADIUS_STEPS as f32)
        };
        let id = self.add_mesh_data(data, None);
        self.queue_2d
            .poly_meshes
            .borrow_mut()
            .insert((sides, inner_key), id);
        id
    }
}

/// The view and projection that turn pixels, measured from the top left of a window `size` pixels
/// across, into positions on the screen
fn pixel_view_proj(size: I32Vec2) -> ViewProj {
    ViewProj {
        view: Mat4::identity(),
        proj: nalgebra_glm::ortho(0.0, size.x as f32, size.y as f32, 0.0, -1.0, 1.0),
    }
}

/// `rect` with both its edges rounded down to whole pixels, so rectangles that share an edge
/// always meet exactly
fn snap_to_pixels(rect: Rectangle) -> Rectangle {
    let (x0, y0) = (rect.pos.x.floor(), rect.pos.y.floor());
    let (x1, y1) = (
        (rect.pos.x + rect.size.x).floor(),
        (rect.pos.y + rect.size.y).floor(),
    );
    Rectangle::new(x0, y0, x1 - x0, y1 - y0)
}

/// The box of pixels a clip rectangle lets drawing through in a window `window_height` pixels
/// tall. It's snapped the same way drawn rectangles are, so a clip never cuts a pixel off the
/// rectangle it was made to match
fn scissor_box(clip: Rectangle, window_height: i32) -> ScissorBox {
    let clip = snap_to_pixels(clip);
    let (width, height) = (clip.size.x.max(0.0) as i32, clip.size.y.max(0.0) as i32);
    ScissorBox {
        x: clip.pos.x as i32,
        y: window_height - (clip.pos.y as i32 + height),
        width,
        height,
    }
}

/// The model matrix that puts the built-in quad over `rect` in pixels
fn quad_model(rect: Rectangle) -> Mat4 {
    let rect = snap_to_pixels(rect);
    let center = rect.pos + rect.size / 2.0;
    let placed = nalgebra_glm::translate(&Mat4::identity(), &vec3(center.x, center.y, 0.0));
    // Half turned, since the quad's texture coordinates run the other way
    nalgebra_glm::scale(&placed, &vec3(-rect.size.x / 2.0, -rect.size.y / 2.0, 1.0))
}

/// The model matrix that puts a polygon mesh with its center at `center` in pixels, stretched by
/// `scale` pixels, and turned clockwise by `rotation` radians
fn polygon_model(center: Vec2, scale: Vec2, rotation: f32) -> Mat4 {
    let placed = nalgebra_glm::translate(&Mat4::identity(), &vec3(center.x, center.y, 0.0));
    let turned = nalgebra_glm::rotate_z(&placed, rotation);
    nalgebra_glm::scale(&turned, &vec3(scale.x, scale.y, 1.0))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::{vec4, I32Vec2, Vec3};
    use std::f32::consts::FRAC_PI_2;

    fn window() -> I32Vec2 {
        I32Vec2::new(800, 600)
    }

    /// Where a point of a mesh drawn with `model` lands in the window, in pixels from the top left
    fn on_screen(model: &Mat4, p: Vec3) -> Vec2 {
        let clip = pixel_view_proj(window()).combined() * model * vec4(p.x, p.y, p.z, 1.0);
        let ndc = clip.xyz() / clip.w;
        vec2((ndc.x + 1.0) * 400.0, (1.0 - ndc.y) * 300.0)
    }

    fn assert_near(a: Vec2, b: Vec2) {
        assert!((a - b).norm() < 1e-3, "{a:?} isn't {b:?}");
    }

    #[test]
    fn pixels_land_where_the_window_lays_them_out_with_the_origin_at_the_top_left() {
        let identity = Mat4::identity();
        assert_near(on_screen(&identity, vec3(0.0, 0.0, 0.0)), vec2(0.0, 0.0));
        assert_near(
            on_screen(&identity, vec3(800.0, 600.0, 0.0)),
            vec2(800.0, 600.0),
        );
        assert_near(
            on_screen(&identity, vec3(400.0, 300.0, 0.0)),
            vec2(400.0, 300.0),
        );
        assert_near(
            on_screen(&identity, vec3(100.0, 500.0, 0.0)),
            vec2(100.0, 500.0),
        );
    }

    #[test]
    fn drawing_at_depth_zero_is_never_cut_off() {
        let clip = pixel_view_proj(window()).combined() * vec4(123.0, 456.0, 0.0, 1.0);
        let depth = clip.z / clip.w;
        assert!((-1.0..=1.0).contains(&depth), "{depth}");
    }

    #[test]
    fn a_rect_covers_its_pixels_with_the_texture_upright() {
        let rect = Rectangle::new(10.0, 20.0, 100.0, 50.0);
        let quad = MeshData::quad();
        for v in &quad.vertices {
            let at = on_screen(&quad_model(rect), v.position);
            // The texture's top left is the rect's top left, and its bottom right the rect's
            let expected = vec2(10.0 + 100.0 * v.uv.x, 20.0 + 50.0 * v.uv.y);
            assert_near(at, expected);
        }
    }

    #[test]
    fn rects_snap_to_whole_pixels_so_neighbors_meet_exactly() {
        let quad = MeshData::quad();
        let edges = |rect: Rectangle| {
            let xs: Vec<f32> = quad
                .vertices
                .iter()
                .map(|v| on_screen(&quad_model(rect), v.position).x)
                .collect();
            (
                xs.iter().copied().fold(f32::MAX, f32::min),
                xs.iter().copied().fold(f32::MIN, f32::max),
            )
        };
        let (a_left, a_right) = edges(Rectangle::new(0.0, 0.0, 10.5, 5.0));
        let (b_left, b_right) = edges(Rectangle::new(10.5, 0.0, 10.5, 5.0));
        assert!((a_left - 0.0).abs() < 1e-3 && (a_right - 10.0).abs() < 1e-3);
        assert!((b_left - 10.0).abs() < 1e-3 && (b_right - 21.0).abs() < 1e-3);
    }

    #[test]
    fn snapping_rounds_both_edges_down() {
        let snapped = snap_to_pixels(Rectangle::new(10.6, 20.2, 5.6, 4.1));
        assert_eq!(snapped, Rectangle::new(10.0, 20.0, 6.0, 4.0));
    }

    #[test]
    fn a_polygon_starts_pointing_right_and_turns_clockwise() {
        let model = polygon_model(vec2(100.0, 100.0), vec2(10.0, 20.0), 0.0);
        assert_near(on_screen(&model, vec3(1.0, 0.0, 0.0)), vec2(110.0, 100.0));
        assert_near(on_screen(&model, vec3(0.0, 1.0, 0.0)), vec2(100.0, 120.0));

        let turned = polygon_model(vec2(100.0, 100.0), vec2(10.0, 10.0), FRAC_PI_2);
        // A quarter turn clockwise swings the right side down to the bottom
        assert_near(on_screen(&turned, vec3(1.0, 0.0, 0.0)), vec2(100.0, 110.0));
    }

    #[test]
    fn a_polygon_is_stretched_before_it_is_turned() {
        let model = polygon_model(vec2(100.0, 100.0), vec2(10.0, 20.0), FRAC_PI_2);
        // Stretched to (0, 20) first, then a quarter turn clockwise puts it to the left
        assert_near(on_screen(&model, vec3(0.0, 1.0, 0.0)), vec2(80.0, 100.0));
    }

    #[test]
    fn a_clip_counts_up_from_the_bottom_of_the_window() {
        let clip = scissor_box(Rectangle::new(10.0, 20.0, 100.0, 50.0), 600);
        let expected = ScissorBox {
            x: 10,
            y: 530,
            width: 100,
            height: 50,
        };
        assert_eq!(clip, expected);
    }

    #[test]
    fn a_clip_covers_exactly_the_pixels_a_rect_drawn_there_would() {
        let rects = [
            Rectangle::new(10.6, 20.2, 5.6, 4.1),
            Rectangle::new(0.0, 0.0, 800.0, 600.0),
            Rectangle::new(33.5, 47.5, 20.5, 10.5),
            Rectangle::new(-4.5, 590.4, 30.2, 20.9),
        ];
        for rect in rects {
            let drawn = snap_to_pixels(rect);
            let clip = scissor_box(rect, 600);
            assert_eq!(clip.x as f32, drawn.pos.x, "{rect:?}");
            assert_eq!(clip.width as f32, drawn.size.x, "{rect:?}");
            assert_eq!(clip.height as f32, drawn.size.y, "{rect:?}");
            // Measured down from the top of the window, it starts where the rect does
            assert_eq!((600 - clip.y - clip.height) as f32, drawn.pos.y, "{rect:?}");
        }
    }

    #[test]
    fn a_clip_with_fractions_isnt_a_pixel_smaller_than_what_it_clips() {
        let clip = scissor_box(Rectangle::new(10.6, 20.2, 5.6, 4.1), 600);
        assert_eq!((clip.x, clip.width, clip.height), (10, 6, 4));
    }

    #[test]
    fn an_empty_or_backwards_clip_covers_nothing() {
        let empty = scissor_box(Rectangle::new(10.0, 10.0, 0.0, 0.0), 600);
        assert_eq!((empty.width, empty.height), (0, 0));
        let backwards = scissor_box(Rectangle::new(10.0, 10.0, -5.0, -5.0), 600);
        assert_eq!((backwards.width, backwards.height), (0, 0));
    }
}
