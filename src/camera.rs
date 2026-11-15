//! This module implements the Camera structure. Cameras can either be perspective (typical for 3D) or orthographic
//! (typical for 2D)

use nalgebra_glm::{DVec3, Mat4, Vec3};

use crate::geometry::ray::Ray;

use crate::geometry::frustum::Frustum;

#[derive(Debug, Copy, Clone)]
/// Which kind of projection the camera uses.
pub enum ProjectionKind {
    Perspective {
        /// Vertical field of view, in radians
        fov_rad: f32,
        /// The closest distance that gets drawn
        near: f32,
        /// The furthest distance that gets drawn
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
            near: 0.1,
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
            ProjectionKind::Perspective { fov_rad, near, far } => {
                nalgebra_glm::perspective(aspect, fov_rad, near, far)
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
        Frustum::from_inv_proj_view(self.inv_proj_view(aspect))
    }

    /// Roughly how many pixels across the radius of a sphere at `center` looks on a screen
    /// `viewport_height` pixels tall.
    pub fn projected_radius_px(&self, center: Vec3, radius: f32, viewport_height: f32) -> f32 {
        let half_view_height = match self.projection_kind {
            ProjectionKind::Perspective { fov_rad, .. } => {
                (fov_rad / 2.0).tan() * nalgebra_glm::distance(&self.position, &center)
            }
            ProjectionKind::Orthographic { bottom, top, .. } => (top - bottom) / 2.0,
        };
        radius / half_view_height * viewport_height / 2.0
    }

    /// A ray through the middle of pixel (`x`, `y`) of a `width` by `height` image, starting on
    /// the near plane
    pub fn get_ray(&self, x: f32, y: f32, width: f32, height: f32) -> Ray {
        let ndc_x = 2.0 * (x + 0.5) / width - 1.0;
        let ndc_y = 1.0 - 2.0 * (y + 0.5) / height;
        let inv = self.inv_proj_view(width / height);
        let unproject = |ndc_z: f32| {
            let p = inv * nalgebra_glm::vec4(ndc_x, ndc_y, ndc_z, 1.0);
            p.xyz() / p.w
        };
        let near = unproject(-1.0);
        let far = unproject(1.0);
        Ray::new(near, far - near)
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

/// A movable `f64` origin, for turning far-off `f64` positions into small `f32` ones near it
#[derive(Debug, Copy, Clone, Default)]
pub struct FloatingOrigin {
    pub pos: DVec3,
}

impl FloatingOrigin {
    /// An origin at `pos`
    pub fn new(pos: DVec3) -> Self {
        Self { pos }
    }

    /// Where `world` is relative to the origin
    pub fn relative(&self, world: DVec3) -> Vec3 {
        nalgebra_glm::convert(world - self.pos)
    }

    /// A model transform for something at `world`, with its own rotation and scale
    pub fn transform(&self, world: DVec3, rotation_scale: Mat4) -> Mat4 {
        nalgebra_glm::translation(&self.relative(world)) * rotation_scale
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::{vec2, vec3, vec4, Vec2};

    const W: f32 = 800.0;
    const H: f32 = 600.0;

    fn looking_at_origin(projection_kind: ProjectionKind) -> Camera {
        Camera::new(vec3(0.0, 0.0, 5.0), Vec3::zeros(), vec3(0.0, 1.0, 0.0), projection_kind)
    }

    fn perspective() -> Camera {
        looking_at_origin(ProjectionKind::Perspective {
            fov_rad: std::f32::consts::FRAC_PI_3,
            near: 0.1,
            far: 100.0,
        })
    }

    fn orthographic() -> Camera {
        looking_at_origin(ProjectionKind::Orthographic {
            left: -4.0,
            right: 4.0,
            bottom: -3.0,
            top: 3.0,
            near: 0.1,
            far: 100.0,
        })
    }

    /// Where `p` lands on a `W` by `H` image, in pixels from the top left
    fn to_screen(camera: &Camera, p: Vec3) -> Vec2 {
        let (view, proj) = camera.view_proj_matrices(W / H);
        let clip = proj * view * vec4(p.x, p.y, p.z, 1.0);
        let ndc = clip.xyz() / clip.w;
        vec2((ndc.x + 1.0) / 2.0 * W, (1.0 - ndc.y) / 2.0 * H)
    }

    /// How far `p` is from the line along `ray`
    fn miss_distance(ray: &Ray, p: Vec3) -> f32 {
        let to_p = p - ray.origin();
        (to_p - ray.dir() * to_p.dot(&ray.dir())).norm()
    }

    fn assert_rays_hit_what_they_point_at(camera: &Camera) {
        for p in [
            Vec3::zeros(),
            vec3(1.0, -0.5, 0.0),
            vec3(-2.0, 1.0, -3.0),
            vec3(0.5, 2.0, 1.5),
        ] {
            // Pixel (x, y) is the one whose top left corner is at (x, y)
            let px = to_screen(camera, p) - vec2(0.5, 0.5);
            let ray = camera.get_ray(px.x, px.y, W, H);
            let miss = miss_distance(&ray, p);
            assert!(
                miss < 1e-3,
                "{p:?} missed by {miss}, ray from {:?} going {:?}",
                ray.origin(),
                ray.dir()
            );
        }
    }

    #[test]
    fn perspective_rays_go_through_the_point_on_that_pixel() {
        assert_rays_hit_what_they_point_at(&perspective());
    }

    #[test]
    fn orthographic_rays_go_through_the_point_on_that_pixel() {
        assert_rays_hit_what_they_point_at(&orthographic());
    }

    #[test]
    fn orthographic_rays_all_point_where_the_camera_looks() {
        let camera = orthographic();
        for (x, y) in [(W / 2.0 - 0.5, H / 2.0 - 0.5), (0.0, 0.0), (W - 1.0, 100.0)] {
            let dir = camera.get_ray(x, y, W, H).dir();
            assert!((dir - vec3(0.0, 0.0, -1.0)).norm() < 1e-5, "({x}, {y}) went {dir:?}");
        }
    }

    #[test]
    fn projected_radius_fills_the_screen_at_the_view_edge() {
        let camera = Camera::new(
            Vec3::zeros(),
            nalgebra_glm::vec3(0.0, 0.0, -1.0),
            nalgebra_glm::vec3(0.0, 1.0, 0.0),
            ProjectionKind::Perspective {
                fov_rad: std::f32::consts::FRAC_PI_2,
                near: 0.1,
                far: 100.0,
            },
        );
        // With a 90 degree view, a radius-10 sphere 10 away spans the whole screen height
        let px = camera.projected_radius_px(nalgebra_glm::vec3(0.0, 0.0, -10.0), 10.0, 600.0);
        assert!((px - 300.0).abs() < 1e-3);
    }

    #[test]
    fn floating_origin_keeps_precision_far_away() {
        // 1000 km out, f32 can only step in ~6 cm increments, but the offset is small
        let origin = FloatingOrigin::new(DVec3::new(1.0e6, 0.0, 0.0));
        let rel = origin.relative(DVec3::new(1.0e6 + 0.001, 0.0, 0.0));
        assert!((rel.x - 0.001).abs() < 1e-6);
    }

    #[test]
    fn floating_origin_transform_moves_then_keeps_rotation_scale() {
        let origin = FloatingOrigin::new(DVec3::new(10.0, 0.0, 0.0));
        let scale = nalgebra_glm::scaling(&nalgebra_glm::vec3(2.0, 2.0, 2.0));
        let m = origin.transform(DVec3::new(13.0, 0.0, 0.0), scale);
        let p = m * nalgebra_glm::vec4(1.0, 0.0, 0.0, 1.0);
        assert!((p.x - 5.0).abs() < 1e-6); // scaled to 2, then moved 3 along
    }
}
