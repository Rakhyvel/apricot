//! Endless perlin noise terrain you can fly around, built a chunk at a time as you get near it,
//! with trees scattered over the flatter grassy parts.
//! WASD  - fly around
//! Space - go up
//! Shift - go down
//! Ctrl  - go faster
//! Tab   - free the cursor
//! Esc   - quit
//! Move the mouse to look around
//! Run with `cargo run --example perlin`.

use std::{collections::HashSet, f32::consts::FRAC_PI_4};

use apricot::{
    app::{run, App, AppConfig, Scene},
    camera::ProjectionKind,
    chunked_map::{ChunkCoord, ChunkGrid},
    error::Error,
    fly_controller::FlyController,
    input::Scancode,
    mesh_data::MeshData,
    model_set::{Model, ModelSet},
    noise,
    rectangle::Rectangle,
    render_core::{MeshId, RenderContext},
    shadow_map::DirectionalLightSource,
    terrain::{normal_at, scatter, surface_height},
};
use nalgebra_glm::{vec2, vec3, vec4};

/// How many cells across each chunk is
const CHUNK_CELLS: u32 = 32;
/// How many chunks out from the camera get built
const VIEW_RADIUS: i32 = 6;
/// Chunks farther than this get unloaded. A little past the view, so turning around doesn't rebuild them
const UNLOAD_RADIUS: i32 = VIEW_RADIUS + 2;
/// How many chunks get built per tick, so flying fast doesn't hitch
const CHUNKS_PER_TICK: usize = 2;
/// How tall the tallest peak is
const HEIGHT: f32 = 24.0;
/// Everything below this (0 to 1) gets flattened into lakes
const WATER_LEVEL: f32 = 0.3;
const SEED: i32 = 42;
/// How many spots per chunk get tried for a tree
const TREE_ATTEMPTS: u32 = 60;
/// How close two trees can be
const TREE_SPACING: f32 = 4.0;
/// How steep the ground can be for a tree. 0 is flat, 1 is a cliff
const TREE_MAX_SLOPE: f32 = 0.25;

