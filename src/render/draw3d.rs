//! Drawing meshes in 3D, and the outlines around them

use nalgebra_glm::{Mat4, Vec3, Vec4};

use crate::{
    camera::ViewProj,
    opengl::{self, DrawOptions, RawMesh, Stencil, Uniform},
    render::assets::{Mesh, MeshId, ProgramId, TextureId},
    render::RenderContext,
};

/// How many times bigger than its model an outline's copy of it is
const OUTLINE_SCALE: f32 = 1.2;

impl RenderContext {
    /// The uniform called `name` in `program`. Setting it works any time, whatever's being drawn
    pub fn uniform(&self, program: ProgramId, name: &str) -> Uniform {
        self.get_program_from_id(program).uniform(name)
    }

    fn bind(&self, program: ProgramId) {
        self.get_program_from_id(program).bind();
    }

    pub fn clear(&self, color: Vec4) {
        opengl::clear(color);
    }

    /// Draw `mesh` with `program`, which gets the model, view, and projection matrices
    pub fn draw(
        &self,
        program: ProgramId,
        mesh: &Mesh,
        model_matrix: nalgebra_glm::Mat4,
        view_proj: &ViewProj,
    ) {
        self.uniform(program, "u_model_matrix")
            .set_mat4(&model_matrix);
        self.uniform(program, "u_view_matrix")
            .set_mat4(&view_proj.view);
        self.uniform(program, "u_proj_matrix")
            .set_mat4(&view_proj.proj);

        self.bind(program);
        mesh.draw();
    }

    /// Draw `mesh_id` with the built-in 3D shader from the current camera, in a solid `color`
    pub fn draw_mesh(&self, mesh_id: MeshId, model: Mat4, color: Vec4) {
        self.draw_mesh_textured(mesh_id, self.builtins.white_texture, model, color);
    }

    /// Draw `mesh_id` with a texture, tinted by `tint`. A white tint leaves the texture as is
    pub fn draw_mesh_textured(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
    ) {
        self.draw_mesh_textured_with(mesh_id, texture_id, model, tint, &DrawOptions::default());
    }

    /// Draw a mesh with a given `program` and using given `options`
    pub fn draw_mesh_with_program(
        &self,
        program: ProgramId,
        mesh_id: MeshId,
        model: Mat4,
        options: &DrawOptions,
    ) {
        let before = options.apply();
        self.upload_light(program);
        self.uniform(program, "u_camera_pos")
            .set_vec3(&self.camera().position());
        let mesh = self
            .assets
            .meshes
            .require(mesh_id, "draw_mesh_with_program");
        self.draw(program, &mesh, model, &self.camera_view_proj());
        before.restore();
    }

    pub fn draw_sky(&self, sun_dir: Vec3) {
        let program = self.builtins.program_sky;
        self.bind(program);

        self.uniform(program, "u_sun_dir").set_vec3(&sun_dir);

        let cam = self.camera_view_proj();
        let inv_view_proj = (cam.proj * cam.view).try_inverse().unwrap();

        self.uniform(program, "u_inv_view_proj")
            .set_mat4(&inv_view_proj);
        self.uniform(program, "u_camera_pos")
            .set_vec3(&self.camera.get().position());

        let options = DrawOptions {
            depth_write: false,
            culling: false,
            ..Default::default() // depth test stays on, so only pixels nothing's drawn on get shaded
        };

        let before = options.apply();

        let model_matrix = Mat4::identity();

        let sky_view_proj = ViewProj {
            view: Mat4::identity(),
            proj: Mat4::identity(),
        };

        let mesh = self.assets.meshes.require(self.builtins.quad, "draw_sky");
        self.draw(program, &mesh, model_matrix, &sky_view_proj);

        before.restore();
    }

