//! A Bounding Volume Hierarchy. A tree of boxes for quickly finding which objects overlap a frustum,
//! a sphere, a ray, or any other shape

use rand::{rngs::StdRng, Rng, SeedableRng};

use super::{aabb::AABB, frustum::Frustum, ray::Ray, sphere::Sphere};

/// A tree of objects and their bounding boxes. For drawing models, `ModelSet` manages one for you
pub struct BVH<Object: Copy> {
    nodes: Vec<Node<Object>>,
    free: Vec<NodeId>,
    root: Option<NodeId>,
    rng: StdRng,
}

/// Refers to an object's spot in a `BVH`, for moving or removing it
#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct NodeId(u32);

impl NodeId {
    fn index(self) -> usize {
        self.0 as usize
    }
}

struct Node<Object> {
    volume: AABB,
    left: Option<NodeId>,
    right: Option<NodeId>,
    parent: Option<NodeId>,
    /// Only leaves hold objects
    object: Option<Object>,
    height: i32,
}

impl<Object> Node<Object> {
    fn is_leaf(&self) -> bool {
        self.left.is_none() && self.right.is_none()
    }

    fn children(&self) -> [Option<NodeId>; 2] {
        [self.left, self.right]
    }
}

impl<Object: Copy> BVH<Object> {
    /// How far past an object's box its leaf's box reaches, so small moves don't need a rebuild
    const AABB_EXTENSION: f32 = 0.1;
    /// How far ahead a moving object's box is stretched, in multiples of its last move
    const AABB_MULTIPLIER: f32 = 2.0;

    /// An empty tree
    pub fn new() -> Self {
        Self {
            nodes: vec![],
            free: vec![],
            root: None,
            rng: StdRng::from_entropy(),
        }
    }

    /// Add an object with the box it fills, getting back its spot in the tree
    pub fn insert(&mut self, object: Object, aabb: AABB) -> NodeId {
        let leaf = self.allocate(Node {
            volume: Self::fatten(aabb),
            left: None,
            right: None,
            parent: None,
            object: Some(object),
            height: 0,
        });
        self.insert_leaf(leaf);
        leaf
    }

    /// Take an object out of the tree
    pub fn remove(&mut self, id: NodeId) {
        assert!(self.node(id).is_leaf(), "can only remove objects");
        self.remove_leaf(id);
        self.release(id);
    }

    /// Tell the tree an object now fills `aabb`, after moving by `displacement`. Gives back whether
    /// the tree had to change
    pub fn move_obj(&mut self, id: NodeId, aabb: &AABB, displacement: &nalgebra_glm::Vec3) -> bool {
        assert!(self.node(id).is_leaf(), "can only move objects");
        if self.node(id).volume.contains(aabb) {
            return false;
        }

        self.remove_leaf(id);

        // Stretch the box ahead of where it's heading
        let mut volume = Self::fatten(*aabb);
        let d = Self::AABB_MULTIPLIER * displacement;
        for axis in 0..3 {
            if d[axis] < 0.0 {
                volume.min[axis] += d[axis];
            } else {
                volume.max[axis] += d[axis];
            }
        }
        self.node_mut(id).volume = volume;

        self.insert_leaf(id);
        true
    }

