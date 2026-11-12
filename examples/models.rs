//! A grid of spinning cubes, UV spheres, and icospheres kept in a `ModelSet`. Hover over one to
//! outline and label it.
//! Run with `cargo run --example models`.

use std::f32::consts::{FRAC_PI_4, TAU};

use apricot::{
    app::{run, App, AppConfig, Scene},
    camera::{Camera, ProjectionKind},
    error::Error,
    mesh_data::MeshData,
    model_set::{Model, ModelSet},
    rectangle::Rectangle,
    render_core::{LinePath, LineStyle, MeshId, TextureId},
    shadow_map::DirectionalLightSource,
};
use nalgebra_glm::{vec2, vec3, vec4, Mat4};

const GRID: i32 = 5;
const SPACING: f32 = 1.6;
/// Icospheres split 0 through this many minus 1 times (20, 80, and 320 faces)
const ICOSPHERE_LEVELS: u32 = 3;

fn main() -> Result<(), Error> {
    run(
        AppConfig {
            title: "Models",
            ..Default::default()
        },
        Models::new,
    )
}

/// The keys for the model set. Any `Copy + Eq + Hash` type works
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
enum Thing {
    Floor,
    Shape(i32, i32),
}

struct Models {
    angle: f32,
    light: DirectionalLightSource,
    models: ModelSet<Thing>,
    hovered: Option<Thing>,
    ring: LinePath,
}

impl Models {
    fn new(app: &App) -> Self {
        let renderer = &app.renderer;

        renderer.set_camera(Camera::new(
            vec3(0.0, 7.0, 9.0),
            vec3(0.0, -1.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            ProjectionKind::Perspective {
                fov_rad: FRAC_PI_4,
                near: 0.1,
                far: 20.0,
            },
        ));

        let mut models = ModelSet::new();

        let checker = renderer.create_texture_rgba(16, 16, &checkerboard(16));
        let floor = nalgebra_glm::scale(
            &nalgebra_glm::translation(&vec3(0.0, -1.0, 0.0)),
            &vec3(12.0, 1.0, 12.0),
        );
        let mut floor = Model::new(renderer.builtins.plane, checker, floor);
        floor.casts_shadow = false; // nothing's under it to shadow
        models.insert(renderer, Thing::Floor, floor);

        let uv_sphere = renderer.add_mesh_data(MeshData::uv_sphere(24, 12), None);
        let icospheres: Vec<MeshId> = (0..ICOSPHERE_LEVELS)
            .map(|n| renderer.add_mesh_data(MeshData::icosphere(n), None))
            .collect();
        let white = renderer.builtins.white_texture;

        for (x, z) in grid() {
            // The checker shows off the UV sphere's texture wrapping, and the spin
            let (mesh, texture): (MeshId, TextureId) = match shape_at(x, z) {
                Shape::Cube => (renderer.builtins.cube, white),
                Shape::UvSphere => (uv_sphere, checker),
                Shape::Icosphere(n) => (icospheres[n as usize], white),
            };
            let mut model = Model::new(mesh, texture, shape_transform(x, z, 0.0));
            // Fade from orange to teal across the grid
            let t = (x + z + 2 * (GRID / 2)) as f32 / (2 * (GRID - 1)) as f32;
            model.tint = vec4(1.0 - 0.7 * t, 0.6 + 0.2 * t, 0.25 + 0.5 * t, 1.0);
            models.insert(renderer, Thing::Shape(x, z), model);
        }

        Self {
            angle: 0.0,
            light: DirectionalLightSource::new(vec3(0.5, 1.0, 0.3), 2048).unwrap(),
            models,
            hovered: None,
            ring: LinePath::new(
                &circle(4.8, 128),
                LineStyle {
                    color: vec4(1.0, 1.0, 1.0, 0.9),
                    width: 5.0,
                    ..Default::default()
                },
            ),
        }
    }

    /// The closest shape under the mouse, if any
    fn hovered_shape(&self, app: &App) -> Option<Thing> {
        let ray = app.renderer.screen_ray(app.input.mouse_pos);
        self.models
            .raycast(&app.renderer, &ray)
            .map(|(key, _)| key)
            .filter(|key| matches!(key, Thing::Shape(..)))
    }
}

impl Scene for Models {
    fn update(&mut self, app: &App) {
        self.angle += app.dt.as_secs_f32();
        self.hovered = self.hovered_shape(app);

        for (x, z) in grid() {
            let key = Thing::Shape(x, z);
            self.models
                .set_transform(&app.renderer, key, shape_transform(x, z, self.angle));
            if let Some(shape) = self.models.get_mut(key) {
                shape.outlined = self.hovered == Some(key);
            }
        }
    }

