//! What a draw call is remembered as until the frame is drawn

use nalgebra_glm::{Vec2, Vec4};

use crate::{
    geometry::rectangle::Rectangle,
    render::assets::{MeshId, TextureId},
};

/// How many steps a ring's inner radius is rounded to, so rings that are almost the same share a
/// mesh
pub(super) const INNER_RADIUS_STEPS: u32 = 1024;

/// A draw waiting for the end of the frame, with the layer and clip it was made under
#[derive(Debug, PartialEq)]
pub(super) struct Queued {
    pub(super) layer: i32,
    pub(super) clip: Option<Rectangle>,
    pub(super) command: DrawCommand,
}

/// Which mesh a filled polygon uses. Meshes only get made when the frame is drawn
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum PolygonShape {
    /// A mesh the user made
    Mesh(MeshId),
    /// The built-in square spanning -1 to 1
    Quad,
    /// A regular polygon, or a ring of one when `inner_key` isn't 0. The inner radius is
    /// `inner_key` out of `INNER_RADIUS_STEPS`
    Ngon { sides: u32, inner_key: u32 },
}

impl PolygonShape {
    /// A regular polygon with at least 3 sides, and a ring cut out of it if `inner` is above 0.
    /// `inner` is how far in the ring goes, from 0 to 1
    pub(super) fn ngon(sides: u32, inner: f32) -> Self {
        Self::Ngon {
            sides: sides.max(3),
            inner_key: (inner.clamp(0.0, 1.0) * INNER_RADIUS_STEPS as f32).round() as u32,
        }
    }
}

#[derive(Debug, PartialEq)]
pub(super) enum DrawCommand {
    FillRect {
        rect: Rectangle,
        color: Vec4,
    },
    FillPolygon {
        center: Vec2,
        /// Half-size in pixels along the mesh's X and Y. For a polygon both are just its radius
        scale: Vec2,
        shape: PolygonShape,
        rotation: f32,
        color: Vec4,
    },
    DrawTexture {
        texture_id: TextureId,
        dest: Rectangle,
        /// The part of the texture to draw, in the texture's pixels
        source: Rectangle,
        color_mod: Vec4,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nearly_the_same_ring_shares_a_mesh() {
        let shape = |inner| PolygonShape::ngon(8, inner);
        assert_eq!(shape(0.5), shape(0.5001));
        assert_ne!(shape(0.5), shape(0.52));
        let (sides, inner_key) = (8, 0);
        assert_eq!(shape(-3.0), PolygonShape::Ngon { sides, inner_key });
        let (sides, inner_key) = (8, 1024);
        assert_eq!(shape(3.0), PolygonShape::Ngon { sides, inner_key });
    }
}
