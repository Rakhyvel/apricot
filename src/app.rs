//! This module defines an Apricot App.
//!
//! Apps are made up of a stack of `Scene`s. The `Scene` at the top of the stack is the one that is updated and rendered
//! to the screen. Input information such as the keyboard state and mouse are passed along to the active scene, along
//! with output contexts such as the renderer, which allow the scene to output to the screen.
//!
//! Scenes can either push new scenes onto the stack, or pop themselves off. This allows for fairly intuitive GUI
//! management.

use std::cell::Cell;
use std::time::{Duration, Instant};

use sdl2::event::{Event, EventType, WindowEvent};
use sdl2::video::SwapInterval;
use sdl2::Sdl;

use crate::audio::Audio;
use crate::error::Error;
use crate::input::{Input, Scancode};
use crate::opengl;
use crate::rectangle::Rectangle;

use super::render_core::RenderContext;

/// Struct that contains all information about an app, that is passed down to an active `Scene`.
pub struct App {
    // Screen stuff
    /// The current size of the window
    pub window_size: nalgebra_glm::I32Vec2,
    /// The OpenGL rendering context
    pub renderer: RenderContext,
    /// Sound effects
    pub audio: Audio,

    // Main loop stuff
    /// Whether or not the app is running
    pub running: Cell<bool>,
    cursor_captured: Cell<bool>,
    /// How many seconds the app has been up
    pub seconds: f64,
    /// How many ticks have occured since the app started
    pub ticks: usize,
    /// The update timestep, defaults to 60 times per second.
    pub dt: Duration,
    /// Frames rendered in the last second
    pub fps: u32,

    /// Keyboard and mouse state for this tick
    pub input: Input,
}

pub struct AppConfig {
    /// The window's title
    pub title: &'static str,
    /// The window's starting size, in pixels
    pub window_size: nalgebra_glm::I32Vec2,
    /// Whether the cursor starts out hidden and locked to the window.
    pub cursor_captured: bool,
    /// Whether pressing Escape quits the app
    pub escape_quits: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Apricot app",
            window_size: nalgebra_glm::I32Vec2::new(800, 600),
            cursor_captured: false,
            escape_quits: true,
        }
    }
}

