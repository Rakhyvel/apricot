//! This module defines an Apricot App.
//!
//! Apps are made up of a stack of `Scene`s. The `Scene` at the top of the stack is the one that is updated and rendered
//! to the screen. Input information such as the keyboard state and mouse are passed along to the active scene, along
//! with output contexts such as the renderer, which allow the scene to output to the screen.
//!
//! Scenes can either push new scenes onto the stack, or pop themselves off. This allows for fairly intuitive GUI
//! management.

use std::cell::RefCell;
use std::time::Instant;

use sdl2::event::{Event, WindowEvent};
use sdl2::keyboard::Scancode;
use sdl2::sys::{SDL_GetPerformanceCounter, SDL_GetPerformanceFrequency};
use sdl2::video::SwapInterval;
use sdl2::Sdl;

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
    pub running: bool,
    /// How many seconds the app has been up
    pub seconds: f32,
    /// How many ticks have occured since the app started
    pub ticks: usize,

    // User input state
    /// Static map of key states, where the boolean at index `k` determines if the scancode `k` is currently pressed
    pub keys: [bool; 256],
    /// The position of the mouse, relative to the top-left corner of the screen
    pub mouse_pos: nalgebra_glm::Vec2,
    /// The relative motion of the mouse
    pub mouse_vel: nalgebra_glm::Vec2,
    /// Whether the left mouse button is down
    pub mouse_left_down: bool,
    /// Whether the right mouse button is down
    pub mouse_right_down: bool,
    prev_mouse_left_down: bool,
    prev_mouse_right_down: bool,
    /// Whether the left mouse button was clicked (ie it was down the previous tick, but is now up)
    pub mouse_left_clicked: bool,
    /// Whether the right mouse button was clicked (ie it was down the previous tick, but is now up)
    pub mouse_right_clicked: bool,
    /// The motion of the mouse wheel
    pub mouse_wheel: f32,
}

/// Starts a new app, with the `init` scene as the first scene in the stack.
pub fn run(
    window_size: nalgebra_glm::I32Vec2,
    window_title: &'static str,
    init: &dyn Fn(&App) -> RefCell<Box<dyn Scene>>,
) -> Result<(), String> {
    let sdl_context = sdl2::init()?;
    let video_subsystem = sdl_context.video()?;
    let _audio_subsystem = sdl_context.audio()?;

    let gl_attr = video_subsystem.gl_attr();
    gl_attr.set_context_profile(sdl2::video::GLProfile::Core);
    gl_attr.set_context_version(3, 3);
    gl_attr.set_double_buffer(true);

    let window = video_subsystem
        .window(window_title, window_size.x as u32, window_size.y as u32)
        .resizable()
        .opengl()
        .build()
        .unwrap();

    let _gl_context = window.gl_create_context().unwrap();

    let _gl =
        gl::load_with(|s| video_subsystem.gl_get_proc_address(s) as *const std::os::raw::c_void);

    window
        .subsystem()
        .gl_set_swap_interval(SwapInterval::VSync)
        .unwrap();

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
        window_size,
        renderer: RenderContext::new(),
        // sdl_context,
        running: true,
        keys: [false; 256],
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
    };

    let initial_scene = init(&app);
    let mut scene_stack: Vec<RefCell<Box<dyn Scene>>> = vec![];
    scene_stack.push(initial_scene);

    let time = Instant::now();
    let mut start = time.elapsed().as_millis();
    let mut current;
    let mut previous = 0;
    let mut lag = 0;
    let mut elapsed;
    let mut frames = 0;
    const DELTA_T: u128 = 16;
    while app.running {
        app.seconds = time.elapsed().as_secs_f32();
        current = time.elapsed().as_millis();
        elapsed = current - previous;

        previous = current;
        lag += elapsed;

        let scene_stale = false;
        while lag >= DELTA_T {
            app.reset_input();
            app.poll_input(&sdl_context);
            // sdl_context.mouse().warp_mouse_in_window(
            //     &window,
            //     app.screen_width / 2,
            //     app.screen_height / 2,
            // );
            // sdl_context.mouse().set_relative_mouse_mode(true);

            if let Some(scene_ref) = scene_stack.last() {
                scene_ref.borrow_mut().update(&app);
                app.ticks += 1;
            }

            if !scene_stale {
                // if scene isn't stale, purge the scene
                lag -= DELTA_T;
            } else {
                break;
            }
        }

        if !scene_stale {
            app.renderer.int_screen_resolution = app.window_size;
            if let Some(scene_ref) = scene_stack.last() {
                scene_ref.borrow_mut().render(&app);
                frames += 1;
            }
            window.gl_swap_window();
        }

        let end = unsafe { SDL_GetPerformanceCounter() };
        let freq = unsafe { SDL_GetPerformanceFrequency() };
        let seconds = (end as f64 - (start as f64)) / (freq as f64);
        if seconds > 5.0 {
            println!("5 seconds;  fps: {}", frames / 5);
            start = end as u128;
            frames = 0;
        }
    }

    Ok(())
}

impl App {
    fn reset_input(&mut self) {
        self.mouse_vel = nalgebra_glm::vec2(0.0, 0.0);
        self.mouse_wheel = 0.0;
        self.prev_mouse_left_down = self.mouse_left_down;
        self.prev_mouse_right_down = self.mouse_right_down;
    }

    fn poll_input(&mut self, sdl_context: &Sdl) {
        let mut event_queue = sdl_context.event_pump().unwrap();
        for event in event_queue.poll_iter() {
            match event {
                Event::Quit { .. } => {
                    self.running = false;
                }

                Event::MouseMotion {
                    x, y, xrel, yrel, ..
                } => {
                    self.mouse_pos = nalgebra_glm::vec2(x as f32, y as f32);
                    self.mouse_vel = nalgebra_glm::vec2(xrel as f32, yrel as f32);
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
                    self.mouse_wheel = y as f32;
                }

                Event::Window { win_event, .. } => {
                    if let WindowEvent::Resized(new_width, new_height) = win_event {
                        self.window_size = nalgebra_glm::I32Vec2::new(new_width, new_height)
                    }
                }

                Event::KeyDown { scancode, .. } => match scancode {
                    Some(sc) => {
                        self.keys[sc as usize] = true;
                        if self.keys[Scancode::Escape as usize] {
                            self.running = false
                        }
                    }
                    None => {}
                },

                Event::KeyUp { scancode, .. } => match scancode {
                    Some(sc) => self.keys[sc as usize] = false,
                    None => {}
                },

                _ => {}
            }
        }

        self.mouse_left_clicked = !self.prev_mouse_left_down && self.mouse_left_down;
        self.mouse_right_clicked = !self.prev_mouse_right_down && self.mouse_right_down;
    }
}

/// A scene is a something that can be updated, and rendered
pub trait Scene {
    // TODO: Return a "command" enum so that scene's can affect App state
    fn update(&mut self, app: &App);
    fn render(&mut self, app: &App);
}
