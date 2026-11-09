//! This module defines a rectangle

use nalgebra_glm::vec2;

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
            && self.pos.x + self.size.x > p.x
            && self.pos.y + self.size.y > p.y
    }

    pub fn intersect(&self, other: &Self) -> Self {
        let min_x = self.pos.x.max(other.pos.x);
        let min_y = self.pos.y.max(other.pos.y);

        let max_x = (self.pos.x + self.size.x).min(other.pos.x + other.size.x);
        let max_y = (self.pos.y + self.size.y).min(other.pos.y + other.size.y);

        Self {
            pos: vec2(min_x, min_y),
            size: vec2((max_x - min_x).max(0.0), (max_y - min_y).max(0.0)),
        }
    }
}
