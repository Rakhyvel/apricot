//! This module defines the frustum data structure. A frustum is often used to represent the volume that is visible to
//! a camera.

use super::plane::Plane;

#[derive(Debug, Copy, Clone)]
/// A frustum data structure
pub struct Frustum {
    corners: [nalgebra_glm::Vec3; 8],
    planes: [Plane; 6],
}

impl Frustum {
    /// Create a new frustum from it's planes and corners
    pub fn new(planes: [Plane; 6], corners: [nalgebra_glm::Vec3; 8]) -> Self {
        Self { corners, planes }
    }

    /// Create a new frustum from just it's corners
    pub fn from_corners(corners: [nalgebra_glm::Vec3; 8]) -> Self {
        let near_bottom_left = corners[0];
        let near_bottom_right = corners[1];
        let near_top_left = corners[2];
        let near_top_right = corners[3];
        let far_bottom_left = corners[4];
        let far_bottom_right = corners[5];
        let far_top_left = corners[6];
        let far_top_right = corners[7];

        Self::new(
            [
                Self::construct_plane(near_top_left, near_bottom_left, far_bottom_left), // Left
                Self::construct_plane(near_top_right, far_bottom_right, near_bottom_right), // Right
                Self::construct_plane(near_top_left, far_top_left, near_top_right),      // Top
                Self::construct_plane(near_bottom_left, near_bottom_right, far_bottom_left), // Bottom
                Self::construct_plane(near_top_left, near_top_right, near_bottom_left),      // Near
                Self::construct_plane(far_top_left, far_bottom_left, far_top_right),         // Far
            ],
            corners,
        )
    }

    /// Create a frustum from an inverse-proj-view matrix
    pub fn from_inv_proj_view(inv_proj_view: nalgebra_glm::Mat4) -> Self {
        let clip_space_corners: [nalgebra_glm::Vec4; 8] = [
            nalgebra_glm::vec4(-1.0, -1.0, -1.0, 1.0), // near-bottom-left
            nalgebra_glm::vec4(1.0, -1.0, -1.0, 1.0),  // near-bottom-right
            nalgebra_glm::vec4(-1.0, 1.0, -1.0, 1.0),  // near-top-left
            nalgebra_glm::vec4(1.0, 1.0, -1.0, 1.0),   // near-top-right
            nalgebra_glm::vec4(-1.0, -1.0, 0.9999999, 1.0), // far-bottom-left
            nalgebra_glm::vec4(1.0, -1.0, 0.9999999, 1.0), // far-bottom-right
            nalgebra_glm::vec4(-1.0, 1.0, 0.9999999, 1.0), // far-top-left
            nalgebra_glm::vec4(1.0, 1.0, 0.9999999, 1.0), // far-top-right
        ];
        let corners = clip_space_corners.map(|corner| {
            let c = inv_proj_view * corner;
            c.xyz() / c.w
        });
        Self::from_corners(corners)
    }

    /// Transform a frustum by a Mat4
    pub fn transform(&self, matrix: nalgebra_glm::Mat4) -> Self {
        let new_corners = self.corners.map(|c| {
            let c = matrix * nalgebra_glm::vec4(c.x, c.y, c.z, 1.0);
            c.xyz() / c.w
        });
        Self::from_corners(new_corners)
    }

    /// Get a frustum's planes
    pub fn planes(&self) -> &[Plane; 6] {
        &self.planes
    }

    /// Get the corners of a frustum
    pub fn corners(&self) -> [nalgebra_glm::Vec3; 8] {
        self.corners
    }

    fn construct_plane(
        a: nalgebra_glm::Vec3,
        b: nalgebra_glm::Vec3,
        c: nalgebra_glm::Vec3,
    ) -> Plane {
        Plane::from_center_normal(a, (b - a).cross(&(c - a)).normalize())
    }
}