    /// Every object whose box `hits` says yes to. `hits` is also asked about the bigger boxes that
    /// group objects together, so it should say yes to any box that might hold a hit
    pub fn query<'a>(
        &'a self,
        hits: impl Fn(&AABB) -> bool + 'a,
    ) -> impl Iterator<Item = Object> + 'a {
        let mut stack: Vec<NodeId> = self.root.into_iter().collect();
        std::iter::from_fn(move || {
            while let Some(id) = stack.pop() {
                let node = self.node(id);
                if !hits(&node.volume) {
                    continue;
                }
                stack.extend(node.children().into_iter().flatten());
                if node.object.is_some() {
                    return node.object;
                }
            }
            None
        })
    }

    /// Every object whose box is inside or touching `frustum`
    pub fn iter_frustum<'a>(&'a self, frustum: &'a Frustum) -> impl Iterator<Item = Object> + 'a {
        self.query(move |volume| volume.within_frustum(frustum))
    }

    /// Every object whose box overlaps `sphere`
    pub fn iter_sphere<'a>(&'a self, sphere: &'a Sphere) -> impl Iterator<Item = Object> + 'a {
        self.query(move |volume| volume.within_sphere(sphere))
    }

    /// Every object whose box `ray` passes through, in no particular order
    pub fn iter_ray<'a>(&'a self, ray: &'a Ray) -> impl Iterator<Item = Object> + 'a {
        self.query(move |volume| volume.raycast(ray, 0.0..f32::INFINITY).is_some())
    }

    /// The tree as a graphviz graph, for looking at how it's balanced
    pub fn to_graphviz(&self) -> String {
        let mut dot = String::from("digraph BVH {\n");
        let mut stack: Vec<NodeId> = self.root.into_iter().collect();
        while let Some(id) = stack.pop() {
            let node = self.node(id);
            dot += &format!("  {} [label=\"{:?}\"]\n", id.0, node.volume);
            for child in node.children().into_iter().flatten() {
                dot += &format!("  {} -> {}\n", id.0, child.0);
                stack.push(child);
            }
        }
        dot + "}\n"
    }

    fn fatten(mut aabb: AABB) -> AABB {
        let r = nalgebra_glm::Vec3::repeat(Self::AABB_EXTENSION);
        aabb.min -= r;
        aabb.max += r;
        aabb
    }

    fn insert_leaf(&mut self, leaf: NodeId) {
        let Some(root) = self.root else {
            self.root = Some(leaf);
            return;
        };

        // Walk down to the sibling that grows the tree's boxes the least
        let aabb = self.node(leaf).volume;
        let mut index = root;
        while let (Some(left), Some(right)) = (self.node(index).left, self.node(index).right) {
            let area = self.node(index).volume.surface_area();
            let combined_area = self.node(index).volume.union(aabb).surface_area();

            // Cost of a new parent for this node and the leaf, versus pushing the leaf further down
            let cost = 2.0 * combined_area;
            let inheritance_cost = 2.0 * (combined_area - area);
            let descend_cost = |child: NodeId| {
                let child = self.node(child);
                let grown = aabb.union(child.volume).surface_area();
                let growth = if child.is_leaf() {
                    grown
                } else {
                    grown - child.volume.surface_area()
                };
                growth + inheritance_cost
            };
            let (left_cost, right_cost) = (descend_cost(left), descend_cost(right));

            if cost < left_cost && cost < right_cost {
                break;
            }
            index = if left_cost < right_cost { left } else { right };
        }
        let sibling = index;

        // Give the sibling and the leaf a new parent, in the sibling's old place
        let old_parent = self.node(sibling).parent;
        let flip = self.rng.gen_bool(0.5);
        let (left, right) = if flip {
            (sibling, leaf)
        } else {
            (leaf, sibling)
        };
        let new_parent = self.allocate(Node {
            volume: AABB::empty(),
            left: Some(left),
            right: Some(right),
            parent: old_parent,
            object: None,
            height: self.node(sibling).height + 1,
        });
        match old_parent {
            Some(parent) => self.replace_child(parent, sibling, new_parent),
            None => self.root = Some(new_parent),
        }
        self.node_mut(sibling).parent = Some(new_parent);
        self.node_mut(leaf).parent = Some(new_parent);

        self.adjust_bounds(Some(new_parent));
    }

    /// Unhook a leaf from the tree, freeing the parent it no longer needs. The leaf itself stays
    /// allocated, so it can be put back
    fn remove_leaf(&mut self, leaf: NodeId) {
        let Some(parent) = self.node(leaf).parent else {
            self.root = None;
            return;
        };
        let sibling = if self.node(parent).left == Some(leaf) {
            self.node(parent).right
        } else {
            self.node(parent).left
        }
        .expect("a parent always has two children");

        // The sibling takes the parent's place
        let grandparent = self.node(parent).parent;
        match grandparent {
            Some(grandparent) => self.replace_child(grandparent, parent, sibling),
            None => self.root = Some(sibling),
        }
        self.node_mut(sibling).parent = grandparent;
        self.node_mut(leaf).parent = None;
        self.release(parent);
        self.adjust_bounds(grandparent);
    }

    fn replace_child(&mut self, parent: NodeId, old: NodeId, new: NodeId) {
        let node = self.node_mut(parent);
        if node.left == Some(old) {
            node.left = Some(new);
        } else {
            assert_eq!(node.right, Some(old));
            node.right = Some(new);
        }
    }

    /// Refit every box from `start` up to the root around its children
    fn adjust_bounds(&mut self, start: Option<NodeId>) {
        let mut next = start;
        while let Some(id) = next {
            let node = self.node(id);
            let (left, right) = (
                node.left.expect("internal nodes have two children"),
                node.right.expect("internal nodes have two children"),
            );
            let volume = self.node(left).volume.union(self.node(right).volume);
            let node = self.node_mut(id);
            node.volume = volume;
            next = node.parent;
        }
    }

    fn allocate(&mut self, node: Node<Object>) -> NodeId {
        match self.free.pop() {
            Some(id) => {
                *self.node_mut(id) = node;
                id
            }
            None => {
                self.nodes.push(node);
                NodeId(self.nodes.len() as u32 - 1)
            }
        }
    }

    fn release(&mut self, id: NodeId) {
        let node = self.node_mut(id);
        node.object = None;
        node.left = None;
        node.right = None;
        node.parent = None;
        self.free.push(id);
    }

    fn node(&self, id: NodeId) -> &Node<Object> {
        &self.nodes[id.index()]
    }

    fn node_mut(&mut self, id: NodeId) -> &mut Node<Object> {
        &mut self.nodes[id.index()]
    }
}

