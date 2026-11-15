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

/// A light shining from far away in one direction, like the sun, with a shadow map
pub struct DirectionalLight {
    /// Where the light comes from, and the box it fits around the camera's view for shadows
    pub frustum: LightFrustum,
    map: ShadowMap,
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
    /// A light shining from `direction`, with a `shadow_map_size` by `shadow_map_size` shadow map
    pub fn new(direction: Vec3, shadow_map_size: i32) -> Result<Self, Error> {
        Ok(Self {
            frustum: LightFrustum::new(direction, shadow_map_size),
            map: ShadowMap::new(shadow_map_size)?,
            has_shadows: false,
        })
    }

    pub fn light_proj_view(&self) -> Mat4 {
        self.frustum.view_proj.combined()
    }

    pub(crate) fn active(&self) -> ActiveLight {
        ActiveLight {
            light_dir: self.frustum.direction,
            light_proj_view: self.light_proj_view(),
            depth_map: self.map.depth_map.id,
            texel_size: self.frustum.texel_size,
            shadows: self.has_shadows,
        }
    }
}

/// Where a directional light comes from, and the box it fits around a camera's view to cast
/// shadows into
#[derive(Debug, Clone)]
pub struct LightFrustum {
    /// Toward the light, always normalized
    direction: Vec3,
    /// How far from the camera shadows reach
    pub shadow_distance: f32,
    /// How far past the camera's view, toward the light, things still cast shadows into it
    pub caster_distance: f32,
    /// How many texels across the shadow map is, so the box can move in whole texels
    map_size: i32,
    view_proj: ViewProj,
    texel_size: f32,
}

impl LightFrustum {
    /// A light shining from `direction`, for a `map_size` by `map_size` shadow map
    pub fn new(direction: Vec3, map_size: i32) -> Self {
        Self {
            direction: direction.normalize(),
            shadow_distance: 50.0,
            caster_distance: 100.0,
            map_size,
            view_proj: ViewProj {
                view: Mat4::identity(),
                proj: Mat4::identity(),
            },
            texel_size: 0.0,
        }
    }

    /// The direction toward the light, normalized
    pub fn direction(&self) -> Vec3 {
        self.direction
    }

    /// Point the light the other way. `direction` is toward the light, and doesn't need to be
    /// normalized
    pub fn set_direction(&mut self, direction: Vec3) {
        self.direction = direction.normalize();
    }

    /// The light's view and projection from the last fit
    pub fn view_proj(&self) -> ViewProj {
        self.view_proj
    }

    /// How wide one shadow map texel is in the world, from the last fit
    pub fn texel_size(&self) -> f32 {
        self.texel_size
    }

    /// Fit the box around everything `camera` can see on a screen with the given aspect ratio,
    /// out to `shadow_distance`
    pub fn fit_to_camera(&mut self, camera: &Camera, aspect: f32) {
        let near = camera.projection_kind.near();
        self.fit_to_slice(camera, aspect, near, self.shadow_distance);
    }

    /// Fit the box around the part of `camera`'s view between `near` and `far`
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

        let dir = self.direction;
        // look_at breaks if `up` is parallel to the view direction
        let up = if dir.y.abs() > 0.99 {
            vec3(0.0, 0.0, 1.0)
        } else {
            vec3(0.0, 1.0, 0.0)
        };

        // Measure the camera frustum from a light at the origin
        let light_rot = nalgebra_glm::look_at(&Vec3::zeros(), &-dir, &up);
        let texel = 2.0 * radius / self.map_size as f32;
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
}

/// The depth texture shadows get drawn into, and the framebuffer that draws into it
struct ShadowMap {
    frame_buffer: Fbo,
    depth_map: Texture,
    size: i32,
}

