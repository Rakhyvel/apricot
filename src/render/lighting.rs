use std::{
    cell::RefCell,
    rc::{Rc, Weak},
};

use nalgebra_glm::{vec3, vec4, Mat4, Vec3};

use crate::{
    camera::{Camera, ViewProj},
    error::Error,
    geometry::frustum::Frustum,
    opengl::{self, DrawOptions, Fbo, Texture},
    render::assets::{MeshId, ProgramId},
    render::RenderContext,
};

/// A light shining from far away in one direction, like the sun, with a shadow map
pub struct DirectionalLight {
    /// Where the light comes from, and the box it fits around the camera's view for shadows
    pub frustum: LightFrustum,
    map: Rc<ShadowMap>,
    /// Whether it's had a shadow pass yet
    has_shadows: bool,
}

impl DirectionalLight {
    /// A light shining from `direction`, with a `shadow_map_size` by `shadow_map_size` shadow map
    pub fn new(direction: Vec3, shadow_map_size: u32) -> Result<Self, Error> {
        Ok(Self {
            frustum: LightFrustum::new(direction, shadow_map_size),
            map: Rc::new(ShadowMap::new(shadow_map_size)?),
            has_shadows: false,
        })
    }

    pub fn light_proj_view(&self) -> Mat4 {
        self.frustum.view_proj.combined()
    }

    /// What drawing needs to know about the light right now
    fn state(&self) -> LightState {
        self.frustum.state(self.has_shadows)
    }
}

/// What the shaders need to know about a light, as of its last shadow pass
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LightState {
    direction: Vec3,
    proj_view: Mat4,
    texel_size: f32,
    shadows: bool,
}

/// The active light
pub(crate) struct Lighting<M = ShadowMap> {
    active: RefCell<Option<Active<M>>>,
}

struct Active<M> {
    map: Weak<M>,
    state: LightState,
}

impl<M> Lighting<M> {
    pub(crate) fn new() -> Self {
        Self {
            active: RefCell::new(None),
        }
    }

    /// Turn on the light with shadow map `map` and `state`, or turn the light off for `None`
    pub(crate) fn set(&self, light: Option<(&Rc<M>, LightState)>) {
        *self.active.borrow_mut() = light.map(|(map, state)| Active {
            map: Rc::downgrade(map),
            state,
        });
    }

    /// Bring the light up to date after a shadow pass, if `map` is the light that's on
    pub(crate) fn refresh(&self, map: &Rc<M>, state: LightState) {
        if let Some(active) = self.active.borrow_mut().as_mut() {
            if std::ptr::eq(active.map.as_ptr(), Rc::as_ptr(map)) {
                active.state = state;
            }
        }
    }

