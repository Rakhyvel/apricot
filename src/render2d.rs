use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use nalgebra_glm::{vec2, vec3, Mat4, Vec2, Vec4};

use crate::{
    mesh::MeshData,
    opengl::{self, DrawOptions},
    render::assets::{FontId, MeshId, TextureId},
};

use super::{
    camera::{Camera, ProjectionKind, ViewProj},
    font::Font,
    geometry::rectangle::Rectangle,
    render_core::RenderContext,
};

/// How many sides circles and rings get. Enough to look round at any reasonable size
const CIRCLE_SIDES: u32 = 64;

/// How many steps a ring's inner radius is rounded to, so rings that are almost the same share a
/// mesh
const INNER_RADIUS_STEPS: u32 = 1024;

/// What 2D drawing keeps between calls. The queued draws, the draw layer, the clip stack, and the
/// polygon meshes it's made so far
pub(crate) struct Queue2d {
    draws: RefCell<Vec<Queued>>,
    layer: Cell<i32>,
    clips: RefCell<ClipStack>,
    poly_meshes: RefCell<HashMap<(u32, u32), MeshId>>,
    font: Cell<FontId>,
    /// View and projection for 2D, which looks straight at the -1..1 square
    view_proj: ViewProj,
}

impl Queue2d {
    /// An empty queue that draws text with `font` until told otherwise
    pub(crate) fn new(font: FontId) -> Self {
        let camera = Camera::new(
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 0.0, 1.0),
            vec3(0.0, 1.0, 0.0),
            ProjectionKind::Orthographic {
                left: -1.0,
                right: 1.0,
                bottom: -1.0,
                top: 1.0,
                near: 0.1,
                far: 10.0,
            },
        );
        Self {
            draws: RefCell::default(),
            layer: Cell::default(),
            clips: RefCell::default(),
            poly_meshes: RefCell::default(),
            font: Cell::new(font),
            view_proj: camera.view_proj(1.0),
        }
    }
}

/// A draw waiting for the end of the frame, with the layer and clip it was made under
#[derive(Debug, PartialEq)]
struct Queued {
    layer: i32,
    clip: Option<Rectangle>,
    command: DrawCommand,
}

/// Which mesh a filled polygon uses. Meshes only get made when the frame is drawn
#[derive(Debug, Clone, Copy, PartialEq)]
enum PolygonShape {
    /// A mesh the user made
    Mesh(MeshId),
    /// The built-in square spanning -1 to 1
    Quad,
    /// A regular polygon, or a ring of one when `inner_key` isn't 0. The inner radius is
    /// `inner_key` out of `INNER_RADIUS_STEPS`
    Ngon { sides: u32, inner_key: u32 },
}

impl PolygonShape {
    /// A regular polygon with at least 3 sides, and a ring cut out of it if `inner` is above 0.
    /// `inner` is how far in the ring goes, from 0 to 1
    fn ngon(sides: u32, inner: f32) -> Self {
        Self::Ngon {
            sides: sides.max(3),
            inner_key: (inner.clamp(0.0, 1.0) * INNER_RADIUS_STEPS as f32).round() as u32,
        }
    }
}

#[derive(Debug, PartialEq)]
enum DrawCommand {
    FillRect {
        rect: Rectangle,
        color: Vec4,
    },
    FillPolygon {
        center: Vec2,
        /// Half-size in pixels along the mesh's X and Y. For a polygon both are just its radius
        scale: Vec2,
        shape: PolygonShape,
        rotation: f32,
        color: Vec4,
    },
    CopyTexture {
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: Vec4,
    },
}

impl Queue2d {
    fn push(&self, command: DrawCommand) {
        self.draws.borrow_mut().push(Queued {
            layer: self.layer.get(),
            clip: self.clips.borrow().current(),
            command,
        });
    }

    fn fill_rect(&self, rect: Rectangle, color: Vec4) {
        self.push(DrawCommand::FillRect { rect, color });
    }

    fn fill_polygon(
        &self,
        shape: PolygonShape,
        center: Vec2,
        scale: Vec2,
        rotation: f32,
        color: Vec4,
    ) {
        self.push(DrawCommand::FillPolygon {
            center,
            scale,
            shape,
            rotation,
            color,
        });
    }

