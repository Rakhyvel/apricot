//! This module deals with managing fonts

use nalgebra_glm::{vec2, Vec2, Vec4};
use sdl2::{pixels::Color, rect::Rect, surface::Surface, ttf::Sdl2TtfContext};
use std::{collections::HashMap, path::Path};

use crate::error::Error;

use super::{
    opengl::Texture,
    rectangle::Rectangle,
    render_core::{OpaqueId, RenderContext, TextureId},
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

/// A cached font
pub struct Font {
    pub cache_texture: TextureId,
    glyphs: [Glyph; ASCII_NUM_PRINTABLE_CHARS], //< All 95 printable ASCII glyphs
    height: usize,
    _ascender: usize,
    _descender: usize,
    line_skip: usize, //< Used for newlines, the amount of pixels to increment Y cursor by
}

/// Opaque type used by a FontManager to associate fonts.
#[derive(Copy, Clone)]
/// An opaque reference id to a font within the manager
pub struct FontId(usize);

/// Manages the loading and rendering of fonts
pub struct FontManager {
    ttf_context: Sdl2TtfContext,
    fonts: Vec<Font>,                    //< List of fonts
    keys: HashMap<&'static str, FontId>, //< Maps font names to ids in the font list
}

impl Font {
    /// Create a new font
    pub fn new(font: &sdl2::ttf::Font, renderer: &RenderContext) -> Result<Self, Error> {
        let height = font.height() as usize;
        let _ascender = font.ascent() as usize;
        let _descender = font.descent() as usize;
        let line_skip = font.recommended_line_spacing() as usize;
        let (cache_texture, glyphs) = pack_glyphs(font, height, renderer)?;
        Ok(Self {
            cache_texture,
            glyphs,
            height,
            _ascender,
            _descender,
            line_skip,
        })
    }

    /// Measure how large the text would be if drawn
    pub fn measure(&self, text: &str) -> Vec2 {
        self.layout(text, |_, _| {})
    }

    /// Draw text in a font to the screen
    pub fn draw(&self, pos: Vec2, text: &str, renderer: &RenderContext, color: Vec4) {
        self.layout(text, |offset, glyph| {
            let dest = Rectangle {
                pos: pos + offset,
                size: glyph.rect.size,
            };
            renderer.copy_texture(dest, self.cache_texture, glyph.rect, &color);
        });
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

/// Render every printable ASCII glyph into one atlas texture
fn pack_glyphs(
    font: &sdl2::ttf::Font,
    height: usize,
    renderer: &RenderContext,
) -> Result<(TextureId, [Glyph; ASCII_NUM_PRINTABLE_CHARS]), Error> {
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

    let font_texture = Texture::from_surface(mega_surface);
    Ok((renderer.add_texture(font_texture, None), glyphs))
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
            fonts: vec![],
            keys: HashMap::new(),
        })
    }

    /// Add a new font from a .ttf/.otf to the manager and retrieve it's ID
    pub fn add_font(
        &mut self,
        path: &'static str,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
        renderer: &RenderContext,
    ) -> Result<FontId, Error> {
        let font = {
            let mut ttf_font =
                self.ttf_context
                    .load_font(Path::new(path), size)
                    .map_err(|msg| Error::Font {
                        font: format!("{name}"),
                        msg,
                    })?;
            ttf_font.set_style(style);
            Font::new(&ttf_font, renderer)?
        };
        Ok(self.push(font, name))
    }

    /// Add a font from .ttf/.otf bytes in memory
    pub fn add_font_from_bytes(
        &mut self,
        bytes: &[u8],
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
        renderer: &RenderContext,
    ) -> Result<FontId, Error> {
        let font = {
            let rwops = sdl2::rwops::RWops::from_bytes(bytes).map_err(|msg| Error::Font {
                font: format!("{name}"),
                msg,
            })?;
            let mut ttf_font =
                self.ttf_context
                    .load_font_from_rwops(rwops, size)
                    .map_err(|msg| Error::Font {
                        font: format!("{name}"),
                        msg,
                    })?;
            ttf_font.set_style(style);
            Font::new(&ttf_font, renderer)?
        };
        Ok(self.push(font, name))
    }

    fn push(&mut self, font: Font, name: &'static str) -> FontId {
        let id = FontId::new(self.fonts.len());
        self.fonts.push(font);
        self.keys.insert(name, id);
        id
    }

    /// Retrieve a font from it's ID
    pub fn get_font_from_id(&self, id: FontId) -> Option<&Font> {
        self.fonts.get(id.as_usize())
    }

    /// Get a font ID from it's name
    pub fn get_id_from_name(&self, name: &'static str) -> Option<FontId> {
        self.keys.get(name).copied()
    }

    /// Get a font from it's name
    pub fn get_font(&self, name: &'static str) -> Option<&Font> {
        self.get_font_from_id(self.get_id_from_name(name)?)
    }
}

impl OpaqueId for FontId {
    fn new(id: usize) -> Self {
        FontId(id)
    }

    fn as_usize(&self) -> usize {
        self.0
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
