//! The 2D canvas. Drawing calls are recorded as the frame goes, then drawn together at the end in
//! layer order

use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
};

use nalgebra_glm::{vec2, Vec2, Vec4};

use crate::{
    geometry::rectangle::Rectangle,
    render::{
        assets::{FontId, MeshId, TextureId},
        text::Font,
    },
    render_core::RenderContext,
};

mod command;
mod flush;
mod nine_slice;

pub use nine_slice::NineSlice;

use command::{DrawCommand, PolygonShape, Queued};

/// How many sides circles and rings get. Enough to look round at any reasonable size
const CIRCLE_SIDES: u32 = 64;

/// What 2D drawing keeps between calls. The queued draws, the draw layer, the clip stack, and the
/// polygon meshes it's made so far
pub(crate) struct Queue2d {
    draws: RefCell<Vec<Queued>>,
    layer: Cell<i32>,
    clips: RefCell<ClipStack>,
    poly_meshes: RefCell<HashMap<(u32, u32), MeshId>>,
    font: Cell<FontId>,
}

impl Queue2d {
    /// An empty queue that draws text with `font` until told otherwise
    pub(crate) fn new(font: FontId) -> Self {
        Self {
            draws: RefCell::default(),
            layer: Cell::default(),
            clips: RefCell::default(),
            poly_meshes: RefCell::default(),
            font: Cell::new(font),
        }
    }
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

    fn draw_text(&self, font: &Font, pos: Vec2, text: &str, color: Vec4) {
        font.layout(text, |offset, source| {
            let dest = Rectangle {
                pos: pos + offset,
                size: source.size,
            };
            self.copy_texture(dest, font.cache_texture, source, color);
        });
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

impl RenderContext {
    /// Size of `text` in pixels if it were drawn with `draw_text`
    pub fn measure_text(&self, text: &str) -> Vec2 {
        self.get_current_font().measure(text)
    }

    pub fn set_font(&self, font: FontId) {
        self.queue_2d.font.set(font);
    }

    /// The font text is drawn with. If the font that was set has been unloaded, that's the
    /// default font
    pub fn get_current_font_id(&self) -> FontId {
        self.assets
            .fonts
            .live_or(self.queue_2d.font.get(), self.builtins.font)
    }

    pub fn get_current_font(&self) -> std::cell::Ref<'_, Font> {
        self.get_font_from_id(self.get_current_font_id())
            .expect("the default font was unloaded")
    }

    pub fn draw_text(&self, pos: Vec2, text: &str, color: Vec4) {
        let font = self.get_current_font();
        assert!(
            self.get_texture_from_id(font.cache_texture).is_some(),
            "draw_text: {:?} isn't a loaded texture",
            font.cache_texture
        );
        self.queue_2d.draw_text(&font, pos, text, color);
    }

    /// Set the current draw layer. Draw calls on higher layers render on top of lower layers,
    /// regardless of call order. Layer 0 is the default. Resets to 0 after flush_2d_queue.
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

    /// Filled regular polygon. `rotation` 0 puts the first vertex at +X.
    pub fn draw_poly(&self, center: Vec2, sides: u32, radius: f32, rotation: f32, color: Vec4) {
        self.queue_2d
            .draw_poly(center, sides, radius, rotation, color);
    }

    /// Draws a filled circle
    pub fn fill_circle(&self, center: Vec2, radius: f32, color: Vec4) {
        self.queue_2d.fill_circle(center, radius, color);
    }

    /// Draws a line from `a` to `b`. The ends are cut off square, no rounded caps
    pub fn draw_line(&self, a: Vec2, b: Vec2, thickness: f32, color: Vec4) {
        self.queue_2d.draw_line(a, b, thickness, color);
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
        self.queue_2d
            .draw_poly_lines(center, sides, radius, rotation, thickness, color);
    }

    /// Circular ring between two radii
    pub fn draw_ring(&self, center: Vec2, inner_radius: f32, outer_radius: f32, color: Vec4) {
        self.queue_2d
            .draw_ring(center, inner_radius, outer_radius, color);
    }

    /// Push a rectangle to the clip stack
    pub fn push_clip(&self, r: Rectangle) {
        self.queue_2d.push_clip(r);
    }

    /// Pop most recently pushed rectangle from the clip stack
    pub fn pop_clip(&self) {
        self.queue_2d.pop_clip();
    }

    /// The rectangle at the top of the clip stack
    pub fn current_clip(&self) -> Option<Rectangle> {
        self.queue_2d.current_clip()
    }

    pub fn draw_rect(&self, rect: Rectangle, thickness: f32, color: Vec4) {
        self.queue_2d.draw_rect(rect, thickness, color);
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
    use crate::render::text::test_font;

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
    fn text_records_one_copy_per_glyph_at_the_place_the_font_puts_it() {
        let queue = queue();
        let texture = Handle::for_tests(3);
        // 8 by 16 glyphs that advance 10, with a newline moving down 20
        let font = test_font(texture);
        queue.draw_text(&font, vec2(100.0, 50.0), "ai\nb\u{e9}", RED);
        let glyph = |byte: u8| {
            let x = (byte - 32) as f32 * 8.0;
            Rectangle::new(x, 0.0, 8.0, 16.0)
        };
        let copy = |x: f32, y: f32, byte: u8| DrawCommand::CopyTexture {
            dest: Rectangle::new(x, y, 8.0, 16.0),
            texture_id: texture,
            texture_dest: glyph(byte),
            color_mod: RED,
        };
        let commands: Vec<DrawCommand> =
            queue.take_draws().into_iter().map(|d| d.command).collect();
        // The accented letter has no glyph, so it gets no copy
        assert_eq!(
            commands,
            [
                copy(100.0, 50.0, b'a'),
                copy(110.0, 50.0, b'i'),
                copy(100.0, 70.0, b'b')
            ]
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
}
