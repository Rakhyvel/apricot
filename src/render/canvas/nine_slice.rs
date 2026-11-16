//! Stretching a texture over any size without squishing its edges

use nalgebra_glm::{vec2, Vec2, Vec4};

use super::Queue2d;
use crate::{geometry::rectangle::Rectangle, render::assets::TextureId, render::RenderContext};

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

impl Queue2d {
    fn nine_slice(&self, nine_slice: NineSlice, texture_size: Vec2, dest: Rectangle, tint: Vec4) {
        for (dest, src) in nine_slice_pieces(texture_size, nine_slice.border, dest) {
            self.copy_texture(dest, nine_slice.texture, src, tint);
        }
    }
}

impl RenderContext {
    /// Draws a nine-slice texture stretched over `dest`. The corners stay their real size, the
    /// edges stretch one way, and the middle stretches both ways. If `dest` is too small to fit
    /// two borders, the corners shrink to fit
    pub fn render_nine_slice(&self, nine_slice: NineSlice, dest: Rectangle, tint: Vec4) {
        let texture = self
            .assets
            .textures
            .require(nine_slice.texture, "render_nine_slice");
        let (w, h) = texture.get_dimensions().expect("texture has no dimensions");
        self.queue_2d
            .nine_slice(nine_slice, vec2(w as f32, h as f32), dest, tint);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Handle;
    use crate::render::canvas::command::DrawCommand;

    const TINT: Vec4 = Vec4::new(1.0, 0.0, 0.0, 1.0);

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
        let queue = Queue2d::new(Handle::for_tests(0));
        let texture = Handle::for_tests(1);
        let dest = Rectangle::new(0.0, 0.0, 90.0, 60.0);
        let nine_slice = NineSlice {
            texture,
            border: 5.0,
        };
        queue.nine_slice(nine_slice, vec2(20.0, 20.0), dest, TINT);
        let commands: Vec<DrawCommand> =
            queue.take_draws().into_iter().map(|d| d.command).collect();
        let expected: Vec<DrawCommand> = nine_slice_pieces(vec2(20.0, 20.0), 5.0, dest)
            .into_iter()
            .map(|(dest, src)| DrawCommand::CopyTexture {
                dest,
                texture_id: texture,
                texture_dest: src,
                color_mod: TINT,
            })
            .collect();
        assert_eq!(commands.len(), 9);
        assert_eq!(commands, expected);
    }
}
