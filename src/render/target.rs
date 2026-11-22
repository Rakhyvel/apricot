//! Drawing into a texture instead of the screen, for minimaps, mirrors, cameras on screens, etc.

use nalgebra_glm::{I32Vec2, Vec4};

use crate::{
    error::Error,
    opengl::{self, Fbo, Texture},
    render::{assets::TextureId, RenderContext},
};

/// A square texture you can draw the scene into, then draw on screen like any other texture
pub struct RenderTarget {
    fbo: Fbo,
    _depth: Texture, // only kept alive for the framebuffer
    /// What got drawn, for `draw_texture`. Stored bottom row first, like everything GL draws
    pub texture: TextureId,
    pub size: u32,
}

impl RenderContext {
    /// Make a `size` x `size` texture to draw into
    pub fn create_render_target(&self, size: u32) -> Result<RenderTarget, Error> {
        let mut color = Texture::new();
        color.set_pixels(size, size, &vec![0; (size * size * 4) as usize]);
        let mut depth = Texture::new();
        depth.load_depth_buffer(size, size);
        let fbo = Fbo::new();
        fbo.attach_color_depth(&color, &depth)?;
        Ok(RenderTarget {
            fbo,
            _depth: depth,
            texture: self.add_texture(color, None),
            size,
        })
    }

    /// Clear `target` to `clear`, and send what `draw` draws there instead of the screen,
    /// from the current camera. Do shadow passes before this: they draw back to the screen when done
    pub fn render_to_target(&self, target: &RenderTarget, clear: Vec4, draw: impl FnOnce()) {
        let screen = self.screen_size();
        let size = target.size as i32;
        self.set_screen_size(I32Vec2::new(size, size)); // so the camera's aspect matches the target
        target.fbo.bind();
        opengl::set_viewport(size, size);
        opengl::clear(clear);

        draw();

        target.fbo.unbind();
        self.set_screen_size(screen);
        opengl::set_viewport(screen.x, screen.y);
    }
}
