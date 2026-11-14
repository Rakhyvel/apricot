//! GL settings for a draw, and putting them back afterward

use super::*;

/// How a draw uses the stencil buffer, a hidden per-pixel mark
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stencil {
    /// Ignore the stencil buffer
    Off,
    /// Mark every pixel this draws
    Mark,
    /// Only draw on pixels that haven't been marked
    OutsideMarks,
}

/// GL settings for a draw. Whatever they change gets put back afterward
#[derive(Debug, Clone, Copy)]
pub struct DrawOptions {
    /// Whether things in front hide things behind
    pub depth_test: bool,
    /// Whether this draw hides things drawn after it
    pub depth_write: bool,
    /// Whether triangles facing away aren't drawn
    pub culling: bool,
    /// Whether see-through colors mix with what's behind them
    pub blending: bool,
    /// Whether the vertex shader sets each point's size with `gl_PointSize`
    pub shader_point_size: bool,
    /// How thick lines are, in pixels. Most drivers only support 1
    pub line_width: f32,
    /// How this draw uses the stencil buffer
    pub stencil: Stencil,
    /// Pushes depths back by (slope factor, constant units), so surfaces drawn twice don't flicker
    pub depth_bias: Option<(f32, f32)>,
}

impl Default for DrawOptions {
    fn default() -> Self {
        Self {
            depth_test: true,
            depth_write: true,
            culling: true,
            blending: true,
            shader_point_size: false,
            line_width: 1.0,
            stencil: Stencil::Off,
            depth_bias: None,
        }
    }
}

thread_local! {
    // The options GL is set to right now, so restoring doesn't have to ask the driver
    static CURRENT_OPTIONS: Cell<DrawOptions> = Cell::new(DrawOptions::default());
}

impl DrawOptions {
    /// Switch GL over to these options, giving back the settings to restore afterward
    pub(crate) fn apply(&self) -> DrawOptions {
        let before = CURRENT_OPTIONS.with(Cell::get);
        self.restore();
        before
    }

    /// Switch GL over to these options, without keeping what was there before
    pub(crate) fn restore(&self) {
        self.set();
        CURRENT_OPTIONS.with(|current| current.set(*self));
    }

    fn set(&self) {
        let toggle = |cap: GLenum, on: bool| unsafe {
            if on {
                gl::Enable(cap)
            } else {
                gl::Disable(cap)
            }
        };
        toggle(gl::DEPTH_TEST, self.depth_test);
        toggle(gl::CULL_FACE, self.culling);
        toggle(gl::BLEND, self.blending);
        toggle(gl::PROGRAM_POINT_SIZE, self.shader_point_size);
        toggle(gl::STENCIL_TEST, self.stencil != Stencil::Off);
        toggle(gl::POLYGON_OFFSET_FILL, self.depth_bias.is_some());
        unsafe {
            gl::DepthMask(if self.depth_write {
                gl::TRUE
            } else {
                gl::FALSE
            });
            gl::LineWidth(self.line_width);
            if let Some((factor, units)) = self.depth_bias {
                gl::PolygonOffset(factor, units);
            }
            match self.stencil {
                // Writable, so clearing the screen clears the stencil too
                Stencil::Off => gl::StencilMask(0xFF),
                Stencil::Mark => {
                    gl::StencilFunc(gl::ALWAYS, 1, 0xFF);
                    gl::StencilOp(gl::KEEP, gl::REPLACE, gl::REPLACE);
                    gl::StencilMask(0xFF);
                }
                Stencil::OutsideMarks => {
                    gl::StencilFunc(gl::NOTEQUAL, 1, 0xFF);
                    gl::StencilMask(0x00);
                }
            }
        }
        check_gl();
    }
}
