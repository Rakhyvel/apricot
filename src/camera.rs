//! This module implements the Camera structure. Cameras can either be perspective (typical for 3D) or orthographic
//! (typical for 2D)

use super::frustrum::Frustrum;

#[derive(Debug, Copy, Clone)]
/// Which kind of projection the camera uses.
pub enum ProjectionKind {
    Perspective {
        fov: f32,
        far: f32,
    },
    Orthographic {
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
            fov: 3.5,
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
    proj_matrix: nalgebra_glm::Mat4,
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
            proj_matrix: nalgebra_glm::identity(),
        };
        retval.regen_view_proj_matrices();
        retval
    }

    /// Retrieves the camera's view and projection matrices
    pub fn view_proj_matrices(&self) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        (self.view_matrix, self.proj_matrix)
    }

    /// Regenerates the camera's view and projection matrices. This is _SLOW_!
    pub fn regen_view_proj_matrices(&mut self) {
        let view_matrix = nalgebra_glm::look_at(&self.position, &self.lookat, &self.up);
        let proj_matrix = match self.projection_kind {
            ProjectionKind::Perspective { fov, far } => {
                nalgebra_glm::perspective(800.0 / 600.0, fov, 0.1, far)
            }
            ProjectionKind::Orthographic {
                left,
                right,
                bottom,
                top,
                near,
                far,
            } => nalgebra_glm::ortho(left, right, bottom, top, near, far),
        };

        self.view_matrix = view_matrix;
        self.proj_matrix = proj_matrix;
    }

    /// Returns the inverse of the projection and view matrices multiplied together. This is _SLOW_!
    pub fn inv_proj_view(&self) -> nalgebra_glm::Mat4 {
        let proj_view_matrix = self.proj_matrix * self.view_matrix;
        nalgebra_glm::inverse(&proj_view_matrix)
    }

    /// Returns the inverse of the projection matrix and inverse of the view matrix
    pub fn inv_proj_and_view(&self) -> (nalgebra_glm::Mat4, nalgebra_glm::Mat4) {
        (
            // TODO: Store these, probably
            nalgebra_glm::inverse(&self.proj_matrix),
            nalgebra_glm::inverse(&self.view_matrix),
        )
    }

    /// Returns the frustrum for this camera
    pub fn frustum(&self) -> Frustrum {
        // TODO: Store frustrum!
        Frustrum::from_inv_proj_view(self.inv_proj_view(), false)
    }

    /// Sets the position of the camera. This regenerates the view and projection matrix, so is fairly slow.
    pub fn set_position(&mut self, position: nalgebra_glm::Vec3) {
        self.position = position;
        self.regen_view_proj_matrices()
    }

    /// Sets the lookat of the camera. This regenerates the view and projection matrix, so is fairly slow.
    pub fn set_lookat(&mut self, lookat: nalgebra_glm::Vec3) {
        self.lookat = lookat;
        self.regen_view_proj_matrices()
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
}