    fn copy_texture(
        &self,
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: Vec4,
    ) {
        self.push(DrawCommand::CopyTexture {
            dest,
            texture_id,
            texture_dest,
            color_mod,
        });
    }

    fn nine_slice(&self, nine_slice: NineSlice, texture_size: Vec2, dest: Rectangle, tint: Vec4) {
        for (dest, src) in nine_slice_pieces(texture_size, nine_slice.border, dest) {
            self.copy_texture(dest, nine_slice.texture, src, tint);
        }
    }

    fn draw_poly(&self, center: Vec2, sides: u32, radius: f32, rotation: f32, color: Vec4) {
        let shape = PolygonShape::ngon(sides, 0.0);
        self.fill_polygon(shape, center, vec2(radius, radius), rotation, color);
    }

    fn fill_circle(&self, center: Vec2, radius: f32, color: Vec4) {
        self.draw_poly(center, CIRCLE_SIDES, radius, 0.0, color);
    }

    fn draw_line(&self, a: Vec2, b: Vec2, thickness: f32, color: Vec4) {
        let d = b - a;
        // The quad spans -1..1, so stretch it to half the line's length and thickness
        self.fill_polygon(
            PolygonShape::Quad,
            (a + b) / 2.0,
            vec2(d.norm() / 2.0, thickness / 2.0),
            d.y.atan2(d.x),
            color,
        );
    }

    fn draw_poly_lines(
        &self,
        center: Vec2,
        sides: u32,
        radius: f32,
        rotation: f32,
        thickness: f32,
        color: Vec4,
    ) {
        let inner = ((radius - thickness) / radius).max(0.0);
        let shape = PolygonShape::ngon(sides, inner);
        self.fill_polygon(shape, center, vec2(radius, radius), rotation, color);
    }

    fn draw_ring(&self, center: Vec2, inner_radius: f32, outer_radius: f32, color: Vec4) {
        let inner = (inner_radius / outer_radius).clamp(0.0, 1.0);
        let shape = PolygonShape::ngon(CIRCLE_SIDES, inner);
        self.fill_polygon(shape, center, vec2(outer_radius, outer_radius), 0.0, color);
    }

    fn draw_rect(&self, rect: Rectangle, thickness: f32, color: Vec4) {
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

    fn set_layer(&self, layer: i32) {
        self.layer.set(layer);
    }

    fn push_clip(&self, r: Rectangle) {
        self.clips.borrow_mut().push(r);
    }

    fn pop_clip(&self) {
        self.clips.borrow_mut().pop();
    }

    fn current_clip(&self) -> Option<Rectangle> {
        self.clips.borrow().current()
    }

    /// Everything recorded since the last frame, lowest layer first
    fn take_draws(&self) -> Vec<Queued> {
        let mut draws = std::mem::take(&mut *self.draws.borrow_mut());
        draws.sort_by_key(|draw| draw.layer); // stable, so same-layer draws keep their order
        draws
    }

    /// Get ready for the next frame. Gives back whether every clip that was pushed got popped
    fn end_frame(&self) -> bool {
        self.layer.set(0);
        self.clips.borrow_mut().end_frame()
    }
}

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

/// The nine pairs of where to draw and what to cut out of the texture, left to right and top to
/// bottom, to stretch a `texture_size` texture with edges `border` wide over `dest`. If `dest` is
/// too small to fit two borders, the drawn corners shrink to fit
fn nine_slice_pieces(
    texture_size: Vec2,
    border: f32,
    dest: Rectangle,
) -> [(Rectangle, Rectangle); 9] {
    let src = nine_slice_rects(
        Rectangle::new(0.0, 0.0, texture_size.x, texture_size.y),
        border,
    );
    let border = border.min(dest.size.x / 2.0).min(dest.size.y / 2.0);
    let dest = nine_slice_rects(dest, border);
    std::array::from_fn(|i| (dest[i], src[i]))
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
        self.draw(program, &mesh, model, &self.queue_2d.view_proj);
    }

    pub fn render_nine_slice(&self, nine_slice: NineSlice, dest: Rectangle, tint: Vec4) {
        let texture = self
            .get_texture_from_id(nine_slice.texture)
            .unwrap_or_else(|| {
                panic!(
                    "render_nine_slice: {:?} isn't a loaded texture",
                    nine_slice.texture
                )
            });
        let (w, h) = texture.get_dimensions().expect("texture has no dimensions");
        self.queue_2d
            .nine_slice(nine_slice, vec2(w as f32, h as f32), dest, tint);
    }