impl<Object: Copy> Default for BVH<Object> {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra_glm::vec3;
    use std::collections::HashSet;

    #[test]
    fn frustum_query_finds_just_the_boxes_in_view() {
        // At depth d the view spans -d to d on both x and y
        let frustum = crate::frustum::test_frustum(1.0);
        let at = |x: f32, y: f32, z: f32| {
            AABB::from_min_max(vec3(x - 0.5, y - 0.5, z - 0.5), vec3(x + 0.5, y + 0.5, z + 0.5))
        };
        let in_view = [
            at(0.0, 0.0, -5.0),
            at(3.0, -2.0, -20.0),
            at(-40.0, 30.0, -60.0),
            at(-10.0, 0.0, -10.0), // across the left edge
            at(0.0, 0.0, -100.0),  // across the far plane
        ];
        let out_of_view = [
            at(0.0, 0.0, 5.0),    // behind
            at(0.0, 0.0, 0.8),    // just behind
            at(-14.0, 0.0, -10.0), // off to the left
            at(0.0, 14.0, -10.0), // above
            at(0.0, 0.0, -105.0), // past the far plane
        ];

        let mut bvh = BVH::new();
        for (i, aabb) in in_view.iter().chain(&out_of_view).enumerate() {
            bvh.insert(i, *aabb);
        }
        let found: HashSet<usize> = bvh.iter_frustum(&frustum).collect();
        let expected: HashSet<usize> = (0..in_view.len()).collect();
        assert_eq!(found, expected);
    }

    /// Little boxes scattered over a 40 unit cube, from a fixed seed
    fn boxes(count: usize) -> Vec<AABB> {
        let mut rng = StdRng::seed_from_u64(11);
        (0..count)
            .map(|_| {
                let min = vec3(
                    rng.gen_range(-20.0..20.0),
                    rng.gen_range(-20.0..20.0),
                    rng.gen_range(-20.0..20.0),
                );
                AABB::from_min_max(min, min + vec3(1.0, 1.5, 0.5))
            })
            .collect()
    }

