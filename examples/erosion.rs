//! An island that rain wears down while you watch. Rivers show up where the most water runs.
//! E     - start or stop the rain
//! R     - grow a new island
//! WASD  - fly around
//! Space - go up
//! Shift - go down
//! Ctrl  - go faster
//! Tab   - free the cursor
//! Esc   - quit
//! Move the mouse to look around
//! Run with `cargo run --release --example erosion`, it's slow in debug.

use std::f32::consts::FRAC_PI_4;

use apricot::{
    app::{run, App, AppConfig, Scene},
    camera::ProjectionKind,
    erosion::ErosionSettings,
    error::Error,
    fly_controller::FlyController,
    height_grid::HeightGrid,
    input::Scancode,
    mesh_data::MeshData,
    render_core::{MeshId, TextureId},
    shadow_map::DirectionalLightSource,
};
use nalgebra_glm::{vec2, vec3, vec4, Mat4, Vec2};

/// How many cells across the island is. The grid has one more point than that per side
const CELLS: u32 = 128;
/// How tall the tallest peak is
const HEIGHT: f32 = 30.0;
/// Where the sea sits, from 0 to 1
const SEA_LEVEL: f32 = 0.12;
/// How many raindrops fall each tick while it's raining
const DROPS_PER_TICK: usize = 150;

fn main() -> Result<(), Error> {
    run(
        AppConfig {
            title: "Erosion",
            cursor_captured: true,
            ..Default::default()
        },
        Erosion::new,
    )
}

struct Erosion {
    fly: FlyController,
    light: DirectionalLightSource,
    grid: HeightGrid,
    mesh: MeshId,
    texture: TextureId,
    raining: bool,
    drops: usize,
    seed: i32,
}

impl Erosion {
    fn new(app: &App) -> Self {
        let renderer = &app.renderer;

        let mut fly = FlyController::new(vec3(0.0, 60.0, 110.0), vec3(0.0, 0.0, 0.0));
        fly.speed = 20.0;
        fly.projection = ProjectionKind::Perspective {
            fov_rad: FRAC_PI_4,
            near: 0.5,
            far: 500.0,
        };
        renderer.set_camera(fly.camera());

        let mut light = DirectionalLightSource::new(vec3(0.6, 0.8, 0.3), 2048).unwrap();
        light.shadow_distance = 150.0;
        app.renderer.set_light(Some(&light));

        let seed = 1;
        let grid = island(seed);
        Self {
            fly,
            light,
            mesh: renderer.add_mesh_data(terrain_mesh(&grid), None),
            texture: renderer.create_texture_rgba(CELLS, CELLS, &terrain_colors(&grid)),
            grid,
            raining: false,
            drops: 0,
            seed,
        }
    }

    /// Rebuild the mesh and texture from the grid
    fn refresh(&self, app: &App) {
        app.renderer
            .replace_mesh_data(self.mesh, terrain_mesh(&self.grid));
        app.renderer
            .update_texture_rgba(self.texture, CELLS, CELLS, &terrain_colors(&self.grid));
    }
}

impl Scene for Erosion {
    fn update(&mut self, app: &App) {
        if app.input.key_pressed(Scancode::Tab) {
            app.set_cursor_captured(!app.cursor_captured());
        }
        if app.cursor_captured() {
            self.fly.update(app);
        }
        app.renderer.set_camera(self.fly.camera());

        if app.input.key_pressed(Scancode::E) {
            self.raining = !self.raining;
        }
        if app.input.key_pressed(Scancode::R) {
            self.seed += 1;
            self.grid = island(self.seed);
            self.drops = 0;
            self.refresh(app);
        }
        if self.raining {
            // A fresh seed each tick, so it doesn't rain on the same spots over and over
            self.grid.erode_with(
                DROPS_PER_TICK,
                (self.seed as u64) << 32 | self.drops as u64,
                &ErosionSettings {
                    height_scale: 15.0,
                    friction: 0.05,
                    brush_radius: 0.01,
                    ..Default::default()
                },
            );
            self.drops += DROPS_PER_TICK;
            self.refresh(app);
        }
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;
        let half = CELLS as f32 / 2.0;
        let terrain = nalgebra_glm::translation(&vec3(-half, 0.0, -half));
        let sea: Mat4 = nalgebra_glm::scale(
            &nalgebra_glm::translation(&vec3(0.0, SEA_LEVEL * HEIGHT, 0.0)),
            &vec3(CELLS as f32 * 3.0, 1.0, CELLS as f32 * 3.0),
        );

        renderer.shadow_pass(&mut self.light, |pass| pass.draw(self.mesh, terrain));

        renderer.clear(vec4(0.6, 0.8, 0.95, 1.0));
        renderer.draw_mesh_textured(self.mesh, self.texture, terrain, vec4(1.0, 1.0, 1.0, 1.0));
        // See-through, so drawn after the land
        renderer.draw_mesh(renderer.builtins.plane, sea, vec4(0.15, 0.4, 0.75, 0.7));

        let status = if self.raining { "raining" } else { "dry" };
        renderer.draw_text(
            vec2(10.0, 10.0),
            &format!(
                "{} FPS, {} drops, {status}. E to toggle rain, R for a new island",
                app.fps, self.drops
            ),
            vec4(0.1, 0.1, 0.1, 1.0),
        );
    }
}

/// Noise that fades into the sea toward the edges, stretched to fill 0 to 1
fn island(seed: i32) -> HeightGrid {
    let width = CELLS as usize + 1;
    let mut grid = HeightGrid::from_noise(width, 0.02, 6, seed, 1.0, vec2(0.0, 0.0));
    grid.normalize();

    let center = CELLS as f32 / 2.0;
    for z in 0..width {
        for x in 0..width {
            let p = vec2(x as f32, z as f32);
            let d = nalgebra_glm::distance(&p, &vec2(center, center)) / center;
            let falloff = 1.0 - smoothstep(0.15, 1.0, d);
            grid.set_height(p, grid.height(p) * falloff);
        }
    }
    grid.normalize();
    grid
}

fn smoothstep(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = ((x - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn terrain_mesh(grid: &HeightGrid) -> MeshData {
    MeshData::heightmap(CELLS, CELLS, |x, z| grid.height(vec2(x, z)) * HEIGHT)
}

/// One color per cell by height, turning blue where lots of water has run
fn terrain_colors(grid: &HeightGrid) -> Vec<u8> {
    let cells: Vec<Vec2> = (0..CELLS * CELLS)
        .map(|i| vec2((i % CELLS) as f32, (i / CELLS) as f32))
        .collect();
    // Flow piles up fast along rivers, so compare it on a log scale
    let max_flow = cells.iter().map(|p| grid.flow(*p)).fold(0.0, f32::max);
    let river_scale = (1.0 + max_flow).ln().max(1e-6);

    cells
        .iter()
        .flat_map(|p| {
            let h = grid.height(*p);
            let land = match h {
                h if h < SEA_LEVEL + 0.03 => [220.0, 205.0, 150.0],
                h if h < 0.45 => [90.0, 160.0, 70.0],
                h if h < 0.7 => [120.0, 115.0, 110.0],
                _ => [240.0, 240.0, 245.0],
            };
            let river = smoothstep(0.4, 0.8, (1.0 + grid.flow(*p)).ln() / river_scale);
            let water = [50.0, 110.0, 200.0];
            let mix = |i: usize| (land[i] + (water[i] - land[i]) * river) as u8;
            [mix(0), mix(1), mix(2), 255]
        })
        .collect()
}
