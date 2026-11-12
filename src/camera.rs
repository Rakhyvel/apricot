//! This module implements the Camera structure. Cameras can either be perspective (typical for 3D) or orthographic
//! (typical for 2D)

use crate::ray::Ray;

use super::frustum::Frustum;

#[derive(Debug, Copy, Clone)]
/// Which kind of projection the camera uses.
pub enum ProjectionKind {
    Perspective {
        fov_rad: f32,
        far: f32,
    },
    Orthographic {
        // TODO: Should these just be an AABB?
        left: f32,
        right: f32,
        bottom: f32,
        top: f32,
        near: f32,
        far: f32,
    },
}

impl Default for ProjectionKind {
    fn default() -> Self {
        Self::Perspective {
            fov_rad: std::f32::consts::FRAC_PI_4,
            far: 1000.0,
        }
    }
}

#[derive(Default, Debug, Copy, Clone)]
/// A camera data structure
pub struct Camera {
    position: nalgebra_glm::Vec3,
    lookat: nalgebra_glm::Vec3,
    up: nalgebra_glm::Vec3,

    pub projection_kind: ProjectionKind,
    view_matrix: nalgebra_glm::Mat4,
}

impl Camera {
    /// Creates a new camera data structure
    pub fn new(
        position: nalgebra_glm::Vec3,
        lookat: nalgebra_glm::Vec3,
        up: nalgebra_glm::Vec3,
        projection_kind: ProjectionKind,
    ) -> Self {
        let mut retval = Self {
            position,
            lookat,
            up,
            projection_kind,
            view_matrix: nalgebra_glm::identity(),
        };
        retval.regen_view_matrix();
        retval
    }

    /// The camera's view matrix
    pub fn view_matrix(&self) -> nalgebra_glm::Mat4 {
        self.view_matrix
    }

    /// The camera's projection matrix for a screen with the given aspect ratio (width / height).
    /// Orthographic cameras ignore the aspect
    pub fn proj_matrix(&self, aspect: f32) -> nalgebra_glm::Mat4 {
        match self.projection_kind {
            ProjectionKind::Perspective { fov_rad, far } => {
                nalgebra_glm::perspective(aspect, fov_rad, 0.1, far)
            }
            ProjectionKind::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => nalgebra_glm::ortho(left, right, bottom, top, near, far),
        }
    }

    /// The camera's view and projection matrices for a screen with the given aspect ratio
    pub fn view_proj_matrices(&self, aspect: f32) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        (self.view_matrix, self.proj_matrix(aspect))
    }

    /// The inverse of the projection and view matrices multiplied together
    pub fn inv_proj_view(&self, aspect: f32) -> nalgebra_glm::Mat4 {
        nalgebra_glm::inverse(&(self.proj_matrix(aspect) * self.view_matrix))
    }

    /// The inverse of the projection matrix and the inverse of the view matrix
    pub fn inv_proj_and_view(&self, aspect: f32) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        (
            nalgebra_glm::inverse(&self.proj_matrix(aspect)),
            nalgebra_glm::inverse(&self.view_matrix),
        )
    }

    /// The camera's view frustum for a screen with the given aspect ratio
    pub fn frustum(&self, aspect: f32) -> Frustum {
        Frustum::from_inv_proj_view(self.inv_proj_view(aspect), false)
    }

    /// A ray from the camera through pixel (`x`, `y`) of a `width` by `height` image
    pub fn get_ray(&self, x: f32, y: f32, width: f32, height: f32) -> Ray {
        let (lower_left_corner, horizontal, vertical) = match self.projection_kind {
            ProjectionKind::Perspective { fov_rad, .. } => {
                // Viewport spans at a near plane 1 unit away
                let viewport_height = 2.0 * (fov_rad / 2.0).tan();
                let viewport_width = width / height * viewport_height;

                let w = (self.position - self.lookat).normalize();
                let u = self.up.cross(&w).normalize();
                let v = w.cross(&u);

                let horizontal = u * viewport_width;
                let vertical = v * viewport_height;
                (
                    self.position - horizontal / 2.0 - vertical / 2.0 - w,
                    horizontal,
                    vertical,
                )
            }
            ProjectionKind::Orthographic {
                left,
                right,
                bottom,
                top,
                ..
            } => {
                let horizontal = nalgebra_glm::vec3(right - left, 0.0, 0.0);
                let vertical = nalgebra_glm::vec3(0.0, top - bottom, 0.0);
                (
                    self.position - horizontal * 0.5 - vertical * 0.5,
                    horizontal,
                    vertical,
                )
            }
        };

        let u = (x + 0.5) / width;
        let v = 1.0 - (y + 0.5) / height;
        let dir = (lower_left_corner + horizontal * u + vertical * v - self.position).normalize();
        Ray::new(self.position, dir)
    }

    /// Sets the position of the camera
    pub fn set_position(&mut self, position: nalgebra_glm::Vec3) {
        self.position = position;
        self.regen_view_matrix()
    }

    /// Sets the lookat of the camera
    pub fn set_lookat(&mut self, lookat: nalgebra_glm::Vec3) {
        self.lookat = lookat;
        self.regen_view_matrix()
    }

    /// Retrieves the position of the camera
    pub fn position(&self) -> nalgebra_glm::Vec3 {
        self.position
    }

    /// Retrieves the lookat of the camera
    pub fn lookat(&self) -> nalgebra_glm::Vec3 {
        self.lookat
    }

    /// Retrieves the up direction for the camera
    pub fn up(&self) -> nalgebra_glm::Vec3 {
        self.up
    }

    fn regen_view_matrix(&mut self) {
        self.view_matrix = nalgebra_glm::look_at(&self.position, &self.lookat, &self.up);
    }
}
