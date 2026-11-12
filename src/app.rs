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

use sdl2::event::{Event, WindowEvent};
pub use sdl2::keyboard::Scancode;
use sdl2::video::SwapInterval;
use sdl2::Sdl;

use crate::error::Error;
use crate::rectangle::Rectangle;

use super::render_core::RenderContext;

/// Struct that contains all information about an app, that is passed down to an active `Scene`.
pub struct App {
    // Screen stuff
    /// The current size of the window
    pub window_size: nalgebra_glm::I32Vec2,
    /// The OpenGL rendering context
    pub renderer: RenderContext,

    // Main loop stuff
    /// Whether or not the app is running
    pub running: Cell<bool>,
    /// How many seconds the app has been up
    pub seconds: f64,
    /// How many ticks have occured since the app started
    pub ticks: usize,
    /// The update timestep, defaults to 60 times per second.
    pub dt: Duration,
    /// Frames rendered in the last second
    pub fps: u32,

    // User input state
    /// Static map of key states, where the boolean at index `k` determines if the scancode `k` is currently pressed
    pub keys: [bool; 256],
    keys_pressed: [bool; 256],
    keys_released: [bool; 256],
    /// The position of the mouse, relative to the top-left corner of the screen
    pub mouse_pos: nalgebra_glm::Vec2,
    /// How far the mouse moved this tick, in pixels
    pub mouse_vel: nalgebra_glm::Vec2,
    /// Whether the left mouse button is down
    pub mouse_left_down: bool,
    /// Whether the right mouse button is down
    pub mouse_right_down: bool,
    prev_mouse_left_down: bool,
    prev_mouse_right_down: bool,
    /// Whether the left mouse button was clicked (ie it was down the previous tick, but is now up)
    pub mouse_left_clicked: bool,
    /// Where the left button went down
    left_press_pos: nalgebra_glm::Vec2,
    right_press_pos: nalgebra_glm::Vec2,
    left_drag_dist: f32,
    right_drag_dist: f32,
    /// Whether the left button is down and has moved beyond the click threshold
    pub mouse_left_dragging: bool,
    /// Whether the current left click event has been consumed
    click_consumed: Cell<bool>,
    /// Whether the current mouse wheel event has been consumed
    wheel_consumed: Cell<bool>,
    /// Whether the right mouse button was clicked (ie it was down the previous tick, but is now up)
    pub mouse_right_clicked: bool,
    /// How many notches the mouse wheel turned this tick. Positive is away from you
    pub mouse_wheel: f32,
}

/// A mouse button
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
}

pub struct AppConfig {
    /// The window's title
    pub title: &'static str,
    /// The window's starting size, in pixels
    pub window_size: nalgebra_glm::I32Vec2,
    /// Keep the mouse captured in the window, for mouse-look cameras
    pub mouse_warp: bool,
    /// Whether pressing Escape quits the app
    pub escape_quits: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Apricot app",
            window_size: nalgebra_glm::I32Vec2::new(800, 600),
            mouse_warp: false,
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

    gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);

    window
        .subsystem()
        .gl_set_swap_interval(SwapInterval::VSync)
        .map_err(Error::Sdl)?;

    unsafe {
        gl::Enable(gl::DEPTH_TEST);
        gl::DepthFunc(gl::LESS);
        gl::Enable(gl::CULL_FACE);
        gl::Enable(gl::MULTISAMPLE);
        gl::Enable(gl::BLEND);
        gl::BlendFunc(gl::SRC_ALPHA, gl::ONE_MINUS_SRC_ALPHA);
        // gl::Enable(gl::FRAMEBUFFER_SRGB);
    }

    let mut app = App {
        window_size: cfg.window_size,
        renderer: RenderContext::new()?,
        // sdl_context,
        running: Cell::new(true),
        dt: Duration::from_secs_f64(1.0 / 60.0),
        keys: [false; 256],
        keys_pressed: [false; 256],
        keys_released: [false; 256],
        mouse_pos: nalgebra_glm::vec2(0.0, 0.0),
        mouse_vel: nalgebra_glm::vec2(0.0, 0.0),
        mouse_left_down: false,
        mouse_right_down: false,
        prev_mouse_left_down: false,
        prev_mouse_right_down: false,
        mouse_left_clicked: false,
        mouse_right_clicked: false,
        mouse_wheel: 0.0,
        seconds: 0.0,
        ticks: 0,
        fps: 0,
        click_consumed: Cell::new(false),
        wheel_consumed: Cell::new(false),
        left_press_pos: nalgebra_glm::vec2(0.0, 0.0),
        right_press_pos: nalgebra_glm::vec2(0.0, 0.0),
        left_drag_dist: 0.0,
        right_drag_dist: 0.0,
        mouse_left_dragging: false,
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
    while app.running.get() {
        app.seconds = time.elapsed().as_secs_f64();
        current = time.elapsed();
        elapsed = current - previous;

        previous = current;
        lag += elapsed;

        let scene_stale = false;
        while lag >= app.dt {
            app.reset_input();
            app.poll_input(&sdl_context)?;
            if cfg.escape_quits && app.key_pressed(Scancode::Escape) {
                app.quit();
            }
            if cfg.mouse_warp {
                sdl_context.mouse().warp_mouse_in_window(
                    &window,
                    app.window_size.x / 2,
                    app.window_size.y / 2,
                );
                sdl_context.mouse().set_relative_mouse_mode(true);
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
    /// Mark the current left click as consumed
    pub fn consume_click(&self) {
        self.click_consumed.set(true);
    }

    /// Returns true if the current left click for this tick has been consumed
    pub fn is_click_consumed(&self) -> bool {
        self.click_consumed.get()
    }

    /// Close the app after this frame
    pub fn quit(&self) {
        self.running.set(false);
    }

    /// Whether `key` is held down
    pub fn key_down(&self, key: Scancode) -> bool {
        self.keys[key as usize]
    }

    /// Whether `key` went down this tick. Holding it down only counts once
    pub fn key_pressed(&self, key: Scancode) -> bool {
        self.keys_pressed[key as usize]
    }

    /// Whether `key` came back up this tick
    pub fn key_released(&self, key: Scancode) -> bool {
        self.keys_released[key as usize]
    }

    /// Whether `button` is held down
    pub fn mouse_down(&self, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => self.mouse_left_down,
            MouseButton::Right => self.mouse_right_down,
        }
    }

    pub fn consume_wheel(&self) {
        self.wheel_consumed.set(true);
    }

    pub fn is_wheel_consumed(&self) -> bool {
        self.wheel_consumed.get()
    }

    fn reset_input(&mut self) {
        self.keys_pressed = [false; 256];
        self.keys_released = [false; 256];
        self.mouse_vel = nalgebra_glm::vec2(0.0, 0.0);
        self.mouse_wheel = 0.0;
        self.prev_mouse_left_down = self.mouse_left_down;
        self.prev_mouse_right_down = self.mouse_right_down;
        self.click_consumed.set(false);
        self.wheel_consumed.set(false);
    }

    fn poll_input(&mut self, sdl_context: &Sdl) -> Result<(), Error> {
        let mut event_queue = sdl_context.event_pump().map_err(Error::Sdl)?;
        for event in event_queue.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.running.set(false);
                }

                Event::MouseMotion {
                    x, y, xrel, yrel, ..
                } => {
                    self.mouse_pos = nalgebra_glm::vec2(x as f32, y as f32);
                    self.mouse_vel += nalgebra_glm::vec2(xrel as f32, yrel as f32);
                    // add up, a tick can get several
                }

                Event::MouseButtonDown { mouse_btn, .. } => match mouse_btn {
                    sdl2::mouse::MouseButton::Left => self.mouse_left_down = true,
                    sdl2::mouse::MouseButton::Right => self.mouse_right_down = true,
                    _ => {}
                },

                Event::MouseButtonUp { mouse_btn, .. } => match mouse_btn {
                    sdl2::mouse::MouseButton::Left => self.mouse_left_down = false,
                    sdl2::mouse::MouseButton::Right => self.mouse_right_down = false,
                    _ => {}
                },

                Event::MouseWheel { y, .. } => {
                    self.mouse_wheel += y as f32;
                }

                Event::Window {
                    win_event: WindowEvent::Resized(new_width, new_height),
                    ..
                } => {
                    self.window_size = nalgebra_glm::I32Vec2::new(new_width, new_height);
                    self.renderer.int_screen_resolution = self.window_size;
                    unsafe { gl::Viewport(0, 0, new_width, new_height) };
                }

                Event::KeyDown {
                    scancode: Some(sc),
                    repeat,
                    ..
                } => {
                    self.keys[sc as usize] = true;
                    if !repeat {
                        self.keys_pressed[sc as usize] = true; // held keys repeat, but only count once
                    }
                }

                Event::KeyUp {
                    scancode: Some(sc), ..
                } => {
                    self.keys[sc as usize] = false;
                    self.keys_released[sc as usize] = true;
                }

                _ => {}
            }
        }

        const CLICK_SLOP_PX: f32 = 4.0;
        let left_pressed = !self.prev_mouse_left_down && self.mouse_left_down;
        let right_pressed = !self.prev_mouse_right_down && self.mouse_right_down;
        let left_released = self.prev_mouse_left_down && !self.mouse_left_down;
        let right_released = self.prev_mouse_right_down && !self.mouse_right_down;

        if left_pressed {
            self.left_press_pos = self.mouse_pos;
            self.left_drag_dist = 0.0
        } else if self.mouse_left_down {
            let d = nalgebra_glm::distance(&self.mouse_pos, &self.left_press_pos);
            self.left_drag_dist = self.left_drag_dist.max(d);
        }
        if right_pressed {
            self.right_press_pos = self.mouse_pos;
            self.right_drag_dist = 0.0
        } else if self.mouse_right_down {
            let d = nalgebra_glm::distance(&self.mouse_pos, &self.right_press_pos);
            self.right_drag_dist = self.right_drag_dist.max(d);
        }

        self.mouse_left_dragging = self.mouse_left_down && self.left_drag_dist > CLICK_SLOP_PX;
        self.mouse_left_clicked = left_released && self.left_drag_dist <= CLICK_SLOP_PX;
        self.mouse_right_clicked = right_released && self.right_drag_dist <= CLICK_SLOP_PX;

        Ok(())
    }

    /// Is the mouse over `r`? Anything clipped away by `push_clip` doesn't count
    pub fn mouse_over(&self, r: &Rectangle) -> bool {
        r.contains_point(&self.mouse_pos)
            && self
                .renderer
                .current_clip()
                .is_none_or(|c| c.contains_point(&self.mouse_pos))
    }
}

/// A scene is a something that can be updated, and rendered
pub trait Scene {
    // TODO: Return a "command" enum so that scene's can affect App state
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App);
}
