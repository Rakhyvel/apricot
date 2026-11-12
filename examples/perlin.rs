//! Endless perlin noise terrain you can fly around, built a chunk at a time as you get near it.
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
    chunked_map::{ChunkCoord, ChunkGrid},
    error::Error,
    fly_controller::FlyController,
    input::Scancode,
    model_set::{Model, ModelSet},
    perlin,
    render_core::RenderContext,
    shadow_map::DirectionalLightSource,
};
use nalgebra_glm::{vec2, vec3, vec4};

/// How many cells across each chunk is
const CHUNK_CELLS: u32 = 32;
/// How many chunks out from the camera get built
const VIEW_RADIUS: i32 = 6;
/// How many chunks get built per tick, so flying fast doesn't hitch
const CHUNKS_PER_TICK: usize = 2;
/// How tall the tallest peak is
const HEIGHT: f32 = 24.0;
/// Everything below this (0 to 1) gets flattened into lakes
const WATER_LEVEL: f32 = 0.3;
const SEED: i32 = 42;

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
    grid: ChunkGrid,
    chunks: ModelSet<ChunkCoord>,
}

impl Terrain {
    fn new(app: &App) -> Self {
        let mut fly = FlyController::new(vec3(0.0, 40.0, 0.0), vec3(0.0, 20.0, -60.0));
        fly.speed = 15.0;
        fly.projection = ProjectionKind::Perspective {
            fov_rad: FRAC_PI_4,
            near: 0.5,
            far: 400.0,
        };
        app.renderer.set_camera(fly.camera());

        let mut light = DirectionalLightSource::new(vec3(0.6, 0.8, 0.3), 2048).unwrap();
        light.shadow_distance = 120.0;

        Self {
            fly,
            light,
            grid: ChunkGrid::new(CHUNK_CELLS),
            chunks: ModelSet::new(),
        }
    }

    /// Build a chunk's mesh and texture, and add it to the set
    fn build_chunk(&mut self, renderer: &RenderContext, chunk: ChunkCoord) {
        let origin = self.grid.origin(chunk);
        let mesh =
            renderer.add_mesh_data(self.grid.mesh(chunk, |x, z| height(x, z) * HEIGHT), None);
        let colors = terrain_colors(origin.x, origin.z);
        let texture = renderer.create_texture_rgba(CHUNK_CELLS, CHUNK_CELLS, &colors);
        let model = Model::new(mesh, texture, nalgebra_glm::translation(&origin));
        self.chunks.insert(renderer, chunk, model);
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

        // Build the closest missing chunks first
        let missing: Vec<ChunkCoord> = self
            .grid
            .chunks_near(self.fly.position, VIEW_RADIUS)
            .into_iter()
            .filter(|chunk| self.chunks.get(*chunk).is_none())
            .take(CHUNKS_PER_TICK)
            .collect();
        for chunk in missing {
            self.build_chunk(&app.renderer, chunk);
        }
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;

        renderer.shadow_pass(&mut self.light, |pass| self.chunks.draw_shadows(pass));

        renderer.clear(vec4(0.6, 0.8, 0.95, 1.0));
        self.chunks.draw(renderer);

        renderer.draw_text(
            vec2(10.0, 10.0),
            &format!("{} FPS, {} chunks", app.fps, self.chunks.len()),
            vec4(0.1, 0.1, 0.1, 1.0),
        );
    }
}

/// How high the ground is at a world position, from `WATER_LEVEL` at the lakes to a bit past 1 on
/// the tallest peaks. Noise at world coordinates means neighboring chunks always line up
fn height(x: f32, z: f32) -> f32 {
    let n = perlin::noise(x, z, 0.02, 6, SEED);
    // Most noise lands near the middle, so stretch it out, then flatten the low parts into lakes.
    // No cap on top, or the peaks get sliced flat
    ((n - 0.35) / 0.3).max(WATER_LEVEL)
}

/// One color per cell of the chunk starting at (`x0`, `z0`), by how high it is. The heightmap
/// stretches its texture over the whole chunk, so each texel lands on exactly one cell
fn terrain_colors(x0: f32, z0: f32) -> Vec<u8> {
    (0..CHUNK_CELLS * CHUNK_CELLS)
        .flat_map(|i| {
            let x = x0 + (i % CHUNK_CELLS) as f32;
            let z = z0 + (i / CHUNK_CELLS) as f32;
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
