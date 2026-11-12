//! A basic hello-world program showing off a spinning 3D cube.
//! WASD  - fly around
//! Space - go up
//! Shift - go down
//! Ctrl  - go faster
//! Tab   - free the cursor
//! P     - pause spinning
//! Esc   - quit
//! Move the mouse to look around
//! Run with `cargo run --example spinning_cube`.

use std::f32::consts::FRAC_PI_4;

use apricot::{
    app::{run, App, AppConfig, Scancode, Scene},
    camera::ProjectionKind,
    error::Error,
    fly_controller::FlyController,
    render_core::TextureId,
    shadow_map::DirectionalLightSource,
};
use nalgebra_glm::{vec2, vec3, vec4};

fn main() -> Result<(), Error> {
    run(
        AppConfig {
            title: "Spinning Cube",
            cursor_captured: true, // for the fly camera
            ..Default::default()
        },
        SpinningCube::new,
    )
}

struct SpinningCube {
    fly: FlyController,
    angle: f32,
    paused: bool,
    light: DirectionalLightSource,
    checker: TextureId,
}

impl SpinningCube {
    fn new(app: &App) -> Self {
        let renderer = &app.renderer;

        let mut fly = FlyController::new(vec3(0.0, 2.5, 5.0), vec3(0.0, -1.0, 0.0));
        fly.projection = ProjectionKind::Perspective {
            fov_rad: FRAC_PI_4,
            near: 0.1,
            far: 30.0,
        };
        renderer.set_camera(fly.camera());

        Self {
            fly,
            angle: 0.0,
            paused: false,
            light: DirectionalLightSource::new(vec3(0.5, 1.0, 0.3), 2048).unwrap(),
            checker: renderer.create_texture_rgba(10, 10, &checkerboard(10)),
        }
    }
}

impl Scene for SpinningCube {
    fn update(&mut self, app: &App) {
        if app.key_pressed(Scancode::Tab) {
            app.set_cursor_captured(!app.cursor_captured());
        }
        if app.cursor_captured() {
            self.fly.update(app); // the cursor's free for clicking menus otherwise
        }
        app.renderer.set_camera(self.fly.camera());

        if app.key_pressed(Scancode::P) {
            self.paused = !self.paused;
        }
        if !self.paused {
            self.angle += app.dt.as_secs_f32();
        }
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;
        let cube = nalgebra_glm::rotate(
            &nalgebra_glm::identity(),
            self.angle,
            &vec3(0.3, 1.0, 0.2).normalize(),
        );
        let floor = nalgebra_glm::scale(
            &nalgebra_glm::translate(&nalgebra_glm::identity(), &vec3(0.0, -1.5, 0.0)),
            &vec3(10.0, 1.0, 10.0),
        );

        // Render the shadow casters onto the shadow map
        renderer.shadow_pass(&mut self.light, |pass| {
            pass.draw(renderer.builtins.cube, cube);
        });

        // Render the cube and floor
        renderer.clear(vec4(0.53, 0.81, 0.92, 1.0));
        renderer.draw_mesh(renderer.builtins.cube, cube, vec4(1.0, 0.6, 0.25, 1.0));
        renderer.draw_mesh_textured(
            renderer.builtins.plane,
            self.checker,
            floor,
            vec4(1.0, 1.0, 1.0, 1.0),
        );

        renderer.draw_text(
            vec2(10.0, 10.0),
            &format!("{} FPS", app.fps),
            vec4(0.3, 0.3, 0.3, 1.0),
        );
    }
}

/// An `n` by `n` light and dark checkerboard, as RGBA bytes
fn checkerboard(n: u32) -> Vec<u8> {
    (0..n * n)
        .flat_map(|i| {
            if (i % n + i / n) % 2 == 0 {
                [235, 235, 235, 255]
            } else {
                [190, 190, 200, 255]
            }
        })
        .collect()
}
