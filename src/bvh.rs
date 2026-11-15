//! A Bounding Volume Hierarchy. A tree of boxes for quickly finding which objects overlap a frustum,
//! a sphere, a ray, or any other shape

use super::{aabb::AABB, frustum::Frustum, ray::Ray, sphere::Sphere};

/// A tree of objects and their bounding boxes. For drawing models, `ModelSet` manages one for you
pub struct BVH<Object: Copy> {
    nodes: Vec<Node<Object>>,
    free: Vec<NodeId>,
    root: Option<NodeId>,
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
    /// The longest path down to a leaf. Leaves are 0
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
        let new_parent = self.allocate(Node {
            volume: AABB::empty(),
            left: Some(sibling),
            right: Some(leaf),
            parent: old_parent,
            object: None,
            height: 0, // refit below
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

    /// Rebalance and refit every node from `start` up to the root
    fn adjust_bounds(&mut self, start: Option<NodeId>) {
        let mut next = start;
        while let Some(id) = next {
            let id = self.balance(id);
            self.refit(id);
            next = self.node(id).parent;
        }
    }

    /// Fit an internal node's box and height around its children
    fn refit(&mut self, id: NodeId) {
        let (left, right) = self.children_of(id);
        let (left, right) = (self.node(left), self.node(right));
        let volume = left.volume.union(right.volume);
        let height = 1 + left.height.max(right.height);
        let node = self.node_mut(id);
        node.volume = volume;
        node.height = height;
    }

    /// If one of `a`'s children is more than a level taller than the other, rotate it up into
    /// `a`'s place. Gives back the node now in `a`'s place
    fn balance(&mut self, a: NodeId) -> NodeId {
        if self.node(a).is_leaf() {
            return a;
        }
        let (left, right) = self.children_of(a);
        let lean = self.node(right).height - self.node(left).height;
        if lean > 1 {
            self.rotate_up(a, right)
        } else if lean < -1 {
            self.rotate_up(a, left)
        } else {
            a
        }
    }

    /// Lift `child` into its parent `a`'s place. `child` keeps its taller child and takes `a`
    /// as its other one, and `a` takes `child`'s shorter child
    fn rotate_up(&mut self, a: NodeId, child: NodeId) -> NodeId {
        let (first, second) = self.children_of(child);
        let (keep, give) = if self.node(first).height > self.node(second).height {
            (first, second)
        } else {
            (second, first)
        };

        let parent = self.node(a).parent;
        match parent {
            Some(parent) => self.replace_child(parent, a, child),
            None => self.root = Some(child),
        }
        let lifted = self.node_mut(child);
        lifted.parent = parent;
        lifted.left = Some(a);
        lifted.right = Some(keep);

        self.replace_child(a, child, give);
        self.node_mut(a).parent = Some(child);
        self.node_mut(give).parent = Some(a);

        self.refit(a);
        self.refit(child);
        child
    }

    fn children_of(&self, id: NodeId) -> (NodeId, NodeId) {
        let node = self.node(id);
        (
            node.left.expect("internal nodes have two children"),
            node.right.expect("internal nodes have two children"),
        )
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
    use rand::{rngs::StdRng, Rng, SeedableRng};
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

    /// Every parent link, height, and box agrees with the children, and there are `leaves` objects
    fn assert_valid(bvh: &BVH<usize>, leaves: usize) {
        let Some(root) = bvh.root else {
            assert_eq!(leaves, 0);
            return;
        };
        assert_eq!(bvh.node(root).parent, None);
        let mut found = 0;
        let mut stack = vec![root];
        while let Some(id) = stack.pop() {
            let node = bvh.node(id);
            match (node.left, node.right) {
                (None, None) => {
                    assert!(node.object.is_some(), "leaf {id:?} has no object");
                    assert_eq!(node.height, 0, "leaf {id:?}");
                    found += 1;
                }
                (Some(left), Some(right)) => {
                    let (l, r) = (bvh.node(left), bvh.node(right));
                    assert_eq!(l.parent, Some(id), "{left:?}'s parent");
                    assert_eq!(r.parent, Some(id), "{right:?}'s parent");
                    assert_eq!(node.height, 1 + l.height.max(r.height), "{id:?}'s height");
                    let union = l.volume.union(r.volume);
                    assert!(node.volume.min == union.min && node.volume.max == union.max, "{id:?}'s box");
                    stack.extend([left, right]);
                }
                _ => panic!("{id:?} has one child"),
            }
        }
        assert_eq!(found, leaves);
    }

    /// The longest path from the root to a leaf, counting both ends
    fn depth(bvh: &BVH<usize>) -> usize {
        fn below(bvh: &BVH<usize>, id: NodeId) -> usize {
            let node = bvh.node(id);
            1 + node.children().into_iter().flatten().map(|c| below(bvh, c)).max().unwrap_or(0)
        }
        bvh.root.map_or(0, |root| below(bvh, root))
    }

    /// How many nodes a query for a half unit sphere around each box's center looks at, on average
    fn average_visits(bvh: &BVH<usize>, aabbs: &[AABB]) -> f32 {
        let total: usize = aabbs
            .iter()
            .map(|aabb| {
                let sphere = Sphere::new(aabb.center(), 0.5);
                let mut visits = 0;
                let mut stack: Vec<NodeId> = bvh.root.into_iter().collect();
                while let Some(id) = stack.pop() {
                    visits += 1;
                    let node = bvh.node(id);
                    if node.volume.within_sphere(&sphere) {
                        stack.extend(node.children().into_iter().flatten());
                    }
                }
                visits
            })
            .sum();
        total as f32 / aabbs.len() as f32
    }

    /// As deep as an AVL tree can get, and small queries only look at a few nodes per level
    fn assert_balanced(bvh: &BVH<usize>, aabbs: &[AABB]) {
        let n = aabbs.len() as f32;
        let nodes = 2.0 * n - 1.0;
        let max_depth = (1.44 * (nodes + 2.0).log2()).floor() as usize + 1;
        assert!(depth(bvh) <= max_depth, "{} deep, at most {max_depth}", depth(bvh));
        let max_visits = 4.0 * n.log2();
        let visits = average_visits(bvh, aabbs);
        assert!(visits <= max_visits, "{visits} visits per query, at most {max_visits}");
    }

    fn unit_box_at(x: f32, y: f32, z: f32) -> AABB {
        AABB::from_min_max(vec3(x, y, z), vec3(x + 1.0, y + 1.0, z + 1.0))
    }

    #[test]
    fn far_apart_clusters_end_up_on_their_own_sides() {
        // Inserted taking turns, so the tree has to sort them out as it goes
        let aabbs: Vec<AABB> = (0..64)
            .map(|i| {
                let cluster_x = if i % 2 == 0 { 0.0 } else { 1000.0 };
                unit_box_at(cluster_x + (i / 2 % 8) as f32 * 2.0, 0.0, (i / 16) as f32 * 2.0)
            })
            .collect();
        let mut bvh = BVH::new();
        for (i, aabb) in aabbs.iter().enumerate() {
            bvh.insert(i, *aabb);
        }
        assert_valid(&bvh, aabbs.len());

        // Each side of the root holds one cluster, about 16 wide
        let (left, right) = bvh.children_of(bvh.root.unwrap());
        for side in [left, right] {
            let volume = bvh.node(side).volume;
            let width = volume.max.x - volume.min.x;
            assert!(width < 50.0, "one side of the root spans {width} across x");
        }
    }

    #[test]
    fn random_inserts_stay_balanced() {
        let mut rng = StdRng::seed_from_u64(5);
        let aabbs: Vec<AABB> = (0..1000)
            .map(|_| {
                let x = rng.gen_range(-50.0..50.0);
                unit_box_at(x, rng.gen_range(-50.0..50.0), rng.gen_range(-50.0..50.0))
            })
            .collect();
        let mut bvh = BVH::new();
        for (i, aabb) in aabbs.iter().enumerate() {
            bvh.insert(i, *aabb);
        }
        assert_valid(&bvh, aabbs.len());
        assert_balanced(&bvh, &aabbs);
    }

    #[test]
    fn inserting_in_a_line_stays_balanced() {
        let aabbs: Vec<AABB> = (0..1000).map(|i| unit_box_at(i as f32 * 2.0, 0.0, 0.0)).collect();
        let mut bvh = BVH::new();
        for (i, aabb) in aabbs.iter().enumerate() {
            bvh.insert(i, *aabb);
        }
        assert_valid(&bvh, aabbs.len());
        assert_balanced(&bvh, &aabbs);
    }

    #[test]
    fn inserting_a_grid_row_by_row_stays_balanced() {
        let aabbs: Vec<AABB> = (0..1024)
            .map(|i| unit_box_at((i % 32) as f32 * 2.0, 0.0, (i / 32) as f32 * 2.0))
            .collect();
        let mut bvh = BVH::new();
        for (i, aabb) in aabbs.iter().enumerate() {
            bvh.insert(i, *aabb);
        }
        assert_valid(&bvh, aabbs.len());
        assert_balanced(&bvh, &aabbs);
    }

    #[test]
    fn removing_from_a_line_stays_balanced() {
        let aabbs: Vec<AABB> = (0..1000).map(|i| unit_box_at(i as f32 * 2.0, 0.0, 0.0)).collect();
        let mut bvh = BVH::new();
        let ids: Vec<NodeId> = aabbs.iter().enumerate().map(|(i, a)| bvh.insert(i, *a)).collect();
        // Take out the whole first half
        for id in &ids[..500] {
            bvh.remove(*id);
        }
        assert_valid(&bvh, 500);
        assert_balanced(&bvh, &aabbs[500..]);
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
        assert_valid(&bvh, live.len());
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
