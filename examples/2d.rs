//! Every 2D drawing function in a grid. Shapes that can rotate spin, and a few sizes pulse.
//! Cells marked "(todo)" are planned functions that don't exist yet.
//! Run with `cargo run --example 2d`.

use std::f32::consts::TAU;

use apricot::{
    app::{run, App, AppConfig, Scene},
    error::Error,
    mesh_data::MeshData,
    rectangle::Rectangle,
    render2d::NineSlice,
    render_core::{Mesh, MeshId, TextureId},
};
use nalgebra_glm::{vec2, vec4, Vec2};

const COLUMNS: usize = 4;
const ROWS: usize = 4;

/// Nine-slice texture size and border, in pixels
const FRAME_SIZE: u32 = 48;
const FRAME_BORDER: u32 = 16;

fn main() -> Result<(), Error> {
    run(
        AppConfig {
            title: "2D Shapes",
            window_size: vec2(1000, 600),
            ..Default::default()
        },
        Shapes::new,
    )
}

struct Shapes {
    checker: TextureId,
    frame: TextureId,
    star: MeshId,
}

impl Shapes {
    fn new(app: &App) -> Self {
        let renderer = &app.renderer;
        Self {
            checker: renderer.create_texture_rgba(8, 8, &checkerboard(8)),
            frame: renderer.create_texture_rgba(
                FRAME_SIZE,
                FRAME_SIZE,
                &frame(FRAME_SIZE, FRAME_BORDER),
            ),
            star: renderer.add_mesh(Mesh::from_data(&star(5, 0.45)), None),
        }
    }
}

/// A cell's label, and how to draw its demo centered on a point (`None` = not implemented yet)
type Cell<'a> = (&'static str, Option<Box<dyn Fn(Vec2) + 'a>>);

impl Scene for Shapes {
    fn update(&mut self, _app: &App) {}

