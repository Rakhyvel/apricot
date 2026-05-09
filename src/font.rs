//! This module deals with managing fonts

use nalgebra_glm::{vec2, Vec2};
use sdl2::{pixels::Color, rect::Rect, surface::Surface, ttf::Sdl2TtfContext};
use std::{collections::HashMap, path::Path};

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

/// A cached font
pub struct Font {
    pub cache_texture: Option<TextureId>,
    glyphs: [Glyph; 95], //< All 95 printable ASCII glyphs
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
    pub fn new(font: &sdl2::ttf::Font, renderer: &RenderContext) -> Self {
        let height = font.height() as usize;
        let _ascender = font.ascent() as usize;
        let _descender = font.descent() as usize;
        let line_skip = font.recommended_line_spacing() as usize;
        let mut retval = Self {
            cache_texture: None,
            glyphs: [Glyph::default(); 95],
            height,
            _ascender,
            _descender,
            line_skip,
        };
        retval.pack_gylphs(font, renderer);
        retval
    }

    /// Measure how large the text would be if drawn
    pub fn measure(&self, text: &str) -> Vec2 {
        let mut width: f32 = 0.0;
        let mut max_line_width: f32 = 0.0;
        let mut height: f32 = self.height as f32;

        for c in text.chars() {
            if c == '\n' {
                height += self.line_skip as f32;
                max_line_width = max_line_width.max(width);
                width = 0.0;
                continue;
            }
            if !c.is_ascii_graphic() && !c.is_whitespace() {
                continue;
            }
            let glyph = self.get_glyph(c as u8);
            width += glyph.advance as f32;
        }

        vec2(max_line_width.max(width), height)
    }

    /// Draw text in a font to the screen
    pub fn draw(&self, pos: Vec2, text: &str, renderer: &RenderContext) {
        let mut cursor = pos;
        for c in text.chars() {
            let c_byte: u8 = c as u8;
            if c == '\n' {
                // Handle new lines
                cursor.y += self.line_skip as f32;
                cursor.x = pos.x;
                continue;
            }
            if !c.is_ascii_graphic() && !c.is_whitespace() {
                // Don't print out non-graphic characters
                continue;
            }
            let glyph = self.get_glyph(c_byte);
            let dest_rect = Rectangle {
                pos: cursor,
                size: glyph.rect.size,
            };
            let color_mod = renderer.color.borrow();
            renderer.copy_texture(
                dest_rect,
                self.cache_texture.unwrap(),
                glyph.rect,
                &color_mod,
            );
            cursor.x += glyph.advance as f32;
        }
    }

    fn pack_gylphs(&mut self, font: &sdl2::ttf::Font, renderer: &RenderContext) {
        let atlas_width = self.height * 32;
        let atlas_height = self.height * 8;
        let mut x_offset: usize = 0;
        let mut y_offset: usize = 0;
        let mut mega_surface = Surface::new(
            atlas_width as u32,
            atlas_height as u32,
            sdl2::pixels::PixelFormatEnum::RGBA32,
        )
        .unwrap();
        let _ = mega_surface.set_blend_mode(sdl2::render::BlendMode::None);
        let color = Color {
            r: 255,
            g: 255,
            b: 255,
            a: 255,
        };
        for i in 32..127 {
            let c = char::from(i as u8);
            let s = c.to_string();
            let mut surf = font.render(&s).blended(color).unwrap();

            if x_offset + surf.width() as usize > atlas_width {
                x_offset = 0;
                y_offset += self.height + 4;
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

            let gylph_metrics = font.find_glyph_metrics(c).unwrap();
            self.set_glyph(i, Glyph::new(dest_rect, gylph_metrics.advance));
        }

        let font_texture = Texture::from_surface(mega_surface);
        self.cache_texture = Some(renderer.add_texture(font_texture, None));
    }

    fn set_glyph(&mut self, char: usize, glyph: Glyph) {
        self.glyphs[char - 32] = glyph;
    }

    fn get_glyph(&self, char: u8) -> Glyph {
        self.glyphs[char as usize - 32]
    }
}

impl FontManager {
    /// Create a new font manager
    pub fn new() -> Self {
        let ttf_context = sdl2::ttf::init().unwrap();
        Self {
            ttf_context,
            fonts: vec![],
            keys: HashMap::new(),
        }
    }

    /// Add a new font to the manager and retrieve it's ID
    pub fn add_font(
        &mut self,
        path: &'static str,
        name: &'static str,
        size: u16,
        style: sdl2::ttf::FontStyle,
        renderer: &RenderContext,
    ) -> FontId {
        let id = FontId::new(self.fonts.len());

        // Load font here with self.ttf_context, ensuring it has the same lifetime as FontManager
        let mut ttf_font = self.ttf_context.load_font(Path::new(path), size).unwrap();
        ttf_font.set_style(style);

        // Create a new Font instance with a reference to ttf_font
        let font = Font::new(&ttf_font, renderer);

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
        self.get_font_from_id(self.get_id_from_name(name).unwrap())
    }
}

impl Default for FontManager {
    fn default() -> Self {
        Self::new()
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
