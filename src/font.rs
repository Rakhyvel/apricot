//! This module deals with managing fonts

use nalgebra_glm::{vec2, Vec2, Vec4};
use sdl2::{pixels::Color, rect::Rect, surface::Surface, ttf::Sdl2TtfContext};
use std::path::Path;

use crate::error::Error;

use super::{
    geometry::rectangle::Rectangle,
    registry::{Handle, Registry},
    render_core::{RenderContext, TextureId},
};

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
    pub cache_texture: TextureId,
    table: GlyphTable,
}

/// Refers to a font added to the renderer
pub type FontId = Handle<Font>;

/// Manages the loading and rendering of fonts
pub struct FontManager {
    ttf_context: Sdl2TtfContext,
    fonts: Registry<Font>,
}

impl Font {
    /// Measure how large the text would be if drawn
    pub fn measure(&self, text: &str) -> Vec2 {
        self.table.measure(text)
    }

    /// Draw text in a font to the screen
    pub fn draw(&self, pos: Vec2, text: &str, renderer: &RenderContext, color: Vec4) {
        self.table.layout(text, |offset, glyph| {
            let dest = Rectangle {
                pos: pos + offset,
                size: glyph.rect.size,
            };
            renderer.copy_texture(dest, self.cache_texture, glyph.rect, &color);
        });
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
    fn new(font: &sdl2::ttf::Font) -> Result<Self, Error> {
        let height = font.height() as usize;
        let (image, glyphs) = pack_glyphs(font, height)?;
        Ok(Self {
            image,
            table: GlyphTable {
                glyphs,
                height,
                line_skip: font.recommended_line_spacing() as usize,
            },
        })
    }

    /// The finished font, once `upload` turns the atlas image into a texture
    pub(crate) fn into_font(self, upload: impl FnOnce(Surface<'static>) -> TextureId) -> Font {
        Font {
            cache_texture: upload(self.image),
            table: self.table,
        }
    }
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
) -> Result<(Surface<'static>, [Glyph; ASCII_NUM_PRINTABLE_CHARS]), Error> {
    let mut glyphs = [Glyph::default(); ASCII_NUM_PRINTABLE_CHARS];
    let mut x_offset: usize = 0;
    let mut y_offset: usize = 0;
    let mut mega_surface = Surface::new(
        height as u32 * 16,
        height as u32 * 16,
        sdl2::pixels::PixelFormatEnum::RGBA32,
    )
    .map_err(Error::Sdl)?;
    let _ = mega_surface.set_blend_mode(sdl2::render::BlendMode::None);
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
            .map_err(|e| Error::Sdl(format!("{e}")))?;
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
        let _ = surf.set_blend_mode(sdl2::render::BlendMode::None);
        let _ = surf.blit(Some(src_rect), &mut mega_surface, Some(dest_rect));
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

impl FontManager {
    /// Create a new font manager
    pub fn new() -> Result<Self, Error> {
        let ttf_context = sdl2::ttf::init().map_err(|msg| Error::Sdl(format!("{msg}")))?;
        Ok(Self {
            ttf_context,
            fonts: Registry::new(),
        })
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
        Atlas::new(&ttf_font)
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
        Atlas::new(&ttf_font)
    }

    /// Add a finished font under `name`
    pub(crate) fn add(&mut self, font: Font, name: &'static str) -> FontId {
        self.fonts.add(font, Some(name))
    }

    /// Take the font out of the manager, giving it back
    pub fn remove(&mut self, id: FontId) -> Option<Font> {
        self.fonts.remove(id)
    }

    /// Retrieve a font from it's ID
    pub fn get_font_from_id(&self, id: FontId) -> Option<&Font> {
        self.fonts.get(id)
    }

    /// Get a font ID from it's name
    pub fn get_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.fonts.lookup(name)
    }

    /// Get a font from it's name
    pub fn get_font(&self, name: &'static str) -> Option<&Font> {
        self.get_font_from_id(self.get_id_from_name(name)?)
    }
}

impl Glyph {
    /// Create a new glyph
    pub fn new(rect: Rect, advance: i32) -> Self {
        Self {
            rect: Rectangle {
                pos: vec2(rect.x as f32, rect.y as f32),
                size: vec2(rect.w as f32, rect.h as f32),
            },
            advance: advance as usize,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Glyphs 8 wide and 16 tall that each advance 10, except `i` which advances 4. A newline
    /// moves down 20
    fn table() -> GlyphTable {
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

    #[test]
    fn measuring_adds_up_the_advances_and_one_line_height() {
        assert_eq!(table().measure(""), vec2(0.0, 16.0));
        assert_eq!(table().measure("abc"), vec2(30.0, 16.0));
        assert_eq!(table().measure("ii"), vec2(8.0, 16.0));
    }

    #[test]
    fn each_newline_goes_down_a_line_skip_and_the_widest_line_sets_the_width() {
        assert_eq!(table().measure("a\nbc"), vec2(20.0, 36.0));
        assert_eq!(table().measure("abc\nd"), vec2(30.0, 36.0));
        assert_eq!(table().measure("a\n"), vec2(10.0, 36.0));
        assert_eq!(table().measure("a\n\nb"), vec2(10.0, 56.0));
    }

    #[test]
    fn characters_without_a_glyph_are_skipped() {
        let plain = table().measure("ab");
        assert_eq!(table().measure("a\t\u{e9}\u{20ac}b\u{7f}"), plain);
    }

    #[test]
    fn layout_places_each_glyph_where_the_cursor_is() {
        let mut placed = Vec::new();
        let size = table().layout("ab\nc", |at, glyph| placed.push((at, glyph.rect.pos.x)));
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
        let font = Font {
            cache_texture: Handle::for_tests(0),
            table: table(),
        };
        assert_eq!(font.measure("a
bc"), vec2(20.0, 36.0));
    }

    #[test]
    fn only_printable_ascii_has_glyphs() {
        let table = table();
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
