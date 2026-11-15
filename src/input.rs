//! Keyboard and mouse state, updated once per tick

use std::cell::Cell;

use nalgebra_glm::Vec2;
use sdl2::event::Event;

pub use sdl2::keyboard::Scancode;

/// How far the mouse can wander while a button is down and still count as a click
const CLICK_SLOP_PX: f32 = 4.0;

/// One past the highest scancode SDL can send
const NUM_KEYS: usize = Scancode::Num as usize;

/// A mouse button
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
}

/// Keyboard and mouse state for the current tick
pub struct Input {
    keys: [bool; NUM_KEYS],
    keys_pressed: [bool; NUM_KEYS],
    keys_released: [bool; NUM_KEYS],
    /// Where the mouse is, in pixels from the top left of the window
    pub mouse_pos: Vec2,
    /// How far the mouse moved this tick, in pixels
    pub mouse_vel: Vec2,
    /// How many notches the mouse wheel turned this tick. Positive is away from you
    pub mouse_wheel: f32,
    left: ButtonState,
    right: ButtonState,
    click_consumed: Cell<bool>,
    wheel_consumed: Cell<bool>,
    joysticks: Vec<Joystick>,
}

impl Default for Input {
    fn default() -> Self {
        Self {
            keys: [false; NUM_KEYS],
            keys_pressed: [false; NUM_KEYS],
            keys_released: [false; NUM_KEYS],
            mouse_pos: Vec2::zeros(),
            mouse_vel: Vec2::zeros(),
            mouse_wheel: 0.0,
            left: ButtonState::default(),
            right: ButtonState::default(),
            click_consumed: Cell::new(false),
            wheel_consumed: Cell::new(false),
            joysticks: Vec::new(),
        }
    }
}

