use nalgebra_glm::{vec3, vec4, Mat4, Vec3};

use crate::{camera::ProjectionKind, error::Error, render_core::MeshId};

use super::{
    camera::Camera,
    frustum::Frustum,
    opengl::{Fbo, Texture},
    render_core::RenderContext,
};

pub struct DirectionalLightSource {
    /// Direction towards the light
    pub light_dir: nalgebra_glm::Vec3,
    pub shadow_distance: f32,
    pub caster_distance: f32,
    frame_buffer: Fbo,
    frame_buffer_size: i32,
    depth_map: Texture,
    light_view: Mat4,
    light_proj: Mat4,
    texel_size: f32,
}

#[derive(Clone, Copy)]
pub(crate) struct ActiveLight {
    pub light_dir: Vec3,
    pub light_proj_view: Mat4,
    pub depth_map: gl::types::GLuint,
    pub texel_size: f32,
}

impl DirectionalLightSource {
    pub fn new(light_dir: Vec3, shadow_map_size: i32) -> Result<Self, Error> {
        let depth_map = Texture::new();
        depth_map.load_depth_buffer(shadow_map_size, shadow_map_size);
        let frame_buffer = Fbo::new();
        frame_buffer.bind();
        depth_map.post_bind()?;
        frame_buffer.unbind();
        Ok(Self {
            light_dir: light_dir.normalize(),
            shadow_distance: 50.0,
            caster_distance: 100.0,
            frame_buffer,
            frame_buffer_size: shadow_map_size,
            depth_map,
            light_view: Mat4::identity(),
            light_proj: Mat4::identity(),
            texel_size: 0.0,
        })
    }

    /// Fit the light's orthographic view box around everything `camera` can see on a screen with
    /// the given aspect ratio
    pub fn fit_to_camera(&mut self, camera: &Camera, aspect: f32) {
        self.fit_to_slice(camera, aspect, 0.1, self.shadow_distance);
    }

    /// Fit the light's view box around the part of `camera`'s view between `near` and `far`
    pub fn fit_to_slice(&mut self, camera: &Camera, aspect: f32, near: f32, far: f32) {
        let view = camera.view_matrix();
        let proj = match camera.projection_kind {
            ProjectionKind::Perspective {
                fov_rad,
                far: cam_far,
            } => nalgebra_glm::perspective(aspect, fov_rad, near, far.min(cam_far)),
            ProjectionKind::Orthographic { .. } => camera.proj_matrix(aspect),
        };

        let corners =
            Frustum::from_inv_proj_view(nalgebra_glm::inverse(&(proj * view)), false).corners();

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
        self.light_view = nalgebra_glm::look_at(&eye, &center, &up);
        self.light_proj = nalgebra_glm::ortho(
            -radius,
            radius,
            -radius,
            radius,
            -self.caster_distance,
            2.0 * radius,
        );
    }

    pub fn light_proj_view(&self) -> Mat4 {
        self.light_proj * self.light_view
    }

    pub fn activate_framebuffer(&self, program_id: u32) {
        self.depth_map.activate(gl::TEXTURE1);
        self.depth_map
            .associate_uniform(program_id, 1, "shadow_map");
    }

    pub(crate) fn active(&self) -> ActiveLight {
        ActiveLight {
            light_dir: self.light_dir.normalize(),
            light_proj_view: self.light_proj_view(),
            depth_map: self.depth_map.id,
            texel_size: self.texel_size,
        }
    }
}

/// Passed to the closure in `RenderContext::shadow_pass`. Meshes drawn through it cast shadows.
pub struct ShadowPass<'a> {
    renderer: &'a RenderContext,
    light_view: Mat4,
    light_proj: Mat4,
}

impl ShadowPass<'_> {
    pub fn draw(&self, mesh_id: MeshId, model: Mat4) {
        let mesh = self
            .renderer
            .get_mesh_from_id(mesh_id)
            .expect("invalid MeshId");
        self.renderer
            .draw(&mesh, model, self.light_view, self.light_proj)
    }

    /// Everything the light can see. Anything outside it can't cast a shadow
    pub fn frustum(&self) -> Frustum {
        Frustum::from_inv_proj_view(
            nalgebra_glm::inverse(&(self.light_proj * self.light_view)),
            false,
        )
    }
}

impl RenderContext {
    /// Render onto `light`'s shadow map, fitted to the current camera.
    pub fn shadow_pass(
        &self,
        light: &mut DirectionalLightSource,
        draw_casters: impl FnOnce(&ShadowPass),
    ) {
        light.fit_to_camera(&self.camera.borrow(), self.aspect_ratio());

        light.frame_buffer.bind();
        unsafe {
            gl::Viewport(0, 0, light.frame_buffer_size, light.frame_buffer_size);
            gl::Enable(gl::DEPTH_TEST);
            gl::Enable(gl::CULL_FACE);
            gl::CullFace(gl::BACK);
            gl::Enable(gl::POLYGON_OFFSET_FILL);
            gl::PolygonOffset(1.0, 1.0);
            gl::Clear(gl::DEPTH_BUFFER_BIT)
        }
        self.set_program_from_id(self.builtins.program_shadow);

        draw_casters(&ShadowPass {
            renderer: self,
            light_view: light.light_view,
            light_proj: light.light_proj,
        });

        light.frame_buffer.unbind();
        unsafe {
            gl::Viewport(
                0,
                0,
                self.int_screen_resolution.x,
                self.int_screen_resolution.y,
            );
            gl::Disable(gl::POLYGON_OFFSET_FILL);
            gl::CullFace(gl::BACK);
        }
        self.active_light.set(Some(light.active()));
    }

    /// Stop lighting `draw_mesh` with the last shaddow pass's light
    pub fn clear_light(&self) {
        self.active_light.set(None);
    }
}
