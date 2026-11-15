//! What the renderer looks through: the camera, the window's size, and the light

use std::cell::{Cell, RefCell};

use nalgebra_glm::{I32Vec2, Vec2, Vec3};

use crate::{
    camera::{Camera, ViewProj, Viewport},
    geometry::frustum::Frustum,
    geometry::ray::Ray,
    render_core::RenderContext,
    shadow_map::{ActiveLight, DirectionalLight},
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

    /// The window as a viewport, for converting between the world and the screen
    pub fn viewport(&self) -> Viewport {
        let res = self.view.screen.get();
        Viewport::new(res.x as f32, res.y as f32)
    }

    /// The window's aspect ratio (width / height)
    pub fn aspect_ratio(&self) -> f32 {
        self.viewport().aspect_ratio()
    }

    /// The current camera's view and projection matrices, fitted to the window
    pub fn camera_view_proj(&self) -> ViewProj {
        self.view
            .camera
            .borrow()
            .view_proj(self.aspect_ratio())
    }

    /// The current camera's view frustum, fitted to the window
    pub fn camera_frustum(&self) -> Frustum {
        self.view.camera.borrow().frustum(self.aspect_ratio())
    }

    /// Roughly how many pixels across the radius of a sphere at `center` looks on screen
    pub fn projected_radius_px(&self, center: Vec3, radius: f32) -> f32 {
        self.view
            .camera
            .borrow()
            .projected_radius_px(center, radius, self.viewport())
    }

    /// Where a world-space point lands on screen, in pixels. `None` if it's behind the camera
    pub fn world_to_screen(&self, p: Vec3) -> Option<Vec2> {
        self.view
            .camera
            .borrow()
            .world_to_screen(p, self.viewport())
    }

    /// The ray from the camera through `pos` on screen, in pixels from the top left
    pub fn screen_ray(&self, pos: Vec2) -> Ray {
        self.view.camera.borrow().screen_ray(pos, self.viewport())
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
    pub fn set_light(&self, light: Option<&DirectionalLight>) {
        self.view.light.set(light.map(DirectionalLight::active));
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