    fn render(&mut self, app: &App) {
        let renderer = &app.renderer;

        renderer.shadow_pass(&mut self.light, |pass| self.models.draw_shadows(pass));

        renderer.clear(vec4(0.53, 0.81, 0.92, 1.0));
        self.models.draw(renderer);
        self.models
            .draw_outlines(renderer, vec4(1.0, 1.0, 1.0, 1.0));

        // A trail circling the grid, just above the floor
        self.ring.style.seam = (self.angle * 0.2).fract();
        renderer.draw_line_path(
            &self.ring,
            nalgebra_glm::translation(&vec3(0.0, -0.98, 0.0)),
        );

        // Label the hovered shape by projecting a point just above it onto the screen
        if let Some(Thing::Shape(x, z)) = self.hovered {
            let above = vec3(x as f32 * SPACING, 1.0, z as f32 * SPACING);
            if let Some(p) = renderer.world_to_screen(above) {
                let label = format!("{} ({x}, {z})", shape_at(x, z).name());
                let size = renderer.measure_text(&label);
                let text_pos = p - vec2(size.x / 2.0, size.y);

                // Dark box behind the text so it reads over anything
                let pad = vec2(6.0, 3.0);
                renderer.fill_rect(
                    Rectangle {
                        pos: text_pos - pad,
                        size: size + pad * 2.0,
                    },
                    vec4(0.0, 0.0, 0.0, 0.6),
                );
                renderer.draw_text(text_pos, &label, vec4(1.0, 1.0, 1.0, 1.0));
            }
        }

        renderer.draw_text(
            vec2(10.0, 10.0),
            &format!("{} FPS, {} models", app.fps, self.models.len()),
            vec4(0.3, 0.3, 0.3, 1.0),
        );
    }
}

/// Every (x, z) cell of the grid, centered on the origin
fn grid() -> impl Iterator<Item = (i32, i32)> {
    let half = GRID / 2;
    (-half..=half).flat_map(move |x| (-half..=half).map(move |z| (x, z)))
}

/// The kinds of shape in the grid
#[derive(Clone, Copy)]
enum Shape {
    Cube,
    UvSphere,
    /// How many times its faces were split
    Icosphere(u32),
}

impl Shape {
    fn name(self) -> String {
        match self {
            Shape::Cube => "Cube".to_string(),
            Shape::UvSphere => "UV sphere".to_string(),
            Shape::Icosphere(n) => format!("Icosphere, {} faces", 20 * 4u32.pow(n)),
        }
    }
}

/// Which shape sits in a cell, cycling along the diagonals. Icospheres get more detailed as you
/// go along their diagonal
fn shape_at(x: i32, z: i32) -> Shape {
    match (x + z).rem_euclid(3) {
        0 => Shape::Cube,
        1 => Shape::UvSphere,
        _ => Shape::Icosphere(x.rem_euclid(ICOSPHERE_LEVELS as i32) as u32),
    }
}

/// Where a shape sits, spinning a little faster the further it is from the corner
fn shape_transform(x: i32, z: i32, angle: f32) -> Mat4 {
    let speed = 1.0 + 0.15 * (x + z + GRID) as f32;
    let spun = nalgebra_glm::rotate(
        &nalgebra_glm::translation(&vec3(x as f32 * SPACING, 0.0, z as f32 * SPACING)),
        angle * speed,
        &vec3(0.3, 1.0, 0.2).normalize(),
    );
    match shape_at(x, z) {
        Shape::Cube => spun,
        // The spheres have radius 1, so shrink them to match the cube
        _ => nalgebra_glm::scale(&spun, &vec3(0.55, 0.55, 0.55)),
    }
}

/// Points around a flat circle on the XZ plane, ending back where it started
fn circle(radius: f32, segments: u32) -> Vec<nalgebra_glm::Vec3> {
    (0..=segments)
        .map(|i| {
            let angle = i as f32 / segments as f32 * TAU;
            vec3(radius * angle.cos(), 0.0, radius * angle.sin())
        })
        .collect()
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