    /// Size of `text` in pixels if it were drawn with `draw_text`
    pub fn measure_text(&self, text: &str) -> Vec2 {
        self.get_current_font().measure(text)
    }

    pub fn set_font(&self, font: FontId) {
        self.queue_2d.font.set(font);
    }

    pub fn get_current_font_id(&self) -> FontId {
        self.queue_2d.font.get()
    }

    pub fn get_current_font(&self) -> std::cell::Ref<'_, Font> {
        self.get_font_from_id(self.queue_2d.font.get())
            .expect("invalid FontId")
    }

    pub fn draw_text(&self, pos: Vec2, text: &str, color: Vec4) {
        self.get_current_font().draw(pos, text, self, color);
    }

    pub fn set_draw_layer(&self, layer: i32) {
        self.queue_2d.set_layer(layer);
    }

    pub fn copy_texture(
        &self,
        dest: Rectangle,
        texture_id: TextureId,
        texture_dest: Rectangle,
        color_mod: &Vec4,
    ) {
        assert!(
            self.get_texture_from_id(texture_id).is_some(),
            "copy_texture: {texture_id:?} isn't a loaded texture"
        );
        self.queue_2d
            .copy_texture(dest, texture_id, texture_dest, *color_mod);
    }

    /// Turns a rectangle in pixels into a model matrix for the built-in quad. Both edges get
    /// snapped to whole pixels, so rectangles that share an edge always meet exactly.
    pub fn rect_model_matrix(&self, rect: Rectangle) -> Mat4 {
        let res = self.screen_size();
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
        self.queue_2d.fill_rect(dest, color);
    }

    pub fn fill_polygon(
        &self,
        mesh_id: MeshId,
        center: Vec2,
        radius: f32,
        rotation: f32,
        color: Vec4,
    ) {
        assert!(
            self.get_mesh_from_id(mesh_id).is_some(),
            "fill_polygon: {mesh_id:?} isn't a loaded mesh"
        );
        self.queue_2d.fill_polygon(
            PolygonShape::Mesh(mesh_id),
            center,
            vec2(radius, radius),
            rotation,
            color,
        );
    }

    pub fn draw_poly(&self, center: Vec2, sides: u32, radius: f32, rotation: f32, color: Vec4) {
        self.queue_2d
            .draw_poly(center, sides, radius, rotation, color);
    }

    pub fn fill_circle(&self, center: Vec2, radius: f32, color: Vec4) {
        self.queue_2d.fill_circle(center, radius, color);
    }

    pub fn draw_line(&self, a: Vec2, b: Vec2, thickness: f32, color: Vec4) {
        self.queue_2d.draw_line(a, b, thickness, color);
    }

    pub fn draw_poly_lines(
        &self,
        center: Vec2,
        sides: u32,
        radius: f32,
        rotation: f32,
        thickness: f32,
        color: Vec4,
    ) {
        self.queue_2d
            .draw_poly_lines(center, sides, radius, rotation, thickness, color);
    }

    pub fn draw_ring(&self, center: Vec2, inner_radius: f32, outer_radius: f32, color: Vec4) {
        self.queue_2d
            .draw_ring(center, inner_radius, outer_radius, color);
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
        let res = self.screen_size();

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
            opengl::set_scissor(draw.clip, res.y);
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
        opengl::set_scissor(None, res.y);
        before.restore();
    }

    pub fn push_clip(&self, r: Rectangle) {
        self.queue_2d.push_clip(r);
    }

    pub fn pop_clip(&self) {
        self.queue_2d.pop_clip();
    }

    pub fn current_clip(&self) -> Option<Rectangle> {
        self.queue_2d.current_clip()
    }

    pub fn draw_rect(&self, rect: Rectangle, thickness: f32, color: Vec4) {
        self.queue_2d.draw_rect(rect, thickness, color);
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

/// Nested clip rectangles, each already cut down to fit inside the ones under it
#[derive(Default)]
struct ClipStack(Vec<Rectangle>);

impl ClipStack {
    fn push(&mut self, r: Rectangle) {
        let r = self.current().map_or(r, |outer| outer.intersect(&r));
        self.0.push(r);
    }

    fn pop(&mut self) {
        self.0.pop();
    }

    fn current(&self) -> Option<Rectangle> {
        self.0.last().copied()
    }

    /// Empty the stack for the next frame. Returns whether every push this frame got popped
    fn end_frame(&mut self) -> bool {
        let balanced = self.0.is_empty();
        self.0.clear();
        balanced
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Handle;

    fn same_rect(a: Rectangle, b: Rectangle) -> bool {
        a.pos == b.pos && a.size == b.size
    }

    #[test]
    fn clips_nest_inside_each_other() {
        let mut clips = ClipStack::default();
        clips.push(Rectangle::new(0.0, 0.0, 100.0, 100.0));
        clips.push(Rectangle::new(50.0, 50.0, 100.0, 100.0));
        assert!(same_rect(
            clips.current().unwrap(),
            Rectangle::new(50.0, 50.0, 50.0, 50.0)
        ));
        clips.pop();
        assert!(same_rect(
            clips.current().unwrap(),
            Rectangle::new(0.0, 0.0, 100.0, 100.0)
        ));
        clips.pop();
        assert!(clips.current().is_none());
        assert!(clips.end_frame());
    }

    #[test]
    fn a_missing_pop_only_spoils_its_own_frame() {
        let mut clips = ClipStack::default();
        clips.push(Rectangle::new(0.0, 0.0, 10.0, 10.0));
        assert!(!clips.end_frame(), "the unpopped clip gets reported");
        assert!(clips.current().is_none(), "the next frame starts unclipped");
        assert!(clips.end_frame());
    }

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

    const RED: Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);

    fn queue() -> Queue2d {
        Queue2d::new(Handle::for_tests(0))
    }

    fn rect_at(x: f32) -> Rectangle {
        Rectangle::new(x, 0.0, 1.0, 1.0)
    }

    /// Where each recorded FillRect starts along x
    fn rect_xs(draws: &[Queued]) -> Vec<f32> {
        draws
            .iter()
            .map(|draw| match &draw.command {
                DrawCommand::FillRect { rect, .. } => rect.pos.x,
                other => panic!("expected a rect, got {other:?}"),
            })
            .collect()
    }

    /// The one command recorded
    fn only_command(queue: &Queue2d) -> DrawCommand {
        let mut draws = queue.take_draws();
        assert_eq!(draws.len(), 1);
        draws.remove(0).command
    }

    #[test]
    fn a_new_queue_draws_text_with_the_font_it_was_given() {
        let queue = Queue2d::new(Handle::for_tests(4));
        assert_eq!(queue.font.get(), Handle::for_tests(4));
    }

    #[test]
    fn draws_come_out_lowest_layer_first_and_keep_their_order_within_a_layer() {
        let queue = queue();
        for (layer, x) in [(1, 0.0), (0, 1.0), (1, 2.0), (-1, 3.0), (0, 4.0)] {
            queue.set_layer(layer);
            queue.fill_rect(rect_at(x), RED);
        }
        let draws = queue.take_draws();
        assert_eq!(
            draws.iter().map(|d| d.layer).collect::<Vec<_>>(),
            [-1, 0, 0, 1, 1]
        );
        assert_eq!(rect_xs(&draws), [3.0, 1.0, 4.0, 0.0, 2.0]);
        assert!(
            queue.take_draws().is_empty(),
            "taking them empties the queue"
        );
    }

    #[test]
    fn a_draw_remembers_the_clip_it_was_made_under() {
        let queue = queue();
        let outer = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        let inner = Rectangle::new(50.0, 50.0, 100.0, 100.0);
        let both = Rectangle::new(50.0, 50.0, 50.0, 50.0);
        queue.push_clip(outer);
        queue.fill_rect(rect_at(0.0), RED);
        queue.push_clip(inner);
        assert_eq!(queue.current_clip(), Some(both));
        queue.fill_rect(rect_at(1.0), RED);
        queue.pop_clip();
        queue.fill_rect(rect_at(2.0), RED);
        queue.pop_clip();
        queue.fill_rect(rect_at(3.0), RED);
        let clips: Vec<_> = queue.take_draws().iter().map(|d| d.clip).collect();
        assert_eq!(clips, [Some(outer), Some(both), Some(outer), None]);
    }

    #[test]
    fn ending_the_frame_resets_the_layer_and_clip_and_reports_a_missing_pop() {
        let queue = queue();
        queue.set_layer(3);
        queue.push_clip(rect_at(0.0));
        assert!(!queue.end_frame());
        queue.fill_rect(rect_at(0.0), RED);
        let draws = queue.take_draws();
        assert_eq!((draws[0].layer, draws[0].clip), (0, None));
        assert!(queue.end_frame());
    }

    #[test]
    fn fills_and_copies_record_what_they_were_given() {
        let queue = queue();
        queue.fill_rect(rect_at(7.0), RED);
        assert_eq!(
            only_command(&queue),
            DrawCommand::FillRect {
                rect: rect_at(7.0),
                color: RED
            }
        );

        let mesh = Handle::for_tests(2);
        let (center, scale) = (vec2(1.0, 2.0), vec2(3.0, 4.0));
        queue.fill_polygon(PolygonShape::Mesh(mesh), center, scale, 0.5, RED);
        assert_eq!(
            only_command(&queue),
            DrawCommand::FillPolygon {
                center,
                scale,
                shape: PolygonShape::Mesh(mesh),
                rotation: 0.5,
                color: RED,
            }
        );

        let texture = Handle::for_tests(5);
        queue.copy_texture(rect_at(1.0), texture, rect_at(2.0), RED);
        assert_eq!(
            only_command(&queue),
            DrawCommand::CopyTexture {
                dest: rect_at(1.0),
                texture_id: texture,
                texture_dest: rect_at(2.0),
                color_mod: RED,
            }
        );
    }

    #[test]
    fn a_line_is_a_stretched_quad_through_both_ends() {
        let queue = queue();
        queue.draw_line(vec2(2.0, 1.0), vec2(8.0, 9.0), 4.0, RED);
        assert_eq!(
            only_command(&queue),
            DrawCommand::FillPolygon {
                center: vec2(5.0, 5.0),
                scale: vec2(5.0, 2.0),
                shape: PolygonShape::Quad,
                rotation: 8.0f32.atan2(6.0),
                color: RED,
            }
        );
    }

    #[test]
    fn a_rect_outline_is_four_bars_that_dont_overlap() {
        let queue = queue();
        queue.draw_rect(Rectangle::new(10.0, 20.0, 100.0, 50.0), 4.0, RED);
        let bars: Vec<Rectangle> = queue
            .take_draws()
            .iter()
            .map(|d| match &d.command {
                DrawCommand::FillRect { rect, .. } => *rect,
                other => panic!("expected a rect, got {other:?}"),
            })
            .collect();
        let top = Rectangle::new(10.0, 20.0, 100.0, 4.0);
        let bottom = Rectangle::new(10.0, 66.0, 100.0, 4.0);
        let left = Rectangle::new(10.0, 24.0, 4.0, 42.0);
        let right = Rectangle::new(106.0, 24.0, 4.0, 42.0);
        assert_eq!(bars, [top, bottom, left, right]);
    }

    #[test]
    fn polygons_rings_and_circles_pick_their_shape_and_size() {
        let center = vec2(5.0, 6.0);
        let polygon = |shape, radius, rotation| DrawCommand::FillPolygon {
            center,
            scale: vec2(radius, radius),
            shape,
            rotation,
            color: RED,
        };
        let ngon = |sides, inner_key| PolygonShape::Ngon { sides, inner_key };
        let queue = queue();

        queue.draw_poly(center, 6, 10.0, 0.5, RED);
        assert_eq!(only_command(&queue), polygon(ngon(6, 0), 10.0, 0.5));
        queue.draw_poly(center, 1, 10.0, 0.0, RED);
        assert_eq!(only_command(&queue), polygon(ngon(3, 0), 10.0, 0.0));
        queue.fill_circle(center, 7.0, RED);
        assert_eq!(
            only_command(&queue),
            polygon(ngon(CIRCLE_SIDES, 0), 7.0, 0.0)
        );

        // Half way in, then so far in it is clamped to the edge
        queue.draw_ring(center, 5.0, 10.0, RED);
        let half = polygon(ngon(CIRCLE_SIDES, 512), 10.0, 0.0);
        assert_eq!(only_command(&queue), half);
        queue.draw_ring(center, 20.0, 10.0, RED);
        let full = polygon(ngon(CIRCLE_SIDES, 1024), 10.0, 0.0);
        assert_eq!(only_command(&queue), full);

        // Outlines grow inward from the radius
        queue.draw_poly_lines(center, 5, 10.0, 0.25, 2.0, RED);
        assert_eq!(only_command(&queue), polygon(ngon(5, 819), 10.0, 0.25));
        queue.draw_poly_lines(center, 5, 10.0, 0.0, 12.0, RED);
        assert_eq!(only_command(&queue), polygon(ngon(5, 0), 10.0, 0.0));
    }

    #[test]
    fn nearly_the_same_ring_shares_a_mesh() {
        let shape = |inner| PolygonShape::ngon(8, inner);
        assert_eq!(shape(0.5), shape(0.5001));
        assert_ne!(shape(0.5), shape(0.52));
        let (sides, inner_key) = (8, 0);
        assert_eq!(shape(-3.0), PolygonShape::Ngon { sides, inner_key });
        let (sides, inner_key) = (8, 1024);
        assert_eq!(shape(3.0), PolygonShape::Ngon { sides, inner_key });
    }

    #[test]
    fn nine_slice_pieces_cut_the_texture_and_stretch_it_over_the_destination() {
        let dest = Rectangle::new(100.0, 50.0, 200.0, 100.0);
        let pieces = nine_slice_pieces(vec2(30.0, 30.0), 10.0, dest);
        let piece = |dest: [f32; 4], src: [f32; 4]| {
            (
                Rectangle::new(dest[0], dest[1], dest[2], dest[3]),
                Rectangle::new(src[0], src[1], src[2], src[3]),
            )
        };
        assert_eq!(
            pieces[0],
            piece([100.0, 50.0, 10.0, 10.0], [0.0, 0.0, 10.0, 10.0])
        );
        assert_eq!(
            pieces[4],
            piece([110.0, 60.0, 180.0, 80.0], [10.0, 10.0, 10.0, 10.0])
        );
        assert_eq!(
            pieces[8],
            piece([290.0, 140.0, 10.0, 10.0], [20.0, 20.0, 10.0, 10.0])
        );
    }

    #[test]
    fn nine_slice_corners_shrink_when_the_destination_is_too_small() {
        let dest = Rectangle::new(0.0, 0.0, 12.0, 40.0);
        let pieces = nine_slice_pieces(vec2(30.0, 30.0), 10.0, dest);
        // Half the width is 6, so the corners are 6 across and the middle has no width
        assert_eq!(pieces[0].0, Rectangle::new(0.0, 0.0, 6.0, 6.0));
        assert_eq!(pieces[1].0.size, vec2(0.0, 6.0));
        assert_eq!(pieces[2].0, Rectangle::new(6.0, 0.0, 6.0, 6.0));
        // Cutting up the texture doesn't change
        assert_eq!(pieces[0].1, Rectangle::new(0.0, 0.0, 10.0, 10.0));
    }

    #[test]
    fn nine_slice_corners_also_shrink_when_the_destination_is_too_short() {
        let dest = Rectangle::new(0.0, 0.0, 40.0, 12.0);
        let pieces = nine_slice_pieces(vec2(30.0, 30.0), 10.0, dest);
        // Half the height is 6, so the corners are 6 tall and the middle has no height
        assert_eq!(pieces[0].0, Rectangle::new(0.0, 0.0, 6.0, 6.0));
        assert_eq!(pieces[3].0.size, vec2(6.0, 0.0));
        assert_eq!(pieces[6].0, Rectangle::new(0.0, 6.0, 6.0, 6.0));
    }

    #[test]
    fn a_nine_slice_records_its_nine_pieces_as_copies() {
        let queue = queue();
        let texture = Handle::for_tests(1);
        let dest = Rectangle::new(0.0, 0.0, 90.0, 60.0);
        let nine_slice = NineSlice {
            texture,
            border: 5.0,
        };
        queue.nine_slice(nine_slice, vec2(20.0, 20.0), dest, RED);
        let commands: Vec<DrawCommand> =
            queue.take_draws().into_iter().map(|d| d.command).collect();
        let expected: Vec<DrawCommand> = nine_slice_pieces(vec2(20.0, 20.0), 5.0, dest)
            .into_iter()
            .map(|(dest, src)| DrawCommand::CopyTexture {
                dest,
                texture_id: texture,
                texture_dest: src,
                color_mod: RED,
            })
            .collect();
        assert_eq!(commands.len(), 9);
        assert_eq!(commands, expected);
    }
}