    /// Every query must find everything a brute force check finds. It can find a little more,
    /// since the tree's boxes are padded
    fn check(bvh: &BVH<usize>, live: &[(usize, AABB)]) {
        let sphere = Sphere::new(vec3(3.0, -2.0, 5.0), 9.0);
        let found: HashSet<usize> = bvh.iter_sphere(&sphere).collect();
        for (i, aabb) in live {
            if aabb.within_sphere(&sphere) {
                assert!(found.contains(i), "sphere query missed box {i}");
            }
        }

        // Aimed at a few boxes' centers, from off past the -x side, so each ray hits something
        for (target, aabb) in live.iter().step_by(17) {
            let start = vec3(-30.0, 0.0, 3.0);
            let ray = Ray::new(start, aabb.center() - start);
            let found: HashSet<usize> = bvh.iter_ray(&ray).collect();
            assert!(found.contains(target), "ray at box {target} missed it");
            for (i, aabb) in live {
                if aabb.raycast(&ray, 0.0..f32::INFINITY).is_some() {
                    assert!(found.contains(i), "ray at box {target} missed box {i}");
                }
            }
        }

        // Starts above everything and goes up
        let away = Ray::new(vec3(0.0, 50.0, 0.0), vec3(0.1, 1.0, 0.2));
        assert_eq!(bvh.iter_ray(&away).count(), 0);

        let camera = crate::camera::Camera::new(
            vec3(0.0, 5.0, 40.0),
            vec3(0.0, 0.0, 0.0),
            vec3(0.0, 1.0, 0.0),
            crate::camera::ProjectionKind::default(),
        );
        let frustum = camera.frustum(1.5);
        let found: HashSet<usize> = bvh.iter_frustum(&frustum).collect();
        for (i, aabb) in live {
            if aabb.within_frustum(&frustum) {
                assert!(found.contains(i), "frustum query missed box {i}");
            }
        }

        let everything: HashSet<usize> = bvh.query(|_| true).collect();
        let expected: HashSet<usize> = live.iter().map(|(i, _)| *i).collect();
        assert_eq!(everything, expected);
    }

    #[test]
    fn queries_match_brute_force() {
        let mut bvh = BVH::new();
        let live: Vec<(usize, AABB)> = boxes(200).into_iter().enumerate().collect();
        for (i, aabb) in &live {
            bvh.insert(*i, *aabb);
        }
        check(&bvh, &live);
    }

    #[test]
    fn removing_and_moving_keep_queries_right() {
        let mut bvh = BVH::new();
        let mut live: Vec<(usize, AABB)> = boxes(150).into_iter().enumerate().collect();
        let ids: Vec<NodeId> = live.iter().map(|(i, aabb)| bvh.insert(*i, *aabb)).collect();

        // Drop every third box
        for i in (0..150).step_by(3) {
            bvh.remove(ids[i]);
        }
        live.retain(|(i, _)| i % 3 != 0);

        // Slide the rest along
        for (i, aabb) in live.iter_mut() {
            let shift = vec3(2.5, -1.0, 0.5);
            *aabb = aabb.translate(shift);
            bvh.move_obj(ids[*i], aabb, &shift);
        }
        check(&bvh, &live);
    }

    #[test]
    fn freed_nodes_get_reused() {
        let mut bvh = BVH::new();
        let aabbs = boxes(50);
        for _ in 0..5 {
            let ids: Vec<NodeId> = aabbs
                .iter()
                .enumerate()
                .map(|(i, a)| bvh.insert(i, *a))
                .collect();
            for id in ids {
                bvh.remove(id);
            }
        }
        // 50 leaves and 49 parents at most, however many times it's refilled
        assert!(bvh.nodes.len() <= 99, "grew to {} nodes", bvh.nodes.len());
        assert_eq!(bvh.query(|_| true).count(), 0);
    }
}
