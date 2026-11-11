//! A basic hello-world program showing off a spinning cube.
//! Run with `cargo run --example spinning_cube`.

use std::{cell::RefCell, f32::consts::FRAC_PI_4};

use apricot::{
    app::{run, App, AppConfig, Scene},
    camera::{Camera, ProjectionKind},
    shadow_map::DirectionalLightSource,
};
use nalgebra_glm::{vec3, vec4};

fn main() -> Result<(), String> {
    run(
        nalgebra_glm::I32Vec2::new(800, 600),
        "Spinning Cube",
        AppConfig { mouse_warp: false },
        &|app| RefCell::new(Box::new(SpinningCube::new(app))),
    )
}

struct SpinningCube {
    angle: f32,
    light: DirectionalLightSource,
}

impl SpinningCube {
    fn new(app: &App) -> Self {
        let renderer = &app.renderer;

        renderer.set_camera(Camera::new(
            vec3(0.0, 2.5, 5.0),
            vec3(0.0, -0.5, 0.0),
            vec3(0.0, 1.0, 0.0),
            ProjectionKind::Perspective {
                fov_rad: FRAC_PI_4,
                far: 10.0,
            },
            app.window_size.x as f32 / app.window_size.y as f32,
        ));

        Self {
            angle: 0.0,
            light: DirectionalLightSource::new(vec3(0.5, 1.0, 0.3), 2048),
        }
    }
}

impl Scene for SpinningCube {
    fn update(&mut self, app: &App) {
        self.angle += 0.1 * app.dt.as_secs_f32();
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;
        let cube = nalgebra_glm::rotate(
            &nalgebra_glm::identity(),
            self.angle,
            &vec3(0.3, 1.0, 0.2).normalize(),
        );
        let floor = nalgebra_glm::scale(
            &nalgebra_glm::translate(&nalgebra_glm::identity(), &vec3(0.0, -1.0, 0.0)),
            &vec3(10.0, 1.0, 10.0),
        );

        // Render the shadow casters onto the shadow map
        renderer.shadow_pass(&mut self.light, |pass| {
            pass.draw(renderer.builtins.cube, cube);
        });

        // Render the cube and floor
        renderer.set_color(vec4(0.53, 0.81, 0.92, 1.0));
        renderer.clear();
        renderer.draw_mesh(renderer.builtins.cube, cube, vec4(1.0, 0.6, 0.25, 1.0));
        renderer.draw_mesh(renderer.builtins.plane, floor, vec4(1.0, 1.0, 1.0, 1.0));
    }
}