impl Input {
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
        self.button(button).down
    }

    /// Whether `button` was clicked this tick: pressed and let go without dragging
    pub fn mouse_clicked(&self, button: MouseButton) -> bool {
        self.button(button).clicked
    }

    /// Whether `button` is held down and the mouse has moved too far for it to be a click
    pub fn mouse_dragging(&self, button: MouseButton) -> bool {
        self.button(button).dragging
    }

    /// Whether there's an unconsumed left click this tick, consuming it if there is.
    pub fn take_click(&self) -> bool {
        let clicked = self.left.clicked && !self.is_click_consumed();
        if clicked {
            self.consume_click();
        }
        clicked
    }

    /// Mark this tick's left click as handled, so nothing else acts on it
    pub fn consume_click(&self) {
        self.click_consumed.set(true);
    }

    /// Whether something already handled this tick's left click
    pub fn is_click_consumed(&self) -> bool {
        self.click_consumed.get()
    }

    /// Mark this tick's mouse wheel as handled, so nothing else acts on it
    pub fn consume_wheel(&self) {
        self.wheel_consumed.set(true);
    }

    /// Whether something already handled this tick's mouse wheel
    pub fn is_wheel_consumed(&self) -> bool {
        self.wheel_consumed.get()
    }

    /// Clear everything that only lasts one tick
    pub(crate) fn begin_tick(&mut self) {
        self.keys_pressed = [false; NUM_KEYS];
        self.keys_released = [false; NUM_KEYS];
        self.mouse_vel = Vec2::zeros();
        self.mouse_wheel = 0.0;
        self.left.begin_tick();
        self.right.begin_tick();
        self.click_consumed.set(false);
        self.wheel_consumed.set(false);
        for js in &mut self.joysticks {
            js.buttons_pressed.fill(false);
        }
    }

    /// Fold one SDL event into the state. Ignores anything that isn't keyboard, mouse, or joystick
    pub(crate) fn handle_event(&mut self, event: &Event) {
        match *event {
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
            Event::MouseMotion {
                x, y, xrel, yrel, ..
            } => {
                self.mouse_pos = nalgebra_glm::vec2(x as f32, y as f32);
                self.mouse_vel += nalgebra_glm::vec2(xrel as f32, yrel as f32); // a tick can get several
                self.left.moved(self.mouse_pos);
                self.right.moved(self.mouse_pos);
            }
            Event::MouseButtonDown {
                mouse_btn, x, y, ..
            } => {
                if let Some(b) = self.sdl_button(mouse_btn) {
                    b.press(nalgebra_glm::vec2(x as f32, y as f32));
                }
            }
            Event::MouseButtonUp {
                mouse_btn, x, y, ..
            } => {
                if let Some(b) = self.sdl_button(mouse_btn) {
                    b.release(nalgebra_glm::vec2(x as f32, y as f32));
                }
            }
            Event::MouseWheel { y, .. } => self.mouse_wheel += y as f32,
            Event::JoyAxisMotion {
                which,
                axis_idx,
                value,
                ..
            } => {
                if let Some(js) = self.joystick_mut(which) {
                    if let Some(a) = js.axes.get_mut(axis_idx as usize) {
                        // i16 runs -32768..32767, so clamp the bottom end
                        *a = (value as f32 / 32767.0).max(-1.0);
                    }
                }
            }
            Event::JoyButtonDown {
                which, button_idx, ..
            } => {
                if let Some(js) = self.joystick_mut(which) {
                    let i = button_idx as usize;
                    if i < js.buttons.len() {
                        js.buttons[i] = true;
                        js.buttons_pressed[i] = true;
                    }
                }
            }
            Event::JoyButtonUp {
                which, button_idx, ..
            } => {
                if let Some(js) = self.joystick_mut(which) {
                    if let Some(b) = js.buttons.get_mut(button_idx as usize) {
                        *b = false;
                    }
                }
            }
            Event::JoyHatMotion {
                which,
                hat_idx,
                state,
                ..
            } => {
                if let Some(js) = self.joystick_mut(which) {
                    if let Some(h) = js.hats.get_mut(hat_idx as usize) {
                        *h = state;
                    }
                }
            }
            _ => {}
        }
    }

    /// Work out this tick's clicks and drags, once all its events are in
    pub(crate) fn end_tick(&mut self) {
        self.left.end_tick();
        self.right.end_tick();
    }

    fn button(&self, button: MouseButton) -> &ButtonState {
        match button {
            MouseButton::Left => &self.left,
            MouseButton::Right => &self.right,
        }
    }

    fn sdl_button(&mut self, button: sdl2::mouse::MouseButton) -> Option<&mut ButtonState> {
        match button {
            sdl2::mouse::MouseButton::Left => Some(&mut self.left),
            sdl2::mouse::MouseButton::Right => Some(&mut self.right),
            _ => None,
        }
    }

    /// The first connected joystick, if any
    pub fn joystick(&self) -> Option<&Joystick> {
        self.joysticks.first()
    }

    /// Every connected joystick
    pub fn joysticks(&self) -> &[Joystick] {
        &self.joysticks
    }

    /// Start tracking a joystick that was just opened
    pub(crate) fn joystick_added(&mut self, js: &sdl2::joystick::Joystick) {
        let id = js.instance_id();
        if self.joysticks.iter().any(|j| j.id == id) {
            return;
        }
        let num_buttons = js.num_buttons() as usize;
        self.joysticks.push(Joystick {
            name: js.name(),
            id,
            axes: vec![0.0; js.num_axes() as usize],
            buttons: vec![false; num_buttons],
            buttons_pressed: vec![false; num_buttons],
            hats: vec![HatState::Centered; js.num_hats() as usize],
        });
    }

    /// Stop tracking a joystick that was unplugged
    pub(crate) fn joystick_removed(&mut self, id: u32) {
        self.joysticks.retain(|j| j.id != id);
    }

    fn joystick_mut(&mut self, id: u32) -> Option<&mut Joystick> {
        self.joysticks.iter_mut().find(|j| j.id == id)
    }
}

/// Tells clicks from drags for one mouse button
#[derive(Debug, Default, Clone, Copy)]
struct ButtonState {
    down: bool,
    released: bool,
    press_pos: Vec2,
    drag_dist: f32,
    clicked: bool,
    dragging: bool,
}

impl ButtonState {
    fn begin_tick(&mut self) {
        self.released = false;
    }

    fn press(&mut self, pos: Vec2) {
        self.down = true;
        self.press_pos = pos;
        self.drag_dist = 0.0;
    }

    fn moved(&mut self, pos: Vec2) {
        if self.down {
            self.drag_dist = self
                .drag_dist
                .max(nalgebra_glm::distance(&pos, &self.press_pos));
        }
    }

