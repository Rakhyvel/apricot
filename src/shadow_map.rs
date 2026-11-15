use nalgebra_glm::{vec3, vec4, Mat4, Vec3};

use crate::{
    camera::ViewProj,
    error::Error,
    render_core::{MeshId, ProgramId},
};

use super::{
    camera::Camera,
    geometry::frustum::Frustum,
    opengl::{self, DrawOptions, Fbo, Texture},
    render_core::RenderContext,
};

pub struct DirectionalLight {
    /// Direction towards the light
    pub light_dir: nalgebra_glm::Vec3,
    pub shadow_distance: f32,
    pub caster_distance: f32,
    frame_buffer: Fbo,
    frame_buffer_size: i32,
    depth_map: Texture,
    view_proj: ViewProj,
    texel_size: f32,
    /// Whether it's had a shadow pass yet
    has_shadows: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct ActiveLight {
    pub light_dir: Vec3,
    pub light_proj_view: Mat4,
    pub depth_map: u32,
    pub texel_size: f32,
    pub shadows: bool,
}

impl DirectionalLight {
    pub fn new(light_dir: Vec3, shadow_map_size: i32) -> Result<Self, Error> {
        let depth_map = Texture::new();
        depth_map.load_depth_buffer(shadow_map_size, shadow_map_size);
        let frame_buffer = Fbo::new();
        frame_buffer.attach_depth(&depth_map)?;
        Ok(Self {
            light_dir: light_dir.normalize(),
            shadow_distance: 50.0,
            caster_distance: 100.0,
            frame_buffer,
            frame_buffer_size: shadow_map_size,
            depth_map,
            view_proj: ViewProj {
                view: Mat4::identity(),
                proj: Mat4::identity(),
            },
            texel_size: 0.0,
            has_shadows: false,
        })
    }

    /// Fit the light's orthographic view box around everything `camera` can see on a screen with
    /// the given aspect ratio
    pub fn fit_to_camera(&mut self, camera: &Camera, aspect: f32) {
        let near = camera.projection_kind.near();
        self.fit_to_slice(camera, aspect, near, self.shadow_distance);
    }

    /// Fit the light's view box around the part of `camera`'s view between `near` and `far`
    pub fn fit_to_slice(&mut self, camera: &Camera, aspect: f32, near: f32, far: f32) {
        let far = far.min(camera.projection_kind.far());
        let proj = camera.proj_matrix_range(aspect, near, far);
        let corners = Frustum::from_proj_view(proj * camera.view_matrix()).corners();

        // Bounding sphere of the slice
        let center = corners.iter().fold(Vec3::zeros(), |sum, c| sum + c) / 8.0;
        let radius = corners
            .iter()
            .map(|c| (c - center).norm())
            .fold(0.0, f32::max);
        let radius = (radius * 16.0).ceil() / 16.0; // prevent frame-to-frame camera float noise

        let dir = self.light_dir.normalize();
        // look_at breaks if `up` is parallel to the view direction
        let up = if dir.y.abs() > 0.99 {
            vec3(0.0, 0.0, 1.0)
        } else {
            vec3(0.0, 1.0, 0.0)
        };

        // Measure the camera frustum from a light at the origin
        let light_rot = nalgebra_glm::look_at(&Vec3::zeros(), &-dir, &up);
        let texel = 2.0 * radius / self.frame_buffer_size as f32;
        self.texel_size = texel;
        let mut c = (light_rot * vec4(center.x, center.y, center.z, 1.0)).xyz();
        c.x = (c.x / texel).floor() * texel;
        c.y = (c.y / texel).floor() * texel;
        let center = (nalgebra_glm::inverse(&light_rot) * vec4(c.x, c.y, c.z, 1.0)).xyz();

        let eye = center + dir * radius;
        self.view_proj = ViewProj {
            view: nalgebra_glm::look_at(&eye, &center, &up),
            proj: nalgebra_glm::ortho(
                -radius,
                radius,
                -radius,
                radius,
                -self.caster_distance,
                2.0 * radius,
            ),
        };
    }

    pub fn light_proj_view(&self) -> Mat4 {
        self.view_proj.combined()
    }

    pub(crate) fn active(&self) -> ActiveLight {
        ActiveLight {
            light_dir: self.light_dir.normalize(),
            light_proj_view: self.light_proj_view(),
            depth_map: self.depth_map.id,
            texel_size: self.texel_size,
            shadows: self.has_shadows,
        }
    }
}

/// Passed to the closure in `RenderContext::shadow_pass`. Meshes drawn through it cast shadows.
pub struct ShadowPass<'a> {
    renderer: &'a RenderContext,
    program: ProgramId,
    view_proj: ViewProj,
}

impl ShadowPass<'_> {
    pub fn draw(&self, mesh_id: MeshId, model: Mat4) {
        let mesh = self
            .renderer
            .get_mesh_from_id(mesh_id)
            .expect("invalid MeshId");
        self.renderer
            .draw(self.program, &mesh, model, &self.view_proj)
    }

    /// Everything the light can see. Anything outside it can't cast a shadow
    pub fn frustum(&self) -> Frustum {
        self.view_proj.frustum()
    }
}

impl RenderContext {
    /// Render onto `light`'s shadow map, fitted to the current camera.
    pub fn shadow_pass(
        &self,
        light: &mut DirectionalLight,
        draw_casters: impl FnOnce(&ShadowPass),
    ) {
        light.fit_to_camera(&self.camera(), self.aspect_ratio());

        light.frame_buffer.bind();
        opengl::set_viewport(light.frame_buffer_size, light.frame_buffer_size);
        let before = DrawOptions {
            depth_bias: Some((1.0, 1.0)),
            ..Default::default()
        }
        .apply();
        opengl::clear_depth();

        draw_casters(&ShadowPass {
            renderer: self,
            program: self.builtins.program_shadow,
            view_proj: light.view_proj,
        });

        light.frame_buffer.unbind();
        let screen = self.screen_size();
        opengl::set_viewport(screen.x, screen.y);
        before.restore();

        light.has_shadows = true;
        self.refresh_light(light.active());
    }
}