/// A camera's frustum for tests such that at depth `d` the view spans `-d..d` on y and
/// `-d * aspect..d * aspect` on x.
#[cfg(test)]
pub(crate) fn test_frustum(aspect: f32) -> Frustum {
    crate::camera::Camera::new(
        nalgebra_glm::Vec3::zeros(),        // At the origin
        nalgebra_glm::vec3(0.0, 0.0, -1.0), // Looking down -z
        nalgebra_glm::vec3(0.0, 1.0, 0.0),
        crate::camera::ProjectionKind::Perspective {
            fov_rad: std::f32::consts::FRAC_PI_2, // 90 degree vertical view
            near: 0.1,
            far: 100.0,
        },
    )
    .frustum(aspect)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::camera::{Camera, ProjectionKind};
    use nalgebra_glm::{vec3, Vec3};

    const LEFT: usize = 0;
    const RIGHT: usize = 1;
    const TOP: usize = 2;
    const BOTTOM: usize = 3;
    const NEAR: usize = 4;
    const FAR: usize = 5;

    /// A camera with a 90 degree view and a 2:1 aspect, so mixing up x and y shows
    struct View {
        position: Vec3,
        forward: Vec3,
        up: Vec3,
    }

    impl View {
        fn right(&self) -> Vec3 {
            self.forward.cross(&self.up)
        }

        /// The world point `right` across, `up` up, and `ahead` in front of the camera
        fn point(&self, right: f32, up: f32, ahead: f32) -> Vec3 {
            self.position + self.right() * right + self.up * up + self.forward * ahead
        }

        fn frustum(&self) -> Frustum {
            Camera::new(
                self.position,
                self.position + self.forward,
                self.up,
                ProjectionKind::Perspective {
                    fov_rad: std::f32::consts::FRAC_PI_2,
                    near: 0.1,
                    far: 100.0,
                },
            )
            .frustum(2.0)
        }
    }

    fn views() -> [View; 2] {
        [
            View {
                position: Vec3::zeros(),
                forward: vec3(0.0, 0.0, -1.0),
                up: vec3(0.0, 1.0, 0.0),
            },
            // Moved off the origin and turned to look down +x
            View {
                position: vec3(3.0, -2.0, 5.0),
                forward: vec3(1.0, 0.0, 0.0),
                up: vec3(0.0, 1.0, 0.0),
            },
        ]
    }

    fn signed_distance(plane: &Plane, p: Vec3) -> f32 {
        plane.normal().dot(&p) + plane.dist()
    }

    #[test]
    fn corners_sit_on_the_near_and_far_planes() {
        for view in views() {
            let expected = [
                view.point(-0.2, -0.1, 0.1),
                view.point(0.2, -0.1, 0.1),
                view.point(-0.2, 0.1, 0.1),
                view.point(0.2, 0.1, 0.1),
                view.point(-200.0, -100.0, 100.0),
                view.point(200.0, -100.0, 100.0),
                view.point(-200.0, 100.0, 100.0),
                view.point(200.0, 100.0, 100.0),
            ];
            for (i, (got, want)) in view.frustum().corners().iter().zip(expected).enumerate() {
                // The far corners are pulled in a hair, so they're always inside
                let tolerance = if i < 4 { 1e-4 } else { 0.5 };
                assert!(
                    (got - want).norm() < tolerance,
                    "corner {i}: {got:?} vs {want:?}"
                );
            }
        }
    }

    #[test]
    fn planes_face_in() {
        for view in views() {
            let frustum = view.frustum();
            let inside = view.point(1.0, -0.5, 10.0);
            for (i, plane) in frustum.planes().iter().enumerate() {
                assert!(signed_distance(plane, inside) > 0.0, "plane {i} faces out");
            }
        }
    }

    #[test]
    fn a_point_past_one_side_is_only_outside_that_plane() {
        for view in views() {
            let frustum = view.frustum();
            // At 10 ahead, the view spans -20..20 across and -10..10 up
            let cases = [
                (LEFT, view.point(-21.0, 0.0, 10.0)),
                (RIGHT, view.point(21.0, 0.0, 10.0)),
                (TOP, view.point(0.0, 11.0, 10.0)),
                (BOTTOM, view.point(0.0, -11.0, 10.0)),
                (NEAR, view.point(0.0, 0.0, 0.05)),
                (FAR, view.point(0.0, 0.0, 101.0)),
            ];
            for (side, p) in cases {
                for (i, plane) in frustum.planes().iter().enumerate() {
                    let outside = signed_distance(plane, p) < 0.0;
                    assert_eq!(outside, i == side, "point past side {side}, plane {i}");
                }
            }
        }
    }
}