    fn release(&mut self, pos: Vec2) {
        self.moved(pos);
        self.down = false;
        self.released = true;
    }

    fn end_tick(&mut self) {
        let wandered = self.drag_dist > CLICK_SLOP_PX;
        self.clicked = self.released && !wandered;
        self.dragging = self.down && wandered;
    }
}

pub use sdl2::joystick::HatState;

/// One connected joystick's state for the current tick
#[derive(Debug, Clone)]
pub struct Joystick {
    /// The name the device reports
    pub name: String,
    /// SDL's instance id, used to route events
    id: u32,
    axes: Vec<f32>,
    buttons: Vec<bool>,
    buttons_pressed: Vec<bool>,
    hats: Vec<HatState>,
}

impl Joystick {
    /// Axis `i` from -1 to 1. Missing axes read 0
    pub fn axis(&self, i: usize) -> f32 {
        self.axes.get(i).copied().unwrap_or(0.0)
    }

    /// Whether button `i` is held down
    pub fn button_down(&self, i: usize) -> bool {
        self.buttons.get(i).copied().unwrap_or(false)
    }

    /// Whether button `i` went down this tick
    pub fn button_pressed(&self, i: usize) -> bool {
        self.buttons_pressed.get(i).copied().unwrap_or(false)
    }

    /// Hat (POV switch) `i`. Missing hats read centered
    pub fn hat(&self, i: usize) -> HatState {
        self.hats.get(i).copied().unwrap_or(HatState::Centered)
    }

    pub fn num_axes(&self) -> usize {
        self.axes.len()
    }

    pub fn num_buttons(&self) -> usize {
        self.buttons.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec2;

    /// Run one tick of a button through `f`
    fn tick(b: &mut ButtonState, f: impl FnOnce(&mut ButtonState)) {
        b.begin_tick();
        f(b);
        b.end_tick();
    }

    #[test]
    fn press_and_release_in_place_is_a_click() {
        let mut b = ButtonState::default();
        tick(&mut b, |b| b.press(vec2(10.0, 10.0)));
        assert!(!b.clicked && !b.dragging);
        tick(&mut b, |b| b.release(vec2(12.0, 11.0)));
        assert!(b.clicked);
        tick(&mut b, |_| {});
        assert!(!b.clicked, "a click only lasts one tick");
    }

    #[test]
    fn a_quick_tap_inside_one_tick_still_clicks() {
        let mut b = ButtonState::default();
        tick(&mut b, |b| {
            b.press(vec2(5.0, 5.0));
            b.release(vec2(5.0, 5.0));
        });
        assert!(b.clicked);
    }

    #[test]
    fn moving_past_the_slop_is_a_drag_not_a_click() {
        let mut b = ButtonState::default();
        tick(&mut b, |b| b.press(vec2(0.0, 0.0)));
        tick(&mut b, |b| b.moved(vec2(20.0, 0.0)));
        assert!(b.dragging);
        // Coming back to where it started doesn't turn it back into a click
        tick(&mut b, |b| b.release(vec2(0.0, 0.0)));
        assert!(!b.clicked && !b.dragging);
    }

    #[test]
    fn keys_go_down_and_up_including_media_keys() {
        let mut input = Input::default();
        let keys = [
            Scancode::A,
            Scancode::Escape,
            Scancode::RGui,
            Scancode::Mode,
            Scancode::AudioPlay,
            Scancode::BrightnessUp,
            Scancode::Sleep,
            Scancode::App2,
        ];
        for sc in keys {
            input.begin_tick();
            input.handle_event(&Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: None,
                scancode: Some(sc),
                keymod: sdl2::keyboard::Mod::NOMOD,
                repeat: false,
            });
            assert!(input.key_down(sc) && input.key_pressed(sc), "{sc:?} down");

            input.begin_tick();
            input.handle_event(&Event::KeyUp {
                timestamp: 0,
                window_id: 0,
                keycode: None,
                scancode: Some(sc),
                keymod: sdl2::keyboard::Mod::NOMOD,
                repeat: false,
            });
            assert!(!input.key_down(sc) && input.key_released(sc), "{sc:?} up");
        }
    }
}
