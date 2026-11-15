//! A free-flying camera controller for looking around while you build

use std::time::Duration;

use crate::{
    camera::{Camera, ProjectionKind},
    input::{Input, MouseButton, Scancode},
};
use nalgebra_glm::Vec3;

/// A free-flying controller. Move the mouse to turn, WASD to fly,
/// Space and Shift to rise and sink, Ctrl to go faster. Works best with the cursor captured.
#[derive(Debug, Clone)]
pub struct FlyController {
    pub position: Vec3,
    /// Angle around the vertical axis, in radians. 0 looks down -Z
    pub yaw: f32,
    /// Angle above the horizon, in radians. Kept just short of straight up or down
    pub pitch: f32,
    /// Units per second
    pub speed: f32,
    /// How many times faster it goes while Ctrl is held
    pub fast_multiplier: f32,
    /// Radians turned per pixel the mouse moves
    pub sensitivity: f32,
    /// A mouse button to hold to look around, for when the mouse isn't captured. `None` always looks
    pub look_button: Option<MouseButton>,
    /// The projection the camera uses
    pub projection: ProjectionKind,
}

impl FlyController {
    const MAX_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.01;

    /// A camera at `position` looking toward `target`
    pub fn new(position: Vec3, target: Vec3) -> Self {
        let dir = (target - position).normalize();
        Self {
            position,
            yaw: dir.x.atan2(-dir.z),
            pitch: dir.y.asin(),
            speed: 5.0,
            fast_multiplier: 4.0,
            sensitivity: 0.003,
            look_button: None,
            projection: ProjectionKind::default(),
        }
    }

    /// Turn and fly from this tick's mouse and keyboard `input`, over `dt`
    pub fn update(&mut self, input: &Input, dt: Duration) {
        if self.look_button.is_none_or(|b| input.mouse_down(b)) {
            self.yaw += input.mouse_vel.x * self.sensitivity;
            self.pitch -= input.mouse_vel.y * self.sensitivity;
            self.pitch = self.pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
        }

        let key = |k: Scancode| input.key_down(k) as i32 as f32;
        let up = nalgebra_glm::vec3(0.0, 1.0, 0.0);
        let forward = nalgebra_glm::vec3(self.yaw.sin(), 0.0, -self.yaw.cos());
        let right = forward.cross(&up);
        let mut velocity = forward * (key(Scancode::W) - key(Scancode::S))
            + right * (key(Scancode::D) - key(Scancode::A))
            + up * (key(Scancode::Space) - key(Scancode::LShift));
        if velocity.norm_squared() > 0.0 {
            velocity = velocity.normalize(); // so diagonals aren't faster
        }

        let fast = 1.0 + (self.fast_multiplier - 1.0) * key(Scancode::LCtrl);
        self.position += velocity * self.speed * fast * dt.as_secs_f32();
    }

