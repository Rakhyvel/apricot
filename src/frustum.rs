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
                Self::construct_plane(near_top_left, near_bottom_left, far_bottom_left, false), // Left
                Self::construct_plane(near_top_right, far_bottom_right, near_bottom_right, false), // Right
                Self::construct_plane(near_top_left, far_top_left, near_top_right, false), // Top
                Self::construct_plane(near_bottom_left, near_bottom_right, far_bottom_left, false), // Bottom
                Self::construct_plane(near_top_left, near_top_right, near_bottom_left, false), // Near
                Self::construct_plane(far_top_left, far_bottom_left, far_top_right, false), // Far
            ],
            corners,
        )
    }

    /// Create a frustum from an inverse-proj-view matrix
    pub fn from_inv_proj_view(inv_proj_view: nalgebra_glm::Mat4, _debug: bool) -> Self {
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
        let corners_w: Vec<nalgebra_glm::Vec4> = clip_space_corners
            .iter()
            .map(|&corner| inv_proj_view * corner)
            .collect();
        // Divide by w
        let corners: Vec<nalgebra_glm::Vec3> = corners_w
            .iter()
            .map(|&corner| corner.xyz() / corner.w) // Convert from Vec4 to Vec3
            .collect();
        Self::from_corners(corners.try_into().unwrap())
    }

    /// Transform a frustum by a Mat4
    pub fn transform(&self, matrix: nalgebra_glm::Mat4) -> Self {
        let new_corners_w: Vec<nalgebra_glm::Vec4> = self
            .corners
            .iter()
            .map(|c| matrix * nalgebra_glm::vec4(c.x, c.y, c.z, 1.0))
            .collect();
        let new_corners: Vec<nalgebra_glm::Vec3> =
            new_corners_w.iter().map(|&c| c.xyz() / c.w).collect();
        Self::from_corners(new_corners.try_into().unwrap())
    }

    /// Get a frustum's planes
    pub fn planes(&self) -> &[Plane; 6] {
        &self.planes
    }

    /// Get a mutable reference to a frustum's planes
    pub fn planes_mut(&mut self) -> &mut [Plane; 6] {
        &mut self.planes
    }

    /// Get the corners of a frustum
    pub fn corners(&self) -> [nalgebra_glm::Vec3; 8] {
        self.corners
    }

    fn construct_plane(
        a: nalgebra_glm::Vec3,
        b: nalgebra_glm::Vec3,
        c: nalgebra_glm::Vec3,
        debug: bool,
    ) -> Plane {
        let plane_normal = (b - a).cross(&(c - a)).normalize();
        let plane = Plane::from_center_normal(a, plane_normal);
        if debug {
            println!("{:?}", plane);
        }
        plane
    }
}
