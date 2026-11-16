//! Lines through points in 3D

use nalgebra_glm::{Mat4, Vec3, Vec4};

use crate::{
    opengl::{DrawOptions, Primitive, RawMesh},
    render::RenderContext,
};

/// How a `LinePath` looks
#[derive(Clone, Copy, Debug)]
pub struct LineStyle {
    /// Thickness in pixels. Most drivers only support 1
    pub width: f32,
    pub color: Vec4,
    /// Whether the line fades out behind the seam, like a trail
    pub fade: bool,
    /// Where along the path the fade starts, from 0 (the first point) to 1 (the last)
    pub seam: f32,
    /// Whether 3D geometry in front of the line hides it
    pub depth_test: bool,
}

impl Default for LineStyle {
    fn default() -> Self {
        Self {
            width: 1.0,
            color: nalgebra_glm::vec4(0.7, 0.9, 0.9, 0.9),
            fade: true,
            seam: 0.0,
            depth_test: true,
        }
    }
}

/// A 3D line through a list of points
pub struct LinePath {
    mesh: RawMesh,
    pub style: LineStyle,
}

impl LineStyle {
    /// The GL settings lines with this style are drawn with
    fn draw_options(&self) -> DrawOptions {
        DrawOptions {
            depth_test: self.depth_test,
            depth_write: false, // lines shouldn't hide what's drawn after them
            line_width: self.width,
            ..Default::default()
        }
    }
}

impl RenderContext {
    /// Draw a line path from the current camera, moved by `transform`
    pub fn draw_line_path(&self, path: &LinePath, transform: Mat4) {
        let style = &path.style;
        let program = self.builtins.program_line;

        let view_proj = self.camera_view_proj();
        self.uniform(program, "u_model_matrix").set_mat4(&transform);
        self.uniform(program, "u_view_matrix")
            .set_mat4(&view_proj.view);
        self.uniform(program, "u_proj_matrix")
            .set_mat4(&view_proj.proj);
        self.uniform(program, "u_color").set_vec4(&style.color);
        self.uniform(program, "u_seam").set_f32(style.seam);
        self.uniform(program, "u_fade").set_i32(style.fade as i32);
        self.uniform(program, "u_num_vertices")
            .set_i32(path.mesh.vertex_count() as i32);

        self.draw_raw(&path.mesh, program, &style.draw_options());
    }
}

impl LinePath {
    /// A line through `points`, in order
    pub fn new(points: &[Vec3], style: LineStyle) -> Self {
        Self {
            mesh: RawMesh::new(&flatten(points), &[3], Primitive::LineStrip),
            style,
        }
    }

    /// Replace the points the line goes through. Cheaper than making a new `LinePath`
    pub fn set_points(&mut self, points: &[Vec3]) {
        self.mesh.set_vertices(&flatten(points));
    }
}

fn flatten(points: &[Vec3]) -> Vec<f32> {
    points.iter().flat_map(|p| [p.x, p.y, p.z]).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec3;

    #[test]
    fn flatten_lays_points_out_as_x_y_z_one_after_another() {
        let points = [vec3(1.0, 2.0, 3.0), vec3(4.0, 5.0, 6.0)];
        assert_eq!(flatten(&points), [1.0, 2.0, 3.0, 4.0, 5.0, 6.0]);
        assert!(flatten(&[]).is_empty());
    }

    #[test]
    fn a_style_draws_with_its_width_and_depth_test_and_never_hides_what_comes_after() {
        for (depth_test, width) in [(true, 1.0), (false, 4.0)] {
            let style = LineStyle {
                depth_test,
                width,
                ..Default::default()
            };
            let options = style.draw_options();
            assert_eq!(options.depth_test, depth_test);
            assert_eq!(options.line_width, width);
            assert!(
                !options.depth_write,
                "lines shouldn't hide what's drawn after them"
            );
            assert!(options.blending, "the fade needs see-through colors");
        }
    }
}
