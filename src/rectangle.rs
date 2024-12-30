//! This module defines a rectangle

#[derive(Default, Copy, Clone)]
/// A rectangle data structure
pub struct Rectangle {
    pub pos: nalgebra_glm::Vec2,
    pub size: nalgebra_glm::Vec2,
}

impl Rectangle {
    pub fn new(x: f32, y: f32, w: f32, h: f32) -> Self {
        Self {
            pos: nalgebra_glm::vec2(x, y),
            size: nalgebra_glm::vec2(w, h),
        }
    }

    pub fn contains_point(&self, p: &nalgebra_glm::Vec2) -> bool {
        self.pos.x <= p.x
            && self.pos.y <= p.y
            && self.pos.x + self.size.x >= p.x
            && self.pos.y + self.size.y >= p.y
    }
}
