//! The app, its main loop, and the stack of scenes it runs.
//!
//! Only the scene on top of the stack gets updated and rendered. Each update returns a `SceneCommand`
//! that can push a new scene on top, like a pause menu, or pop scenes off to go back. Popping the last
//! scene quits the app.

use std::cell::Cell;
use std::time::{Duration, Instant};

use sdl2::event::{Event, WindowEvent};
use sdl2::EventPump;

use crate::audio::Audio;
use crate::error::Error;
use crate::geometry::rectangle::Rectangle;
use crate::input::{Input, Scancode};
use crate::opengl;

use crate::render_core::RenderContext;

mod logging;
mod platform;
mod timing;

use platform::Platform;
use timing::{FixedTimestep, FpsCounter};

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
    running: Cell<bool>,
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

    joystick_subsystem: sdl2::JoystickSubsystem,
    open_joysticks: Vec<sdl2::joystick::Joystick>,
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
    /// The least important log messages that get printed. Ignored if the app already set up its
    /// own logger
    pub log_level: log::LevelFilter,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            title: "Apricot app",
            window_size: nalgebra_glm::I32Vec2::new(800, 600),
            cursor_captured: false,
            escape_quits: true,
            log_level: log::LevelFilter::Warn,
        }
    }
}

/// Start the app with the scene `init` makes as the only one on the stack, and run it until it quits
pub fn run<S: Scene + 'static>(cfg: AppConfig, init: impl FnOnce(&App) -> S) -> Result<(), Error> {
    logging::init(cfg.log_level);
    let mut platform = Platform::new(&cfg)?;

    let mut app = App {
        window_size: cfg.window_size,
        renderer: RenderContext::new()?,
        audio: Audio::new(platform.audio()?),
        running: Cell::new(true),
        cursor_captured: Cell::new(cfg.cursor_captured),
        dt: Duration::from_secs_f64(1.0 / 60.0),
        input: Input::default(),
        seconds: 0.0,
        ticks: 0,
        fps: 0,
        joystick_subsystem: platform.joysticks()?,
        open_joysticks: Vec::new(),
    };
    app.renderer.set_screen_size(app.window_size); // so the aspect is right in `init`

    let mut scenes = SceneStack::new(Box::new(init(&app)));

    let time = Instant::now();
    let mut current;
    let mut previous = Duration::ZERO;
    let mut timestep = FixedTimestep::new(app.dt);
    let mut fps = FpsCounter::new(time.elapsed());
    while app.running.get() {
        app.seconds = time.elapsed().as_secs_f64();
        current = time.elapsed();
        let steps = timestep.advance(current - previous);
        previous = current;

        for _ in 0..steps {
            if let Some(pos) = platform.sync_cursor(app.cursor_captured(), app.window_size)? {
                app.input.mouse_pos = pos;
            }
            app.poll_events(&mut platform.events);
            if cfg.escape_quits && app.input.key_pressed(Scancode::Escape) {
                app.quit();
            }
            if let Some(scene) = scenes.top() {
                let command = scene.update(&app);
                app.ticks += 1;
                scenes.apply(command);
            }
            if scenes.is_empty() {
                app.quit();
                break;
            }
        }

        app.renderer.set_screen_size(app.window_size);
        if let Some(scene) = scenes.top() {
            scene.render(&app);
            fps.frame_drawn();
        }
        app.renderer.flush_2d_queue();
        platform.swap();

        if let Some(rate) = fps.update(current) {
            app.fps = rate;
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

    fn poll_events(&mut self, events: &mut EventPump) {
        self.input.begin_tick();
        for event in events.poll_iter() {
            match event {
                Event::Quit { .. } => self.quit(),
                Event::Window {
                    win_event: WindowEvent::Resized(new_width, new_height),
                    ..
                } => {
                    self.window_size = nalgebra_glm::I32Vec2::new(new_width, new_height);
                    self.renderer.set_screen_size(self.window_size);
                    opengl::set_viewport(new_width, new_height);
                }
                Event::JoyDeviceAdded { which, .. } => {
                    // `which` is a device index here, not an instance id
                    match self.joystick_subsystem.open(which) {
                        Ok(js) => {
                            log::info!(
                                "Joystick connected: {} ({} axes, {} buttons, {} hats)",
                                js.name(),
                                js.num_axes(),
                                js.num_buttons(),
                                js.num_hats()
                            );
                            self.input.joystick_added(&js);
                            self.open_joysticks.push(js);
                        }
                        Err(e) => log::warn!("Couldn't open joystick {which}: {e}"),
                    }
                }
                Event::JoyDeviceRemoved { which, .. } => {
                    // `which` is an instance id here
                    self.open_joysticks.retain(|j| j.instance_id() != which);
                    self.input.joystick_removed(which);
                }
                _ => self.input.handle_event(&event),
            }
        }
        self.input.end_tick();
    }
}

/// Something the app updates and renders. Only the scene on top of the stack runs
pub trait Scene {
    /// Step forward one tick, and say what should happen to the scene stack
    fn update(&mut self, app: &App) -> SceneCommand;
    /// Draw the scene
    fn render(&mut self, app: &App);
}

/// What a scene wants done to the scene stack after its update
pub enum SceneCommand {
    /// Carry on with the same scene
    None,
    /// Put a new scene on top. It's the one that runs until it's popped
    Push(Box<dyn Scene>),
    /// Take this many scenes off the top. The app quits once there are none left
    Pop(usize),
}

impl SceneCommand {
    /// Push `scene`, without having to box it
    pub fn push(scene: impl Scene + 'static) -> Self {
        Self::Push(Box::new(scene))
    }
}

/// The scenes the app is running, with the one that runs on top
struct SceneStack(Vec<Box<dyn Scene>>);

impl SceneStack {
    fn new(first: Box<dyn Scene>) -> Self {
        Self(vec![first])
    }

    fn top(&mut self) -> Option<&mut dyn Scene> {
        self.0
            .last_mut()
            .map(|scene| scene.as_mut() as &mut dyn Scene)
    }

    fn apply(&mut self, command: SceneCommand) {
        match command {
            SceneCommand::None => {}
            SceneCommand::Push(scene) => self.0.push(scene),
            SceneCommand::Pop(n) => self.0.truncate(self.0.len().saturating_sub(n)),
        }
    }

    fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A scene that does nothing, for checking the stack
    struct Blank(#[allow(dead_code)] u8); // boxes of nothing all share one address

    impl Scene for Blank {
        fn update(&mut self, _app: &App) -> SceneCommand {
            SceneCommand::None
        }
        fn render(&mut self, _app: &App) {}
    }

    /// Where a scene lives, so tests can tell which one is on top
    fn addr(scene: &dyn Scene) -> *const () {
        scene as *const dyn Scene as *const ()
    }

    /// A boxed `Blank` and where it lives
    fn blank() -> (Box<dyn Scene>, *const ()) {
        let scene: Box<dyn Scene> = Box::new(Blank(0));
        let at = addr(scene.as_ref());
        (scene, at)
    }

    #[test]
    fn pushing_puts_a_scene_on_top_and_popping_goes_back() {
        let (first, first_at) = blank();
        let (menu, menu_at) = blank();
        let mut scenes = SceneStack::new(first);
        scenes.apply(SceneCommand::None);
        assert_eq!(addr(scenes.top().unwrap()), first_at);

        scenes.apply(SceneCommand::Push(menu));
        assert_eq!(addr(scenes.top().unwrap()), menu_at);

        scenes.apply(SceneCommand::Pop(1));
        assert_eq!(addr(scenes.top().unwrap()), first_at);
        assert!(!scenes.is_empty());
    }

    #[test]
    fn popping_several_goes_back_that_far() {
        let (first, first_at) = blank();
        let mut scenes = SceneStack::new(first);
        for _ in 0..3 {
            scenes.apply(SceneCommand::push(Blank(0)));
        }
        scenes.apply(SceneCommand::Pop(0));
        assert_ne!(
            addr(scenes.top().unwrap()),
            first_at,
            "popping 0 does nothing"
        );
        scenes.apply(SceneCommand::Pop(3));
        assert_eq!(addr(scenes.top().unwrap()), first_at);
    }

    #[test]
    fn popping_everything_empties_the_stack() {
        let (first, _) = blank();
        let mut scenes = SceneStack::new(first);
        scenes.apply(SceneCommand::push(Blank(0)));
        scenes.apply(SceneCommand::Pop(5));
        assert!(scenes.is_empty());
        assert!(scenes.top().is_none());
    }

}