/// Starts a new app, with the scene produced by `init`s the first scene in the stack.
pub fn run<S: Scene + 'static>(cfg: AppConfig, init: impl FnOnce(&App) -> S) -> Result<(), Error> {
    let sdl_context = sdl2::init().map_err(Error::Sdl)?;
    let video_subsystem = sdl_context.video().map_err(Error::Sdl)?;
    let _audio_subsystem = sdl_context.audio().map_err(Error::Sdl)?;

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(3, 3);
    gl_attr.set_double_buffer(true);
    gl_attr.set_multisample_buffers(1);
    gl_attr.set_multisample_samples(4);
    gl_attr.set_depth_size(24);
    gl_attr.set_stencil_size(8); // for model outlines

    let window = video_subsystem
        .window(
            cfg.title,
            cfg.window_size.x as u32,
            cfg.window_size.y as u32,
        )
        .resizable()
        .opengl()
        .build()
        .map_err(|e| Error::Sdl(format!("{e}")))?;

    let _gl_context = window.gl_create_context().map_err(Error::Sdl)?;

    opengl::init(|s| video_subsystem.gl_get_proc_address(s) as *const std::ffi::c_void);

    window
        .subsystem()
        .gl_set_swap_interval(SwapInterval::VSync)
        .map_err(Error::Sdl)?;

    let mut app = App {
        window_size: cfg.window_size,
        renderer: RenderContext::new()?,
        audio: Audio::new(),
        // sdl_context,
        running: Cell::new(true),
        cursor_captured: Cell::new(cfg.cursor_captured),
        dt: Duration::from_secs_f64(1.0 / 60.0),
        input: Input::default(),
        seconds: 0.0,
        ticks: 0,
        fps: 0,
    };
    app.renderer.int_screen_resolution = app.window_size; // so the aspect is right in `init`

    let mut scene_stack: Vec<Box<dyn Scene>> = vec![Box::new(init(&app))];

    let time = Instant::now();
    let mut start = time.elapsed();
    let mut current;
    let mut previous = Duration::ZERO;
    let mut lag = Duration::ZERO;
    let mut elapsed: Duration;
    let mut frames = 0;
    let mut cursor_applied = false;
    while app.running.get() {
        app.seconds = time.elapsed().as_secs_f64();
        current = time.elapsed();
        elapsed = current - previous;

        previous = current;
        lag += elapsed;

        let scene_stale = false;
        while lag >= app.dt {
            let captured = app.cursor_captured();
            if captured != cursor_applied {
                let mouse = sdl_context.mouse();
                mouse.set_relative_mouse_mode(captured);
                if !captured {
                    // Let go in the middle, not wherever it was while hidden
                    let center = app.window_size / 2;
                    mouse.warp_mouse_in_window(&window, center.x, center.y);
                    app.input.mouse_pos = nalgebra_glm::vec2(center.x as f32, center.y as f32);
                }
                // Toss the jump the switch makes, so the camera doesn't lurch
                sdl_context
                    .event()
                    .map_err(Error::Sdl)?
                    .flush_event(EventType::MouseMotion);
                cursor_applied = captured;
            }

            app.poll_events(&sdl_context)?;
            if cfg.escape_quits && app.input.key_pressed(Scancode::Escape) {
                app.quit();
            }
            if let Some(scene) = scene_stack.last_mut() {
                scene.update(&app);
                app.ticks += 1;
            }

            if !scene_stale {
                // if scene isn't stale, purge the scene
                lag -= app.dt;
            } else {
                break;
            }
        }

        if !scene_stale {
            app.renderer.int_screen_resolution = app.window_size;
            if let Some(scene) = scene_stack.last_mut() {
                scene.render(&app);
                frames += 1;
            }
            app.renderer.flush_2d_queue();
            window.gl_swap_window();
        }

        let seconds_since_fps_report = current - start;

        if seconds_since_fps_report >= Duration::from_secs(1) {
            app.fps = (frames as f64 / seconds_since_fps_report.as_secs_f64()).round() as u32;
        }
        if seconds_since_fps_report > Duration::from_secs(5) {
            println!("5 seconds;  fps: {}", frames / 5);
            start = current;
            frames = 0;
        }
    }

    Ok(())
}

impl App {
    /// Capture the cursor (hide it and lock it to the window, for mouse-look) or let it go, like
    /// when opening an inventory. Takes effect next tick
    pub fn set_cursor_captured(&self, captured: bool) {
        self.cursor_captured.set(captured);
    }

    /// Whether the cursor is captured
    pub fn cursor_captured(&self) -> bool {
        self.cursor_captured.get()
    }

    /// Close the app after this frame
    pub fn quit(&self) {
        self.running.set(false);
    }

    /// Is the mouse over `r`? Anything clipped away by `push_clip` doesn't count
    pub fn mouse_over(&self, r: &Rectangle) -> bool {
        let pos = self.input.mouse_pos;
        r.contains_point(&pos)
            && self
                .renderer
                .current_clip()
                .is_none_or(|c| c.contains_point(&pos))
    }

    fn poll_events(&mut self, sdl_context: &Sdl) -> Result<(), Error> {
        self.input.begin_tick();
        let mut event_queue = sdl_context.event_pump().map_err(Error::Sdl)?;
        for event in event_queue.poll_iter() {
            match event {
                Event::Quit { .. } => self.quit(),
                Event::Window {
                    win_event: WindowEvent::Resized(new_width, new_height),
                    ..
                } => {
                    self.window_size = nalgebra_glm::I32Vec2::new(new_width, new_height);
                    self.renderer.int_screen_resolution = self.window_size;
                    opengl::set_viewport(new_width, new_height);
                }
                _ => self.input.handle_event(&event),
            }
        }
        self.input.end_tick();
        Ok(())
    }
}

/// A scene is a something that can be updated, and rendered
pub trait Scene {
    // TODO: Return a "command" enum so that scene's can affect App state
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App);
}