    /// The light that's on and its shadow map. The map stays alive for as long as it's held.
    /// `None` if no light is on, or if its owner dropped it
    pub(crate) fn current(&self) -> Option<(Rc<M>, LightState)> {
        let active = self.active.borrow();
        let active = active.as_ref()?;
        Some((active.map.upgrade()?, active.state))
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
    map_size: u32,
    view_proj: ViewProj,
    texel_size: f32,
}

impl LightFrustum {
    /// A light shining from `direction`, for a `map_size` by `map_size` shadow map
    pub fn new(direction: Vec3, map_size: u32) -> Self {
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

    /// What drawing needs to know about the light as of the last fit. `shadows` says whether
    /// there's a shadow map drawn to use
    fn state(&self, shadows: bool) -> LightState {
        LightState {
            direction: self.direction,
            proj_view: self.view_proj.combined(),
            texel_size: self.texel_size,
            shadows,
        }
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
pub(crate) struct ShadowMap {
    frame_buffer: Fbo,
    depth_map: Texture,
    size: u32,
}

impl ShadowMap {
    fn new(size: u32) -> Result<Self, Error> {
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
            .assets
            .meshes
            .require(mesh_id, "ShadowPass::draw");
        self.renderer
            .draw(self.program, &mesh, model, &self.view_proj)
    }

    /// Everything the light can see. Anything outside it can't cast a shadow
    pub fn frustum(&self) -> Frustum {
        self.view_proj.frustum()
    }
}

impl RenderContext {
    /// Light everything drawn after this with `light`, or with nothing for `None`. Shadow passes on
    /// the same light keep it up to date, and it lights without shadows until its first one. The
    /// renderer doesn't keep the light alive, so dropping it turns the light off
    pub fn set_light(&self, light: Option<&DirectionalLight>) {
        self.lighting
            .set(light.map(|light| (&light.map, light.state())));
    }

    /// Hand `program` the light that's on, and bind its shadow map
    pub(crate) fn upload_light(&self, program: ProgramId) {
        self.uniform(program, "u_shadow_map").set_i32(1);

        match self.lighting.current() {
            Some((map, light)) => {
                self.uniform(program, "u_sun_dir")
                    .set_vec3(&light.direction);
                self.uniform(program, "u_light_mvp")
                    .set_mat4(&light.proj_view);
                self.uniform(program, "u_shadows")
                    .set_i32(light.shadows as i32);
                self.uniform(program, "u_shadow_texel")
                    .set_f32(light.texel_size);
                if light.shadows {
                    opengl::bind_texture_id(1, map.depth_map.id);
                }
            }
            None => {
                self.uniform(program, "u_sun_dir").set_vec3(&Vec3::zeros());
                self.uniform(program, "u_shadows").set_i32(0);
            }
        }
    }

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
        opengl::set_viewport(light.map.size as i32, light.map.size as i32);
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
        self.lighting.refresh(&light.map, light.state());
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

    fn lit(texel_size: f32) -> LightState {
        LightState {
            direction: vec3(0.0, 1.0, 0.0),
            proj_view: Mat4::identity(),
            texel_size,
            shadows: true,
        }
    }

    #[test]
    fn a_fitted_light_describes_itself_for_the_shaders() {
        let light = fitted();
        let state = light.state(true);
        assert_eq!(state.direction, light.direction());
        assert_eq!(state.proj_view, light.view_proj().combined());
        assert_eq!(state.texel_size, light.texel_size());
        assert!(state.shadows);
        assert!(!light.state(false).shadows);
    }

    #[test]
    fn nothing_is_lit_until_a_light_is_set() {
        let lighting: Lighting<u8> = Lighting::new();
        assert!(lighting.current().is_none());
    }

    #[test]
    fn the_light_that_is_set_is_the_one_drawn_with_until_it_is_turned_off() {
        let lighting = Lighting::new();
        let map = Rc::new(7u8);
        lighting.set(Some((&map, lit(0.5))));
        let (current, state) = lighting.current().unwrap();
        assert!(Rc::ptr_eq(&current, &map));
        assert_eq!(state, lit(0.5));

        lighting.set(None);
        assert!(lighting.current().is_none());
    }

    #[test]
    fn setting_a_light_doesnt_keep_it_alive_and_dropping_it_turns_the_light_off() {
        let lighting = Lighting::new();
        let map = Rc::new(7u8);
        lighting.set(Some((&map, lit(0.5))));
        assert_eq!(Rc::strong_count(&map), 1, "the renderer only watches it");

        drop(map);
        assert!(lighting.current().is_none(), "no shadow map left to bind");
    }

    #[test]
    fn a_dropped_light_stays_off_when_another_light_shows_up() {
        let lighting = Lighting::new();
        let old = Rc::new(1u8);
        lighting.set(Some((&old, lit(0.5))));
        drop(old);
        // A light that was never set isn't mistaken for the dropped one
        let newer = Rc::new(2u8);
        lighting.refresh(&newer, lit(9.0));
        assert!(lighting.current().is_none());
    }

    #[test]
    fn refreshing_only_updates_the_light_that_is_on() {
        let lighting = Lighting::new();
        let (on, other) = (Rc::new(1u8), Rc::new(2u8));
        lighting.set(Some((&on, lit(0.5))));
        lighting.refresh(&other, lit(9.0));
        assert_eq!(lighting.current().unwrap().1, lit(0.5));
        lighting.refresh(&on, lit(2.0));
        assert_eq!(lighting.current().unwrap().1, lit(2.0));
    }

    #[test]
    fn a_shadow_map_being_drawn_with_stays_alive_even_if_its_light_is_dropped_mid_draw() {
        let lighting = Lighting::new();
        let map = Rc::new(7u8);
        lighting.set(Some((&map, lit(0.5))));
        let (held, _) = lighting.current().unwrap();
        drop(map);
        assert_eq!(*held, 7);
        drop(held);
        assert!(lighting.current().is_none());
    }
}
