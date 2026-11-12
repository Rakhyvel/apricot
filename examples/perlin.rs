//! Perlin noise terrain you can fly around.
//! WASD  - fly around
//! Space - go up
//! Shift - go down
//! Ctrl  - go faster
//! Tab   - free the cursor
//! Esc   - quit
//! Move the mouse to look around
//! Run with `cargo run --example perlin`.

use std::f32::consts::FRAC_PI_4;

use apricot::{
    app::{run, App, AppConfig, Scene},
    camera::ProjectionKind,
    error::Error,
    fly_controller::FlyController,
    input::Scancode,
    mesh_data::MeshData,
    perlin::PerlinMap,
    render_core::{MeshId, TextureId},
    shadow_map::DirectionalLightSource,
};
use nalgebra_glm::{vec2, vec3, vec4};

/// How many cells across the terrain is. The map has one more point than that per side
const CELLS: u32 = 128;
/// How tall the tallest peak is
const HEIGHT: f32 = 24.0;
/// Everything below this (0 to 1) gets flattened into a lake
const WATER_LEVEL: f32 = 0.3;

fn main() -> Result<(), Error> {
    run(
        AppConfig {
            title: "Perlin Terrain",
            cursor_captured: true,
            ..Default::default()
        },
        Terrain::new,
    )
}

struct Terrain {
    fly: FlyController,
    light: DirectionalLightSource,
    mesh: MeshId,
    texture: TextureId,
}

impl Terrain {
    fn new(app: &App) -> Self {
        let renderer = &app.renderer;

        // Noise, stretched to fill 0 to 1, with the low parts flattened into a lake
        let mut map = PerlinMap::new(CELLS as usize + 1);
        map.generate(0.03, 6, 42, 1.0, vec2(0.0, 0.0));
        map.normalize();
        map.create_shelf(WATER_LEVEL, WATER_LEVEL);
        let height = |x: f32, z: f32| map.height(vec2(x, z));

        let mut fly = FlyController::new(vec3(0.0, 40.0, 90.0), vec3(0.0, 0.0, 0.0));
        fly.speed = 15.0;
        fly.projection = ProjectionKind::Perspective {
            fov_rad: FRAC_PI_4,
            near: 0.5,
            far: 400.0,
        };
        renderer.set_camera(fly.camera());

        let mut light = DirectionalLightSource::new(vec3(0.6, 0.8, 0.3), 2048).unwrap();
        light.shadow_distance = 120.0;

        Self {
            fly,
            light,
            mesh: renderer.add_mesh_data(
                MeshData::heightmap(CELLS, CELLS, |x, z| height(x, z) * HEIGHT),
                None,
            ),
            texture: renderer.create_texture_rgba(CELLS, CELLS, &terrain_colors(&height)),
        }
    }
}

impl Scene for Terrain {
    fn update(&mut self, app: &App) {
        if app.input.key_pressed(Scancode::Tab) {
            app.set_cursor_captured(!app.cursor_captured());
        }
        if app.cursor_captured() {
            self.fly.update(app);
        }
        app.renderer.set_camera(self.fly.camera());
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;
        // The heightmap starts at the origin, so slide it over to center it
        let half = CELLS as f32 / 2.0;
        let transform = nalgebra_glm::translation(&vec3(-half, 0.0, -half));

        renderer.shadow_pass(&mut self.light, |pass| pass.draw(self.mesh, transform));

        renderer.clear(vec4(0.6, 0.8, 0.95, 1.0));
        renderer.draw_mesh_textured(self.mesh, self.texture, transform, vec4(1.0, 1.0, 1.0, 1.0));

        renderer.draw_text(
            vec2(10.0, 10.0),
            &format!("{} FPS", app.fps),
            vec4(0.1, 0.1, 0.1, 1.0),
        );
    }
}

/// One color per cell by how high it is. The heightmap stretches its texture over the whole grid, so each texel lands on exactly one cell
fn terrain_colors(height: &impl Fn(f32, f32) -> f32) -> Vec<u8> {
    (0..CELLS * CELLS)
        .flat_map(|i| {
            let (x, z) = ((i % CELLS) as f32, (i / CELLS) as f32);
            let corners = [
                height(x, z),
                height(x + 1.0, z),
                height(x, z + 1.0),
                height(x + 1.0, z + 1.0),
            ];
            let highest = corners.iter().copied().fold(0.0, f32::max);
            if highest <= WATER_LEVEL + 0.001 {
                return [60, 120, 190, 255]; // all four corners are on the lake
            }
            match corners.iter().sum::<f32>() / 4.0 {
                h if h < 0.38 => [220, 205, 150, 255],
                h if h < 0.65 => [90, 160, 70, 255],
                h if h < 0.82 => [120, 115, 110, 255],
                _ => [240, 240, 245, 255],
            }
        })
        .collect()
}