    /// The direction the camera is facing
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        nalgebra_glm::vec3(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw)
    }

    /// A camera at the controller's position, looking the way it faces
    pub fn camera(&self) -> Camera {
        Camera::new(
            self.position,
            self.position + self.forward(),
            nalgebra_glm::vec3(0.0, 1.0, 0.0),
            self.projection,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use nalgebra_glm::{vec2, vec3, Vec2};
    use sdl2::event::Event;

    const SECOND: Duration = Duration::from_secs(1);

    /// One tick of input with `keys` held, the right mouse button held or not, and the mouse
    /// moved by `mouse_vel`
    fn input(keys: &[Scancode], right_held: bool, mouse_vel: Vec2) -> Input {
        let mut input = Input::default();
        input.begin_tick();
        for &key in keys {
            input.handle_event(&Event::KeyDown {
                timestamp: 0,
                window_id: 0,
                keycode: None,
                scancode: Some(key),
                keymod: sdl2::keyboard::Mod::NOMOD,
                repeat: false,
            });
        }
        if right_held {
            input.handle_event(&Event::MouseButtonDown {
                timestamp: 0,
                window_id: 0,
                which: 0,
                mouse_btn: sdl2::mouse::MouseButton::Right,
                clicks: 1,
                x: 0,
                y: 0,
            });
        }
        input.end_tick();
        input.mouse_vel = mouse_vel;
        input
    }

    /// At the origin looking down -z
    fn at_origin() -> FlyController {
        FlyController::new(Vec3::zeros(), vec3(0.0, 0.0, -1.0))
    }

    fn keys(held: &[Scancode]) -> Input {
        input(held, false, Vec2::zeros())
    }

    #[test]
    fn keys_fly_forward_back_sideways_and_up_at_its_speed() {
        let cases = [
            (Scancode::W, vec3(0.0, 0.0, -5.0)),
            (Scancode::S, vec3(0.0, 0.0, 5.0)),
            (Scancode::D, vec3(5.0, 0.0, 0.0)),
            (Scancode::A, vec3(-5.0, 0.0, 0.0)),
            (Scancode::Space, vec3(0.0, 5.0, 0.0)),
            (Scancode::LShift, vec3(0.0, -5.0, 0.0)),
        ];
        for (key, moved) in cases {
            let mut fly = at_origin();
            fly.update(&keys(&[key]), SECOND);
            assert!(
                (fly.position - moved).norm() < 1e-5,
                "{key:?} went to {:?}",
                fly.position
            );
        }
    }

    #[test]
    fn diagonals_arent_faster_and_ctrl_is() {
        let mut fly = at_origin();
        fly.update(&keys(&[Scancode::W, Scancode::D]), SECOND);
        assert!((fly.position.norm() - 5.0).abs() < 1e-5);

        let mut fly = at_origin();
        fly.update(
            &keys(&[Scancode::W, Scancode::LCtrl]),
            Duration::from_millis(500),
        );
        assert!(
            (fly.position - vec3(0.0, 0.0, -10.0)).norm() < 1e-5,
            "{:?}",
            fly.position
        );
    }

    #[test]
    fn it_flies_level_the_way_it_faces_even_looking_up() {
        let mut fly = FlyController::new(Vec3::zeros(), vec3(1.0, 1.0, 0.0));
        fly.update(&keys(&[Scancode::W]), SECOND);
        assert!(
            (fly.position - vec3(5.0, 0.0, 0.0)).norm() < 1e-5,
            "{:?}",
            fly.position
        );
    }

    #[test]
    fn the_mouse_turns_it_and_pitch_stops_short_of_straight_up() {
        let mut fly = at_origin();
        fly.update(&input(&[], false, vec2(100.0, -50.0)), SECOND);
        assert!((fly.yaw - 0.3).abs() < 1e-6);
        assert!(
            (fly.pitch - 0.15).abs() < 1e-6,
            "moving the mouse up looks up"
        );
        assert_eq!(fly.position, Vec3::zeros(), "no keys held, so it stays put");

        fly.update(&input(&[], false, vec2(0.0, -10_000.0)), SECOND);
        assert_eq!(fly.pitch, FlyController::MAX_PITCH);
        assert!(
            fly.pitch < std::f32::consts::FRAC_PI_2,
            "straight up breaks the camera"
        );
        fly.update(&input(&[], false, vec2(0.0, 20_000.0)), SECOND);
        assert_eq!(fly.pitch, -FlyController::MAX_PITCH);
    }

    #[test]
    fn with_a_look_button_it_only_turns_while_its_held() {
        let mut fly = at_origin();
        fly.look_button = Some(MouseButton::Right);
        fly.update(&input(&[], false, vec2(100.0, 0.0)), SECOND);
        assert_eq!(fly.yaw, 0.0);
        fly.update(&input(&[], true, vec2(100.0, 0.0)), SECOND);
        assert!((fly.yaw - 0.3).abs() < 1e-6);
    }

    #[test]
    fn its_camera_sits_where_it_is_and_looks_where_it_faces() {
        let mut fly = FlyController::new(vec3(1.0, 2.0, 3.0), vec3(1.0, 2.0, -7.0));
        fly.projection = ProjectionKind::Orthographic {
            left: -1.0,
            right: 1.0,
            bottom: -1.0,
            top: 1.0,
            near: 0.1,
            far: 10.0,
        };
        let camera = fly.camera();
        assert_eq!(camera.position(), vec3(1.0, 2.0, 3.0));
        assert!((camera.lookat() - camera.position() - fly.forward()).norm() < 1e-6);
        assert_eq!(camera.up(), vec3(0.0, 1.0, 0.0));
        assert!(matches!(
            camera.projection_kind,
            ProjectionKind::Orthographic { .. }
        ));
    }

    #[test]
    fn fly_controller_faces_its_target() {
        let from = nalgebra_glm::vec3(1.0, 2.0, 3.0);
        let fly = FlyController::new(from, nalgebra_glm::vec3(4.0, -1.0, 7.0));
        let expected = nalgebra_glm::vec3(3.0, -3.0, 4.0).normalize();
        assert!((fly.forward() - expected).norm() < 1e-5);
    }
}