impl ShadowMap {
    fn new(size: i32) -> Result<Self, Error> {
        let depth_map = Texture::new();
        depth_map.load_depth_buffer(size, size);
        let frame_buffer = Fbo::new();
        frame_buffer.attach_depth(&depth_map)?;
        Ok(Self {
            frame_buffer,
            depth_map,
            size,
        })
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
        light
            .frustum
            .fit_to_camera(&self.camera(), self.aspect_ratio());

        light.map.frame_buffer.bind();
        opengl::set_viewport(light.map.size, light.map.size);
        let before = DrawOptions {
            depth_bias: Some((1.0, 1.0)),
            ..Default::default()
        }
        .apply();
        opengl::clear_depth();

        draw_casters(&ShadowPass {
            renderer: self,
            program: self.builtins.program_shadow,
            view_proj: light.frustum.view_proj,
        });

        light.map.frame_buffer.unbind();
        let screen = self.screen_size();
        opengl::set_viewport(screen.x, screen.y);
        before.restore();

        light.has_shadows = true;
        self.refresh_light(light.active());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::ProjectionKind;

    fn camera() -> Camera {
        Camera::new(
            vec3(4.0, 3.0, 10.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            ProjectionKind::Perspective {
                fov_rad: std::f32::consts::FRAC_PI_3,
                near: 0.1,
                far: 200.0,
            },
        )
    }

    fn fitted() -> LightFrustum {
        let mut light = LightFrustum::new(vec3(0.5, 1.0, 0.3), 1024);
        light.shadow_distance = 30.0;
        light.fit_to_camera(&camera(), 1.5);
        light
    }

    /// Where `p` lands in the light's box. Inside is -1 to 1 on every axis
    fn in_light_box(light: &LightFrustum, p: Vec3) -> Vec3 {
        let clip = light.view_proj().combined() * vec4(p.x, p.y, p.z, 1.0);
        clip.xyz() / clip.w
    }

    #[test]
    fn direction_is_kept_normalized() {
        let mut light = LightFrustum::new(vec3(0.0, 3.0, 4.0), 1024);
        assert!((light.direction() - vec3(0.0, 0.6, 0.8)).norm() < 1e-6);
        light.set_direction(vec3(2.0, 0.0, 0.0));
        assert_eq!(light.direction(), vec3(1.0, 0.0, 0.0));
    }

    #[test]
    fn the_box_holds_everything_the_camera_sees_up_to_the_shadow_distance() {
        let light = fitted();
        let proj = camera().proj_matrix_range(1.5, 0.1, 30.0);
        let slice = Frustum::from_proj_view(proj * camera().view_matrix());
        for corner in slice.corners() {
            let p = in_light_box(&light, corner);
            assert!(
                p.iter().all(|v| v.abs() <= 1.0),
                "{corner:?} lands at {p:?}"
            );
        }
    }

    #[test]
    fn the_box_reaches_back_toward_the_light_by_the_caster_distance() {
        let light = fitted();
        // The middle of the box, and how far it is from there to the box's light-side face
        let center = light.view_proj().view.try_inverse().unwrap() * vec4(0.0, 0.0, 0.0, 1.0);
        let eye = center.xyz();
        let toward_light = light.direction();
        let inside = eye + toward_light * (0.9 * light.caster_distance);
        let outside = eye + toward_light * (1.1 * light.caster_distance);
        assert!(in_light_box(&light, inside).z.abs() <= 1.0);
        assert!(in_light_box(&light, outside).z.abs() > 1.0);
    }

    #[test]
    fn the_box_moves_in_whole_texels() {
        // Seen from the light, the box's middle always lands on the texel grid, so shadows don't
        // shimmer as the camera moves
        for x in [0.0, 0.013, 0.37, 5.2] {
            let mut light = LightFrustum::new(vec3(0.5, 1.0, 0.3), 1024);
            let mut cam = camera();
            cam.set_position(vec3(4.0 + x, 3.0, 10.0));
            light.fit_to_camera(&cam, 1.5);

            let dir = light.direction();
            let rot = nalgebra_glm::look_at(&Vec3::zeros(), &-dir, &vec3(0.0, 1.0, 0.0));
            let eye = light.view_proj().view.try_inverse().unwrap() * vec4(0.0, 0.0, 0.0, 1.0);
            let half_size = 1.0 / light.view_proj().proj[(0, 0)];
            let middle = eye.xyz() - dir * half_size;
            let seen = rot * vec4(middle.x, middle.y, middle.z, 1.0);
            for v in [seen.x, seen.y] {
                let texels = v / light.texel_size();
                assert!(
                    (texels - texels.round()).abs() < 1e-2,
                    "moved {x}: {texels} texels"
                );
            }
        }
    }

    /// Half the box's width, and where the light looks at the box from
    fn half_size_and_eye(light: &LightFrustum) -> (f32, Vec3) {
        let eye = light.view_proj().view.try_inverse().unwrap() * vec4(0.0, 0.0, 0.0, 1.0);
        (1.0 / light.view_proj().proj[(0, 0)], eye.xyz())
    }

    /// The middle of the camera's view out to 30, and how far its farthest corner is from there
    fn slice_sphere() -> (Vec3, f32) {
        let proj = camera().proj_matrix_range(1.5, 0.1, 30.0);
        let corners = Frustum::from_proj_view(proj * camera().view_matrix()).corners();
        let center = corners.iter().sum::<Vec3>() / 8.0;
        let radius = corners
            .iter()
            .map(|c| (c - center).norm())
            .fold(0.0, f32::max);
        (center, radius)
    }

    #[test]
    fn the_box_is_no_bigger_than_it_needs_to_be() {
        let (half_size, _) = half_size_and_eye(&fitted());
        let (_, radius) = slice_sphere();
        // Rounded up to the next 16th, so tiny camera moves don't resize it
        assert!(
            half_size >= radius && half_size <= radius + 1.0 / 16.0 + 1e-4,
            "{half_size} for {radius}"
        );
        assert!(((half_size * 16.0) - (half_size * 16.0).round()).abs() < 1e-3);
    }

    #[test]
    fn the_box_is_centered_on_the_slice() {
        let light = fitted();
        let (half_size, eye) = half_size_and_eye(&light);
        let (center, _) = slice_sphere();
        let dir = light.direction();
        let offset = eye - center;
        // Back toward the light by exactly half the box, and off to the side by under 2 texels
        assert!(
            (offset.dot(&dir) - half_size).abs() < 1e-3,
            "{}",
            offset.dot(&dir)
        );
        let sideways = (offset - dir * offset.dot(&dir)).norm();
        assert!(
            sideways < 2.0 * light.texel_size(),
            "{sideways} off to the side"
        );
    }

    #[test]
    fn a_light_straight_overhead_still_fits() {
        let mut light = LightFrustum::new(vec3(0.0, 1.0, 0.0), 1024);
        light.shadow_distance = 30.0;
        light.fit_to_camera(&camera(), 1.5);
        assert!(light.view_proj().combined().iter().all(|v| v.is_finite()));
        let proj = camera().proj_matrix_range(1.5, 0.1, 30.0);
        for corner in Frustum::from_proj_view(proj * camera().view_matrix()).corners() {
            let p = in_light_box(&light, corner);
            assert!(
                p.iter().all(|v| v.abs() <= 1.0),
                "{corner:?} lands at {p:?}"
            );
        }
    }

    #[test]
    fn a_texel_is_the_box_width_split_over_the_map() {
        let light = fitted();
        let width = 2.0 / light.view_proj().proj[(0, 0)];
        assert!((light.texel_size() - width / 1024.0).abs() < 1e-6);
    }
}
