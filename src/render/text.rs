//! Fonts, from rasterizing a font file to laying out text with the result

use nalgebra_glm::{vec2, Vec2};
use sdl2::{pixels::Color, rect::Rect, surface::Surface, ttf::Sdl2TtfContext};
use std::path::Path;

use crate::{error::Error, geometry::rectangle::Rectangle, render::assets::TextureId};

#[derive(Copy, Clone, Default)]
struct Glyph {
    rect: Rectangle,
    advance: usize,
}

const ASCII_FIRST_PRINTABLE_CHAR: u8 = 32;
const ASCII_LAST_PRINTABLE_CHAR: u8 = 127;
const ASCII_NUM_PRINTABLE_CHARS: usize =
    (ASCII_LAST_PRINTABLE_CHAR - ASCII_FIRST_PRINTABLE_CHAR) as usize;

/// Where each printable ASCII glyph is in a font's image, and how text lays out with them
struct GlyphTable {
    /// All 95 printable ASCII glyphs
    glyphs: [Glyph; ASCII_NUM_PRINTABLE_CHARS],
    height: usize,
    /// How many pixels a newline moves down
    line_skip: usize,
}

/// A cached font
pub struct Font {
    pub(crate) cache_texture: TextureId,
    table: GlyphTable,
}

/// Turns font files into atlases. Holds the TTF library open for as long as it lives
pub(crate) struct FontRasterizer {
    ttf_context: Sdl2TtfContext,
}

impl Font {
    /// Measure how large the text would be if drawn
    pub fn measure(&self, text: &str) -> Vec2 {
        self.table.measure(text)
    }

    /// Lays out `text` starting at the origin, calling `place` with where each glyph goes and the
    /// part of the font's image that holds it. Returns the size of the whole block of text
    pub(crate) fn layout(&self, text: &str, mut place: impl FnMut(Vec2, Rectangle)) -> Vec2 {
        self.table
            .layout(text, |offset, glyph| place(offset, glyph.rect))
    }
}

impl GlyphTable {
    /// Measure how large the text would be if drawn
    fn measure(&self, text: &str) -> Vec2 {
        self.layout(text, |_, _| {})
    }

    /// Lays out `text` starting at the origin, calling `place` with each glyph and where it goes.
    /// Returns the size of the whole block of text
    fn layout(&self, text: &str, mut place: impl FnMut(Vec2, &Glyph)) -> Vec2 {
        let mut cursor = Vec2::zeros();
        let mut width: f32 = 0.0;
        for c in text.chars() {
            if c == '\n' {
                cursor = vec2(0.0, cursor.y + self.line_skip as f32);
                continue;
            }
            // Skip anything we don't have a glyph for
            let Some(glyph) = u8::try_from(c).ok().and_then(|b| self.get_glyph(b)) else {
                continue;
            };
            place(cursor, &glyph);
            cursor.x += glyph.advance as f32;
            width = width.max(cursor.x);
        }
        vec2(width, cursor.y + self.height as f32)
    }

    fn get_glyph(&self, char: u8) -> Option<Glyph> {
        let glyph_idx = glyph_idx_from_byte(char)?;
        if glyph_idx as usize >= self.glyphs.len() {
            None
        } else {
            Some(self.glyphs[glyph_idx])
        }
    }
}

/// A font drawn out into one image, with where each glyph landed. Rasterizing a font gives one of
/// these, and the renderer turns its image into a texture
pub(crate) struct Atlas {
    image: Surface<'static>,
    table: GlyphTable,
}

impl Atlas {
    fn new(font: &sdl2::ttf::Font, name: &str) -> Result<Self, Error> {
        let height = font.height() as usize;
        let (image, glyphs) = pack_glyphs(font, height, name)?;
        Ok(Self {
            image,
            table: GlyphTable {
                glyphs,
                height,
                line_skip: font.recommended_line_spacing() as usize,
            },
        })
    }

    /// The finished font, once `upload` turns the atlas image into a texture. It gets the image's
    /// width, height, and tightly packed RGBA bytes
    pub(crate) fn into_font(self, upload: impl FnOnce(u32, u32, &[u8]) -> TextureId) -> Font {
        let surface = self
            .image
            .convert_format(sdl2::pixels::PixelFormatEnum::RGBA32)
            .expect("out of memory converting surface to RGBA");
        let (width, height) = (surface.width() as usize, surface.height() as usize);
        let pixels = surface
            .without_lock()
            .expect("a converted surface never needs locking");
        let packed = pack_rows(pixels, width, height, surface.pitch() as usize);
        Font {
            cache_texture: upload(width as u32, height as u32, &packed),
            table: self.table,
        }
    }
}

