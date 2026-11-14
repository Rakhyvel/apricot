//! What the renderer looks through: the camera, the window's size, and the light

use std::cell::{Cell, RefCell};

use nalgebra_glm::{vec2, vec4, I32Vec2, Mat4, Vec2, Vec3};

use crate::{
    camera::Camera,
    frustum::Frustum,
    ray::Ray,
    render_core::RenderContext,
    shadow_map::{ActiveLight, DirectionalLightSource},
};

#[derive(Default)]
pub(crate) struct View {
    camera: RefCell<Camera>,
    screen: Cell<I32Vec2>,
    light: Cell<Option<ActiveLight>>,
}

impl RenderContext {
    pub fn set_camera(&self, camera: Camera) {
        *self.view.camera.borrow_mut() = camera
    }

    /// The window's aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        let res = self.view.screen.get();
        if res.y == 0 {
            1.0
        } else {
            res.x as f32 / res.y as f32
        }
    }

    /// The current camera's view and projection matrices, fitted to the window
    pub fn camera_view_proj(&self) -> (Mat4, Mat4) {
        self.view
            .camera
            .borrow()
            .view_proj_matrices(self.aspect_ratio())
    }

    /// The current camera's view frustum, fitted to the window
    pub fn camera_frustum(&self) -> Frustum {
        self.view.camera.borrow().frustum(self.aspect_ratio())
    }

    /// Roughly how many pixels across the radius of a sphere at `center` looks on screen
    pub fn projected_radius_px(&self, center: Vec3, radius: f32) -> f32 {
        let height = self.view.screen.get().y as f32;
        self.view
            .camera
            .borrow()
            .projected_radius_px(center, radius, height)
    }

    /// Where a world-space point lands on screen, in pixels. `None` if it's behind the camera
    pub fn world_to_screen(&self, p: Vec3) -> Option<Vec2> {
        let (view, proj) = self.camera_view_proj();
        let clip = proj * view * vec4(p.x, p.y, p.z, 1.0);
        if clip.w <= 0.0 {
            return None;
        }
        let ndc = clip.xyz() / clip.w;
        let res = self.view.screen.get();
        Some(vec2(
            (ndc.x + 1.0) / 2.0 * res.x as f32,
            (1.0 - ndc.y) / 2.0 * res.y as f32,
        ))
    }

    /// A ray from the camera through a pixel on screen
    pub fn screen_ray(&self, pos: Vec2) -> Ray {
        let res = self.view.screen.get();
        self.view
            .camera
            .borrow()
            .get_ray(pos.x, pos.y, res.x as f32, res.y as f32)
    }

    /// A copy of the current camera
    pub fn camera(&self) -> Camera {
        *self.view.camera.borrow()
    }

    /// The window's size, in pixels
    pub fn screen_size(&self) -> I32Vec2 {
        self.view.screen.get()
    }

    pub(crate) fn set_screen_size(&self, size: I32Vec2) {
        self.view.screen.set(size);
    }

    /// Light everything drawn after this with `light`, or with nothing for `None`. Shadow passes on
    /// the same light keep it up to date, and it lights without shadows until its first one
    pub fn set_light(&self, light: Option<&DirectionalLightSource>) {
        self.view
            .light
            .set(light.map(DirectionalLightSource::active));
    }

    pub(crate) fn light(&self) -> Option<ActiveLight> {
        self.view.light.get()
    }

    /// Swap in a light's new state, if it's the light being drawn with
    pub(crate) fn refresh_light(&self, updated: ActiveLight) {
        let same = self
            .view
            .light
            .get()
            .is_some_and(|light| light.depth_map == updated.depth_map);
        if same {
            self.view.light.set(Some(updated));
        }
    }
}
