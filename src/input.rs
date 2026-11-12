//! Keyboard and mouse state, updated once per tick

use std::cell::Cell;

use nalgebra_glm::Vec2;
use sdl2::event::Event;

pub use sdl2::keyboard::Scancode;

/// How far the mouse can wander while a button is down and still count as a click
const CLICK_SLOP_PX: f32 = 4.0;

/// A mouse button
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
}

/// Keyboard and mouse state for the current tick
pub struct Input {
    keys: [bool; 256],
    keys_pressed: [bool; 256],
    keys_released: [bool; 256],
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
}

impl Default for Input {
    fn default() -> Self {
        Self {
            keys: [false; 256],
            keys_pressed: [false; 256],
            keys_released: [false; 256],
            mouse_pos: Vec2::zeros(),
            mouse_vel: Vec2::zeros(),
            mouse_wheel: 0.0,
            left: ButtonState::default(),
            right: ButtonState::default(),
            click_consumed: Cell::new(false),
            wheel_consumed: Cell::new(false),
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
        self.keys_pressed = [false; 256];
        self.keys_released = [false; 256];
        self.mouse_vel = Vec2::zeros();
        self.mouse_wheel = 0.0;
        self.left.begin_tick();
        self.right.begin_tick();
        self.click_consumed.set(false);
        self.wheel_consumed.set(false);
    }

    /// Fold one SDL event into the state. Ignores anything that isn't keyboard or mouse
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
}