/// `height` rows of `width` RGBA pixels, copied out of `bytes` where each row starts `pitch` bytes
/// after the last, so any padding at the end of a row is dropped
fn pack_rows(bytes: &[u8], width: usize, height: usize, pitch: usize) -> Vec<u8> {
    let row = width * 4;
    (0..height)
        .flat_map(|y| &bytes[y * pitch..y * pitch + row])
        .copied()
        .collect()
}

fn font_error(name: &str, msg: String) -> Error {
    Error::Font {
        font: name.to_string(),
        msg,
    }
}

/// Every printable ASCII glyph drawn into one image, and where each one is
fn pack_glyphs(
    font: &sdl2::ttf::Font,
    height: usize,
    name: &str,
) -> Result<(Surface<'static>, [Glyph; ASCII_NUM_PRINTABLE_CHARS]), Error> {
    let mut glyphs = [Glyph::default(); ASCII_NUM_PRINTABLE_CHARS];
    let mut x_offset: usize = 0;
    let mut y_offset: usize = 0;
    let mut mega_surface = Surface::new(
        height as u32 * 16,
        height as u32 * 16,
        sdl2::pixels::PixelFormatEnum::RGBA32,
    )
    .map_err(|msg| font_error(name, msg))?;
    mega_surface
        .set_blend_mode(sdl2::render::BlendMode::None)
        .map_err(|msg| font_error(name, msg))?;
    let color = Color {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    for (glyph, i) in glyphs
        .iter_mut()
        .zip(ASCII_FIRST_PRINTABLE_CHAR..ASCII_LAST_PRINTABLE_CHAR)
    {
        let c = char::from(i);
        let mut surf = font
            .render(&c.to_string())
            .blended(color)
            .map_err(|e| font_error(name, e.to_string()))?;
        if x_offset + surf.width() as usize > height * 16 {
            x_offset = 0;
            y_offset += height;
        }
        let src_rect = Rect::new(0, 0, surf.width(), surf.height());
        let dest_rect = Rect::new(
            x_offset as i32,
            y_offset as i32,
            surf.width(),
            surf.height(),
        );
        surf.set_blend_mode(sdl2::render::BlendMode::None)
            .map_err(|msg| font_error(name, msg))?;
        surf.blit(Some(src_rect), &mut mega_surface, Some(dest_rect))
            .map_err(|msg| font_error(name, msg))?;
        x_offset += surf.width() as usize;

        // A font missing this character keeps a zero-sized glyph
        if let Some(metrics) = font.find_glyph_metrics(c) {
            *glyph = Glyph::new(dest_rect, metrics.advance);
        }
    }

    Ok((mega_surface, glyphs))
}

fn glyph_idx_from_byte(char: u8) -> Option<usize> {
    if !(ASCII_FIRST_PRINTABLE_CHAR..ASCII_LAST_PRINTABLE_CHAR).contains(&char) {
        return None;
    }

    Some((char - ASCII_FIRST_PRINTABLE_CHAR) as usize)
}

impl FontRasterizer {
    pub(crate) fn new() -> Result<Self, Error> {
        let ttf_context = sdl2::ttf::init().map_err(|msg| Error::Sdl(format!("{msg}")))?;
        Ok(Self { ttf_context })
    }

    /// Rasterize a .ttf or .otf font file at `size`
    pub(crate) fn rasterize_file(
        &self,
        path: impl AsRef<Path>,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<Atlas, Error> {
        let mut ttf_font = self
            .ttf_context
            .load_font(path.as_ref(), size)
            .map_err(|msg| font_error(name, msg))?;
        ttf_font.set_style(style);
        Atlas::new(&ttf_font, name)
    }

    /// Rasterize a .ttf or .otf font from its bytes at `size`
    pub(crate) fn rasterize_bytes(
        &self,
        bytes: &[u8],
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
    ) -> Result<Atlas, Error> {
        let rwops = sdl2::rwops::RWops::from_bytes(bytes).map_err(|msg| font_error(name, msg))?;
        let mut ttf_font = self
            .ttf_context
            .load_font_from_rwops(rwops, size)
            .map_err(|msg| font_error(name, msg))?;
        ttf_font.set_style(style);
        Atlas::new(&ttf_font, name)
    }
}

impl Glyph {
    /// Create a new glyph
    fn new(rect: Rect, advance: i32) -> Self {
        Self {
            rect: Rectangle {
                pos: vec2(rect.x as f32, rect.y as f32),
                size: vec2(rect.w as f32, rect.h as f32),
            },
            advance: advance as usize,
        }
    }
}

/// A font for tests with hand-made glyphs. They're 8 wide and 16 tall, and each advances 10
/// except `i`, which advances 4. A newline moves down 20. The glyph for byte `b` is cut from
/// `(b - 32) * 8` across the image
#[cfg(test)]
pub(crate) fn test_font(cache_texture: TextureId) -> Font {
    Font {
        cache_texture,
        table: test_table(),
    }
}

#[cfg(test)]
fn test_table() -> GlyphTable {
    let mut glyphs = [Glyph::default(); ASCII_NUM_PRINTABLE_CHARS];
    for (i, glyph) in glyphs.iter_mut().enumerate() {
        glyph.rect = Rectangle::new(i as f32 * 8.0, 0.0, 8.0, 16.0);
        glyph.advance = 10;
    }
    glyphs[(b'i' - ASCII_FIRST_PRINTABLE_CHAR) as usize].advance = 4;
    GlyphTable {
        glyphs,
        height: 16,
        line_skip: 20,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::Handle;

    #[test]
    fn packing_rows_drops_the_padding_at_the_end_of_each() {
        // Two rows of two pixels, each row padded out to 12 bytes
        let padded: Vec<u8> = (0..24).collect();
        let packed = pack_rows(&padded, 2, 2, 12);
        assert_eq!(
            packed,
            [0, 1, 2, 3, 4, 5, 6, 7, 12, 13, 14, 15, 16, 17, 18, 19]
        );
        // Rows with no padding come back unchanged
        assert_eq!(pack_rows(&padded[..16], 2, 2, 8), &padded[..16]);
    }

    #[test]
    fn measuring_adds_up_the_advances_and_one_line_height() {
        assert_eq!(test_table().measure(""), vec2(0.0, 16.0));
        assert_eq!(test_table().measure("abc"), vec2(30.0, 16.0));
        assert_eq!(test_table().measure("ii"), vec2(8.0, 16.0));
    }

    #[test]
    fn each_newline_goes_down_a_line_skip_and_the_widest_line_sets_the_width() {
        assert_eq!(test_table().measure("a\nbc"), vec2(20.0, 36.0));
        assert_eq!(test_table().measure("abc\nd"), vec2(30.0, 36.0));
        assert_eq!(test_table().measure("a\n"), vec2(10.0, 36.0));
        assert_eq!(test_table().measure("a\n\nb"), vec2(10.0, 56.0));
    }

    #[test]
    fn characters_without_a_glyph_are_skipped() {
        let plain = test_table().measure("ab");
        assert_eq!(test_table().measure("a\t\u{e9}\u{20ac}b\u{7f}"), plain);
    }

    #[test]
    fn layout_places_each_glyph_where_the_cursor_is() {
        let mut placed = Vec::new();
        let size = test_table().layout("ab\nc", |at, glyph| placed.push((at, glyph.rect.pos.x)));
        // The glyph for byte b is the one at b - 32, and each is 8 wide
        let glyph_x = |c: u8| (c - ASCII_FIRST_PRINTABLE_CHAR) as f32 * 8.0;
        assert_eq!(
            placed,
            [
                (vec2(0.0, 0.0), glyph_x(b'a')),
                (vec2(10.0, 0.0), glyph_x(b'b')),
                (vec2(0.0, 20.0), glyph_x(b'c')),
            ]
        );
        assert_eq!(size, vec2(20.0, 36.0));
    }

    #[test]
    fn a_font_measures_with_its_table() {
        let font = test_font(Handle::for_tests(0));
        assert_eq!(font.measure("a\nbc"), vec2(20.0, 36.0));
    }

    #[test]
    fn a_font_lays_out_the_part_of_its_image_for_each_glyph() {
        let font = test_font(Handle::for_tests(0));
        let mut placed = Vec::new();
        let size = font.layout("ab\ni", |at, source| placed.push((at, source)));
        let glyph = |byte: u8| {
            let x = (byte - ASCII_FIRST_PRINTABLE_CHAR) as f32 * 8.0;
            Rectangle::new(x, 0.0, 8.0, 16.0)
        };
        assert_eq!(
            placed,
            [
                (vec2(0.0, 0.0), glyph(b'a')),
                (vec2(10.0, 0.0), glyph(b'b')),
                (vec2(0.0, 20.0), glyph(b'i')),
            ]
        );
        assert_eq!(size, vec2(20.0, 36.0));
    }

    #[test]
    fn only_printable_ascii_has_glyphs() {
        let table = test_table();
        for byte in [32, b'a', 126] {
            assert!(table.get_glyph(byte).is_some(), "byte {byte}");
        }
        for byte in [0, 31, 127, 200] {
            assert!(table.get_glyph(byte).is_none(), "byte {byte}");
        }
        let a = table.get_glyph(b'a').unwrap();
        assert_eq!(
            a.rect.pos.x,
            (b'a' - ASCII_FIRST_PRINTABLE_CHAR) as f32 * 8.0
        );
    }
}
