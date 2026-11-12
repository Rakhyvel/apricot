//! A basic hello-world program showing off a spinning 3D cube.
//! WASD  - fly around
//! Space - go up
//! Shift - go down
//! Ctrl  - go faster
//! Tab   - free the cursor
//! P     - pause spinning
//! B     - beep
//! Esc   - quit
//! Move the mouse to look around
//! Run with `cargo run --example spinning_cube`.

use std::f32::consts::FRAC_PI_4;

use apricot::{
    app::{run, App, AppConfig, Scene},
    audio::SoundId,
    camera::ProjectionKind,
    error::Error,
    fly_controller::FlyController,
    input::Scancode,
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
    beep: SoundId,
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
            beep: app.audio.load_sound_from_bytes(&beep_wav()).unwrap(),
            light: DirectionalLightSource::new(vec3(0.5, 1.0, 0.3), 2048).unwrap(),
            checker: renderer.create_texture_rgba(10, 10, &checkerboard(10)),
        }
    }
}

impl Scene for SpinningCube {
    fn update(&mut self, app: &App) {
        if app.input.key_pressed(Scancode::Tab) {
            app.set_cursor_captured(!app.cursor_captured());
        }
        if app.cursor_captured() {
            self.fly.update(app); // the cursor's free for clicking menus otherwise
        }
        app.renderer.set_camera(self.fly.camera());

        if app.input.key_pressed(Scancode::B) {
            app.audio.play_sound(self.beep, 0.5);
        }
        if app.input.key_pressed(Scancode::P) {
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

/// A short 880 Hz beep as the bytes of a .wav file, so the example doesn't need a sound file
fn beep_wav() -> Vec<u8> {
    const RATE: u32 = 44_100;
    let samples: Vec<i16> = (0..RATE / 8)
        .map(|i| {
            let t = i as f32 / RATE as f32;
            let fade = 1.0 - i as f32 / (RATE / 8) as f32; // fade out so it doesn't click
            ((t * 880.0 * std::f32::consts::TAU).sin() * fade * 12_000.0) as i16
        })
        .collect();

    let data_len = samples.len() as u32 * 2;
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + data_len).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes()); // format chunk size
    wav.extend_from_slice(&1u16.to_le_bytes()); // plain PCM
    wav.extend_from_slice(&1u16.to_le_bytes()); // mono
    wav.extend_from_slice(&RATE.to_le_bytes());
    wav.extend_from_slice(&(RATE * 2).to_le_bytes()); // bytes per second
    wav.extend_from_slice(&2u16.to_le_bytes()); // bytes per sample
    wav.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        wav.extend_from_slice(&sample.to_le_bytes());
    }
    wav
}