    /// Draw `mesh_id` with a texture and custom GL `options`
    pub fn draw_mesh_textured_with(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
        options: &DrawOptions,
    ) {
        let before = options.apply();
        let program = self.builtins.program_3d;
        self.uniform(program, "u_color").set_vec4(&tint);
        self.upload_light(program);
        self.uniform(program, "u_camera_pos")
            .set_vec3(&self.camera().position());

        let texture = self
            .assets
            .textures
            .require(texture_id, "draw_mesh_textured");
        texture.activate(0);
        self.uniform(program, "u_texture").set_i32(0);

        let mesh = self.assets.meshes.require(mesh_id, "draw_mesh_textured");
        self.draw(program, &mesh, model, &self.camera_view_proj());
        before.restore();
    }

    /// Draw `mesh_id` like `draw_mesh_textured`, and mark where it lands so that an
    /// `outline_pass` afterward can outline it
    pub fn draw_mesh_textured_marked(
        &self,
        mesh_id: MeshId,
        texture_id: TextureId,
        model: Mat4,
        tint: Vec4,
    ) {
        let options = DrawOptions {
            stencil: Stencil::Mark,
            ..Default::default()
        };
        self.draw_mesh_textured_with(mesh_id, texture_id, model, tint, &options);
    }

    /// Draw a `RawMesh` with `program` and `options`
    pub fn draw_raw(&self, mesh: &RawMesh, program: ProgramId, options: &DrawOptions) {
        let before = options.apply();
        self.bind(program);
        mesh.draw();
        before.restore();
    }

    /// Draw a solid `color` outline around everything the closure draws through the pass. Draw
    /// them first with `draw_mesh_textured_marked`, so the outline only shows around them
    pub fn outline_pass(&self, color: Vec4, draw_outlined: impl FnOnce(&OutlinePass)) {
        // Only draw where the models themselves aren't
        let before = DrawOptions {
            stencil: Stencil::OutsideMarks,
            depth_test: false,
            ..Default::default()
        }
        .apply();
        let program = self.builtins.program_3d_solid;
        self.uniform(program, "u_color").set_vec4(&color);

        draw_outlined(&OutlinePass {
            renderer: self,
            program,
            view_proj: self.camera_view_proj(),
        });

        before.restore();
    }
}

/// Passed to the closure in `RenderContext::outline_pass`. Meshes drawn through it get an outline.
pub struct OutlinePass<'a> {
    renderer: &'a RenderContext,
    program: ProgramId,
    view_proj: ViewProj,
}

impl OutlinePass<'_> {
    /// Draw the outline of `mesh_id` placed by `model`
    pub fn draw(&self, mesh_id: MeshId, model: Mat4) {
        let mesh = self
            .renderer
            .assets
            .meshes
            .require(mesh_id, "OutlinePass::draw");
        self.renderer.draw(
            self.program,
            &mesh,
            outline_transform(&model),
            &self.view_proj,
        );
    }
}

/// The transform for the slightly bigger copy of a model that makes its outline, so only the rim
/// pokes out past the model
fn outline_transform(model: &Mat4) -> Mat4 {
    nalgebra_glm::scale(model, &Vec3::repeat(OUTLINE_SCALE))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::{translation, vec3, vec4};

    #[test]
    fn an_outline_copy_is_the_model_scaled_up_about_its_own_origin() {
        let model = translation(&vec3(5.0, 0.0, 0.0));
        let place = |p: Vec3| (outline_transform(&model) * vec4(p.x, p.y, p.z, 1.0)).xyz();
        // The model's own origin stays put, and the rest of it is pushed out from there
        assert!((place(vec3(0.0, 0.0, 0.0)) - vec3(5.0, 0.0, 0.0)).norm() < 1e-5);
        let expected = vec3(5.0 + OUTLINE_SCALE, -OUTLINE_SCALE, 0.5 * OUTLINE_SCALE);
        assert!((place(vec3(1.0, -1.0, 0.5)) - expected).norm() < 1e-5);
    }

    #[test]
    fn an_outline_reaches_past_the_model_so_a_rim_shows() {
        let corner = vec4(1.0, -1.0, 1.0, 1.0);
        let outer = (outline_transform(&Mat4::identity()) * corner).xyz();
        assert!(outer.iter().all(|c| c.abs() > 1.0), "{outer:?}");
    }
}