/// Everything in the model set
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Thing {
    Chunk(ChunkCoord),
    Trunk(ChunkCoord, usize),
    Leaves(ChunkCoord, usize),
}

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
    models: ModelSet<Thing>,
    trunk: MeshId,
    leaves: MeshId,
    chunk_count: usize,
    /// The chunk the camera was over last tick
    last_chunk: Option<ChunkCoord>,
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
            models: ModelSet::new(),
            trunk: app.renderer.builtins.cube,
            leaves: app.renderer.add_mesh_data(MeshData::icosphere(1), None),
            chunk_count: 0,
            last_chunk: None,
        }
    }

    /// Build a chunk's ground and trees, and add them to the set
    fn build_chunk(&mut self, renderer: &RenderContext, chunk: ChunkCoord) {
        let origin = self.grid.origin(chunk);
        let mesh = renderer.add_mesh_data(self.grid.mesh(chunk, ground), None);
        let colors = terrain_colors(origin.x, origin.z);
        let texture = renderer.create_texture_rgba(CHUNK_CELLS, CHUNK_CELLS, &colors);
        let model = Model::new(mesh, texture, nalgebra_glm::translation(&origin));
        self.models.insert(renderer, Thing::Chunk(chunk), model);
        self.chunk_count += 1;

        // Trees on grass that isn't too steep. Seeding by chunk means a chunk always grows the same trees
        let size = self.grid.chunk_size();
        let area = Rectangle::new(origin.x, origin.z, size, size);
        let seed = ((chunk.0 as u64) << 32) ^ (chunk.1 as u32 as u64);
        let spots = scatter(area, TREE_ATTEMPTS, TREE_SPACING, seed, |p| {
            let h = height(p.x, p.y);
            let slope = 1.0 - normal_at(ground, p.x, p.y).y;
            h > WATER_LEVEL + 0.05 && h < 0.65 && slope < TREE_MAX_SLOPE
        });

        let white = renderer.builtins.white_texture;
        for (i, p) in spots.into_iter().enumerate() {
            // A little size variety, so it doesn't look like a tree farm
            let scale = 0.8 + 0.6 * noise::perlin(p.x, p.y, 0.7, 1, SEED + 1);
            let base = vec3(p.x, surface_height(ground, p.x, p.y), p.y);

            let trunk = nalgebra_glm::scale(
                &nalgebra_glm::translation(&(base + vec3(0.0, 0.75 * scale, 0.0))),
                &(vec3(0.3, 1.5, 0.3) * scale),
            );
            let mut trunk = Model::new(self.trunk, white, trunk);
            trunk.tint = vec4(0.45, 0.3, 0.2, 1.0);
            self.models.insert(renderer, Thing::Trunk(chunk, i), trunk);

            let leaves = nalgebra_glm::scale(
                &nalgebra_glm::translation(&(base + vec3(0.0, 2.6 * scale, 0.0))),
                &(vec3(1.2, 1.8, 1.2) * scale),
            );
            let mut leaves = Model::new(self.leaves, white, leaves);
            leaves.tint = vec4(0.2, 0.5, 0.2, 1.0);
            self.models
                .insert(renderer, Thing::Leaves(chunk, i), leaves);
        }
    }

    /// Unload every chunk, and its trees, too far from `pos`
    fn unload_far_chunks(&mut self, renderer: &RenderContext, pos: nalgebra_glm::Vec3) {
        let keep: HashSet<ChunkCoord> = self
            .grid
            .chunks_near(pos, UNLOAD_RADIUS)
            .into_iter()
            .collect();
        let far: Vec<Thing> = self
            .models
            .iter()
            .map(|(key, _)| key)
            .filter(|key| match key {
                Thing::Chunk(c) | Thing::Trunk(c, _) | Thing::Leaves(c, _) => !keep.contains(c),
            })
            .collect();
        for key in far {
            let Some(model) = self.models.remove(key) else {
                continue;
            };
            // Trees share their meshes, but each chunk's ground mesh and texture are its own
            if let Thing::Chunk(_) = key {
                renderer.unload_mesh(model.mesh);
                renderer.unload_texture(model.texture);
                self.chunk_count -= 1;
            }
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

        let here = self.grid.chunk_at(self.fly.position);
        if self.last_chunk != Some(here) {
            self.last_chunk = Some(here);
            self.unload_far_chunks(&app.renderer, self.fly.position);
        }

        // Build the closest missing chunks first
        let missing: Vec<ChunkCoord> = self
            .grid
            .chunks_near(self.fly.position, VIEW_RADIUS)
            .into_iter()
            .filter(|chunk| self.models.get(Thing::Chunk(*chunk)).is_none())
            .take(CHUNKS_PER_TICK)
            .collect();
        for chunk in missing {
            self.build_chunk(&app.renderer, chunk);
        }
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;

        renderer.shadow_pass(&mut self.light, |pass| self.models.draw_shadows(pass));

        renderer.clear(vec4(0.6, 0.8, 0.95, 1.0));
        self.models.draw(renderer);

        renderer.draw_text(
            vec2(10.0, 10.0),
            &format!(
                "{} FPS, {} chunks, {} models",
                app.fps,
                self.chunk_count,
                self.models.len()
            ),
            vec4(0.1, 0.1, 0.1, 1.0),
        );
    }
}

/// How high the ground is at a world position, from `WATER_LEVEL` at the lakes to a bit past 1 on
/// the tallest peaks. Noise at world coordinates means neighboring chunks always line up
fn height(x: f32, z: f32) -> f32 {
    let n = noise::perlin(x, z, 0.02, 6, SEED);
    // Most noise lands near the middle, so stretch it out, then flatten the low parts into lakes.
    // No cap on top, or the peaks get sliced flat
    ((n - 0.35) / 0.3).max(WATER_LEVEL)
}

/// The ground's actual height in world units, what the meshes are built from
fn ground(x: f32, z: f32) -> f32 {
    height(x, z) * HEIGHT
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