    fn render(&mut self, app: &App) {
        let r = &app.renderer;
        let t = app.seconds as f32;
        let spin = t; // radians per second
        let pulse = 0.5 + 0.5 * (t * 2.0).sin(); // 0..1

        let cell = vec2(
            app.window_size.x as f32 / COLUMNS as f32,
            app.window_size.y as f32 / ROWS as f32,
        );
        let size = cell.x.min(cell.y) * 0.3; // half the width of each demo

        let orange = vec4(1.0, 0.6, 0.25, 1.0);
        let teal = vec4(0.3, 0.8, 0.75, 1.0);
        let pink = vec4(0.95, 0.45, 0.6, 1.0);
        let yellow = vec4(0.95, 0.85, 0.35, 1.0);
        let violet = vec4(0.6, 0.5, 0.95, 1.0);
        let white = vec4(1.0, 1.0, 1.0, 1.0);
        let label_color = vec4(0.85, 0.85, 0.85, 1.0);
        let todo_color = vec4(0.45, 0.45, 0.5, 1.0);
        let grid_color = vec4(0.25, 0.25, 0.3, 1.0);

        let cells: Vec<Cell> = vec![
            (
                "fill_rect",
                Some(Box::new(|c| {
                    r.fill_rect(
                        Rectangle::new(c.x - size, c.y - size * 0.6, size * 2.0, size * 1.2),
                        orange,
                    )
                })),
            ),
            (
                "draw_rect",
                Some(Box::new(|c| {
                    r.draw_rect(
                        Rectangle::new(c.x - size, c.y - size * 0.6, size * 2.0, size * 1.2),
                        3.0,
                        teal,
                    )
                })),
            ),
            (
                "draw_poly (3)",
                Some(Box::new(|c| r.draw_poly(c, 3, size, spin, pink))),
            ),
            (
                "draw_poly (6)",
                Some(Box::new(|c| r.draw_poly(c, 6, size, -spin, yellow))),
            ),
            (
                "draw_poly_lines",
                Some(Box::new(|c| {
                    r.draw_poly_lines(c, 5, size, spin, 4.0, violet)
                })),
            ),
            (
                "draw_poly_lines (pulse)",
                Some(Box::new(|c| {
                    let thickness = 1.0 + (size - 1.0) * pulse;
                    r.draw_poly_lines(c, 8, size, -spin * 0.5, thickness, orange)
                })),
            ),
            (
                "draw_ring",
                Some(Box::new(|c| {
                    r.draw_ring(c, size * (0.3 + 0.6 * pulse), size, teal)
                })),
            ),
            (
                "fill_polygon (custom mesh)",
                Some(Box::new(|c| {
                    r.fill_polygon(self.star, c, size, spin, yellow)
                })),
            ),
            (
                "copy_texture (tinted)",
                Some(Box::new(|c| {
                    r.copy_texture(
                        Rectangle::new(c.x - size, c.y - size, size * 2.0, size * 2.0),
                        self.checker,
                        Rectangle::new(0.0, 0.0, 8.0, 8.0),
                        &vec4(1.0, 0.5 + 0.5 * pulse, 1.0 - 0.5 * pulse, 1.0),
                    )
                })),
            ),
            (
                "render_nine_slice",
                Some(Box::new(|c| {
                    // Shrinks below two borders wide, to show the corners clamping
                    let w = size * 2.0 * (0.15 + 0.85 * pulse);
                    r.render_nine_slice(
                        NineSlice {
                            texture: self.frame,
                            border: FRAME_BORDER as f32,
                        },
                        Rectangle::new(c.x - w / 2.0, c.y - size * 0.8, w, size * 1.6),
                    )
                })),
            ),
            (
                "draw_text",
                Some(Box::new(|c| {
                    let text = "Hello,\napricot!";
                    let half = r.get_current_font().measure(text) / 2.0;
                    let color = vec4(1.0, 0.6 + 0.4 * pulse, 0.25 + 0.75 * pulse, 1.0);
                    r.draw_text(c - half, text, color)
                })),
            ),
            (
                "set_draw_layer",
                Some(Box::new(|c| {
                    // Drawn first but on a higher layer, so it ends up on top
                    r.set_draw_layer(1);
                    r.fill_rect(
                        Rectangle::new(c.x - size * 0.2, c.y - size * 0.2, size, size),
                        pink,
                    );
                    r.set_draw_layer(0);
                    r.fill_rect(
                        Rectangle::new(c.x - size * 0.8, c.y - size * 0.8, size, size),
                        violet,
                    );
                })),
            ),
            ("draw_line", None),
            ("fill_circle", None),
            ("draw_texture", None),
        ];

        r.clear(vec4(0.12, 0.12, 0.15, 1.0));

        for (i, (label, draw)) in cells.iter().enumerate() {
            let top_left = vec2((i % COLUMNS) as f32 * cell.x, (i / COLUMNS) as f32 * cell.y);
            r.draw_rect(
                Rectangle::new(top_left.x, top_left.y, cell.x, cell.y),
                1.0,
                grid_color,
            );

            // Demo in the upper part of the cell, label along the bottom
            let center = top_left + vec2(cell.x / 2.0, cell.y * 0.45);
            let (label, color) = match draw {
                Some(draw) => {
                    draw(center);
                    (label.to_string(), label_color)
                }
                None => (format!("{label} (todo)"), todo_color),
            };
            let label_size = r.get_current_font().measure(&label);
            r.draw_text(
                top_left + vec2((cell.x - label_size.x) / 2.0, cell.y - label_size.y - 6.0),
                &label,
                color,
            );
        }

        r.draw_text(vec2(6.0, 4.0), &format!("{} FPS", app.fps), white);
    }
}

/// 8x8 white and grey checkerboard, RGBA
fn checkerboard(n: u32) -> Vec<u8> {
    (0..n * n)
        .flat_map(|i| {
            let (x, y) = (i % n, i / n);
            if (x + y) % 2 == 0 {
                [255, 255, 255, 255]
            } else {
                [110, 110, 120, 255]
            }
        })
        .collect()
}

/// A frame for the nine-slice
fn frame(size: u32, border: u32) -> Vec<u8> {
    (0..size * size)
        .flat_map(|i| {
            let (x, y) = (i % size, i / size);
            let edge_x = x < border || x >= size - border;
            let edge_y = y < border || y >= size - border;
            match (edge_x, edge_y) {
                (true, true) => [200, 60, 60, 255],
                (true, false) | (false, true) => [240, 150, 60, 255],
                (false, false) => [250, 235, 200, 255],
            }
        })
        .collect()
}

/// A star with `points` points, with outer radius 1 and inner radius `inner`
fn star(points: u32, inner: f32) -> MeshData {
    let n = points * 2;
    let mut positions = vec![0.0, 0.0, 0.0]; // the center
    let mut indices = Vec::new();
    for i in 0..n {
        let a = i as f32 / n as f32 * TAU;
        let radius = if i % 2 == 0 { 1.0 } else { inner };
        positions.extend_from_slice(&[a.cos() * radius, a.sin() * radius, 0.0]);
        indices.extend_from_slice(&[0, 1 + i, 1 + (i + 1) % n]);
    }
    let count = positions.len() / 3;
    MeshData {
        positions,
        normals: [0.0, 0.0, 1.0].repeat(count),
        uvs: vec![0.0; count * 3],
        indices,
    }
}
