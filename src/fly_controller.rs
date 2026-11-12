//! A free-flying camera controller for looking around while you build

use crate::{
    app::App,
    camera::{Camera, ProjectionKind},
    input::{MouseButton, Scancode},
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

    /// Turn and fly from this tick's mouse and keyboard input
    pub fn update(&mut self, app: &App) {
        if self.look_button.is_none_or(|b| app.input.mouse_down(b)) {
            self.yaw += app.input.mouse_vel.x * self.sensitivity;
            self.pitch -= app.input.mouse_vel.y * self.sensitivity;
            self.pitch = self.pitch.clamp(-Self::MAX_PITCH, Self::MAX_PITCH);
        }

        let key = |k: Scancode| app.input.key_down(k) as i32 as f32;
        let up = nalgebra_glm::vec3(0.0, 1.0, 0.0);
        let forward = self.forward();
        let right = forward.cross(&up).normalize();
        let mut velocity = forward * (key(Scancode::W) - key(Scancode::S))
            + right * (key(Scancode::D) - key(Scancode::A))
            + up * (key(Scancode::Space) - key(Scancode::LShift));
        if velocity.norm_squared() > 0.0 {
            velocity = velocity.normalize(); // so diagonals aren't faster
        }

        let fast = 1.0 + (self.fast_multiplier - 1.0) * key(Scancode::LCtrl);
        self.position += velocity * self.speed * fast * app.dt.as_secs_f32();
    }

    /// The direction the camera is facing
    pub fn forward(&self) -> Vec3 {
        let (sin_yaw, cos_yaw) = self.yaw.sin_cos();
        let (sin_pitch, cos_pitch) = self.pitch.sin_cos();
        nalgebra_glm::vec3(cos_pitch * sin_yaw, sin_pitch, -cos_pitch * cos_yaw)
    }

    /// The camera to hand to `RenderContext::set_camera`
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

    #[test]
    fn fly_controller_faces_its_target() {
        let from = nalgebra_glm::vec3(1.0, 2.0, 3.0);
        let fly = FlyController::new(from, nalgebra_glm::vec3(4.0, -1.0, 7.0));
        let expected = nalgebra_glm::vec3(3.0, -3.0, 4.0).normalize();
        assert!((fly.forward() - expected).norm() < 1e-5);
    }
}
