//! SDL, the window, and the GL context it draws with

use nalgebra_glm::{vec2, I32Vec2, Vec2};
use sdl2::{
    event::EventType,
    video::{GLContext, SwapInterval, Window},
    AudioSubsystem, EventPump, JoystickSubsystem, Sdl,
};

use super::AppConfig;
use crate::{error::Error, opengl};

/// The window, with a GL context ready to draw into, and where its events come from
pub(super) struct Platform {
    // The context goes before the window, so it's dropped first
    _gl_context: GLContext,
    window: Window,
    pub(super) events: EventPump,
    /// Whether the cursor is captured right now, which can lag behind what the app asked for
    cursor_captured: bool,
    sdl: Sdl,
}

impl Platform {
    /// Open a window from `cfg` with a GL 3.3 core context, and load GL
    pub(super) fn new(cfg: &AppConfig) -> Result<Self, Error> {
        let sdl = sdl2::init().map_err(Error::Sdl)?;
        let video = sdl.video().map_err(Error::Sdl)?;

        let gl_attr = video.gl_attr();
        gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
        gl_attr.set_context_version(3, 3);
        gl_attr.set_double_buffer(true);
        gl_attr.set_multisample_buffers(1);
        gl_attr.set_multisample_samples(2);
        gl_attr.set_depth_size(24);
        gl_attr.set_stencil_size(8); // for model outlines

        let window = video
            .window(
                cfg.title,
                cfg.window_size.x as u32,
                cfg.window_size.y as u32,
            )
            .resizable()
            .opengl()
            .build()
            .map_err(|e| Error::Sdl(format!("{e}")))?;
        let gl_context = window.gl_create_context().map_err(Error::Sdl)?;
        opengl::init(|s| video.gl_get_proc_address(s) as *const std::ffi::c_void);
        video
            .gl_set_swap_interval(SwapInterval::VSync)
            .map_err(Error::Sdl)?;

        Ok(Self {
            _gl_context: gl_context,
            window,
            events: sdl.event_pump().map_err(Error::Sdl)?,
            cursor_captured: false,
            sdl,
        })
    }

    pub(super) fn audio(&self) -> Result<AudioSubsystem, Error> {
        self.sdl.audio().map_err(Error::Sdl)
    }

    pub(super) fn joysticks(&self) -> Result<JoystickSubsystem, Error> {
        self.sdl.joystick().map_err(Error::Sdl)
    }

    /// Capture or let go of the cursor to match `captured`, if it doesn't already. Gives back
    /// where the mouse was put if it was let go
    pub(super) fn sync_cursor(
        &mut self,
        captured: bool,
        window_size: I32Vec2,
    ) -> Result<Option<Vec2>, Error> {
        if captured == self.cursor_captured {
            return Ok(None);
        }
        self.cursor_captured = captured;
        let mouse = self.sdl.mouse();
        mouse.set_relative_mouse_mode(captured);
        let mut let_go_at = None;
        if !captured {
            // Let go in the middle, not wherever it was while hidden
            let center = window_size / 2;
            mouse.warp_mouse_in_window(&self.window, center.x, center.y);
            let_go_at = Some(vec2(center.x as f32, center.y as f32));
        }
        // Toss the jump the switch makes, so the camera doesn't lurch
        self.sdl
            .event()
            .map_err(Error::Sdl)?
            .flush_event(EventType::MouseMotion);
        Ok(let_go_at)
    }

    /// Show what was drawn this frame
    pub(super) fn swap(&self) {
        self.window.gl_swap_window();
    }
}
