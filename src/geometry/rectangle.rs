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

#[cfg(test)]
mod tests {
    use super::*;

    /// 4 wide and 2 tall, with its top left at (1, 3)
    fn rect() -> Rectangle {
        Rectangle::new(1.0, 3.0, 4.0, 2.0)
    }

    fn assert_rect(r: Rectangle, x: f32, y: f32, w: f32, h: f32) {
        assert_eq!((r.pos, r.size), (vec2(x, y), vec2(w, h)));
    }

    #[test]
    fn contains_the_top_and_left_edges_but_not_the_bottom_and_right() {
        for p in [(1.0, 3.0), (3.0, 4.0), (4.99, 4.99), (1.0, 4.99), (4.99, 3.0)] {
            assert!(rect().contains_point(&vec2(p.0, p.1)), "{p:?}");
        }
        for p in [(5.0, 4.0), (3.0, 5.0), (0.99, 4.0), (3.0, 2.99), (5.0, 5.0)] {
            assert!(!rect().contains_point(&vec2(p.0, p.1)), "{p:?}");
        }
    }

    #[test]
    fn intersect_keeps_the_overlap_either_way_round() {
        let other = Rectangle::new(3.0, 2.0, 5.0, 2.5);
        assert_rect(rect().intersect(&other), 3.0, 3.0, 2.0, 1.5);
        assert_rect(other.intersect(&rect()), 3.0, 3.0, 2.0, 1.5);
    }

    #[test]
    fn intersect_with_something_inside_is_the_inside_one() {
        let inside = Rectangle::new(2.0, 3.5, 1.0, 1.0);
        assert_rect(rect().intersect(&inside), 2.0, 3.5, 1.0, 1.0);
        assert_rect(inside.intersect(&rect()), 2.0, 3.5, 1.0, 1.0);
    }

    #[test]
    fn intersect_with_nothing_in_common_is_empty() {
        // Off to the side it still overlaps up and down, so only the width goes to 0
        let beside = Rectangle::new(10.0, 3.0, 2.0, 2.0);
        assert_eq!(rect().intersect(&beside).size, vec2(0.0, 2.0));
        let below = Rectangle::new(1.0, 8.0, 4.0, 2.0);
        assert_eq!(rect().intersect(&below).size, vec2(4.0, 0.0));
        let far_off = Rectangle::new(20.0, 20.0, 1.0, 1.0);
        assert_eq!(rect().intersect(&far_off).size, vec2(0.0, 0.0));
        // Sharing just an edge leaves no area either
        let touching = Rectangle::new(5.0, 3.0, 2.0, 2.0);
        assert_eq!(rect().intersect(&touching).size.x, 0.0);
    }
}
