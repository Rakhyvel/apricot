//! This module defines an Axis Aligned Bounding Box.

use core::f32;

use super::{frustum::Frustum, plane::Plane, ray::Ray, sphere::Sphere};

#[derive(Debug, Copy, Clone)]
#[allow(unused)]
/// # Axis Aligned Bounding Box.
/// An axis aligned bounding box is defined by two vertices: the `min`, which contains the smallest bounds on each axis,
/// and the `max`, which contains the maximum values for each axis.
///
/// AABBs are very simple yet powerful, and are used for a lot of things.
pub struct AABB {
    pub min: nalgebra_glm::Vec3,
    pub max: nalgebra_glm::Vec3,
}

#[allow(unused)]
impl AABB {
    pub fn new() -> Self {
        Self {
            min: nalgebra_glm::vec3(f32::MAX, f32::MAX, f32::MAX),
            max: nalgebra_glm::vec3(f32::MIN, f32::MIN, f32::MIN),
        }
    }

    pub fn from_min_max(min: nalgebra_glm::Vec3, max: nalgebra_glm::Vec3) -> Self {
        Self { min, max }
    }

    /// Create an AABB from an iterator of points. This is slow!
    pub fn from_points(points: impl IntoIterator<Item = nalgebra_glm::Vec3>) -> Self {
        let mut retval = AABB::new();
        retval.expand_to_fit(points);
        retval
    }

    /// Union two AABBs to form a single AABB that contains both
    pub fn union(&self, b: AABB) -> AABB {
        AABB::from_min_max(
            nalgebra_glm::min2(&self.min, &b.min),
            nalgebra_glm::max2(&self.max, &b.max),
        )
    }

    /// This function finds the _surface_ area of an AABB.
    pub fn area(&self) -> f32 {
        let d = self.max - self.min;
        2.0 * (d.x * d.y + d.y * d.z + d.z * d.x)
    }

    /// Determines whether or not an AABB intersects with a frustum
    pub fn within_frustum(&self, frustum: &Frustum, debug: bool) -> bool {
        let mut i = 0;
        for plane in frustum.planes() {
            let vmax = self.get_furthest_corner(plane);
            let value = plane.normal().dot(&vmax) + plane.dist();
            // println!("{}", value);
            if value < 0.0 {
                let bounding_sphere = self.bounding_sphere();
                let in_sphere = bounding_sphere.within_frustum(frustum);
                if !in_sphere && debug {
                    println!(
                        "i:{}\nmax:{:?}\nmin:{:?}\nplane:{:?}\nsphere:{:?}",
                        i,
                        self.max,
                        self.min,
                        plane,
                        self.bounding_sphere()
                    );
                }
                return in_sphere;
            }
            i += 1;
        }
        true
    }

    /// Determines whether or not an AABB intersects with a sphere
    pub fn within_sphere(&self, sphere: &Sphere) -> bool {
        let mut radius_squared = sphere.radius.powf(2.0);

        if (sphere.center.x < self.min.x) {
            radius_squared -= (sphere.center.x - self.min.x).powf(2.0);
        }
        if (sphere.center.x > self.max.x) {
            radius_squared -= (sphere.center.x - self.max.x).powf(2.0);
        }
        if (sphere.center.y < self.min.y) {
            radius_squared -= (sphere.center.y - self.min.y).powf(2.0);
        }
        if (sphere.center.y > self.max.y) {
            radius_squared -= (sphere.center.y - self.max.y).powf(2.0);
        }
        if (sphere.center.z < self.min.z) {
            radius_squared -= (sphere.center.z - self.min.z).powf(2.0);
        }
        if (sphere.center.z > self.max.z) {
            radius_squared -= (sphere.center.z - self.max.z).powf(2.0);
        }

        radius_squared > 0.0
    }

    /// Determines if a ray intersects an AABB
    pub fn raycast(&self, ray: &Ray) -> bool {
        let inv = nalgebra_glm::vec3(1.0 / ray.dir.x, 1.0 / ray.dir.y, 1.0 / ray.dir.z);

        let tx1 = (self.min.x - ray.origin.x) * inv.x;
        let tx2 = (self.max.x - ray.origin.x) * inv.x;

        let mut tmin = tx1.min(tx2);
        let mut tmax = tx1.max(tx2);

        let ty1 = (self.min.y - ray.origin.y) * inv.y;
        let ty2 = (self.max.y - ray.origin.y) * inv.y;

        tmin = tmin.max(ty1.min(ty2));
        tmax = tmax.min(ty1.max(ty2));

        let tz1 = (self.min.z - ray.origin.z) * inv.z;
        let tz2 = (self.max.z - ray.origin.z) * inv.z;

        tmin = tmin.max(tz1.min(tz2));
        tmax = tmax.min(tz1.max(tz2));

        tmax >= tmin && tmax >= 0.0
    }

    /// Calculates the bounding sphere for an AABB
    pub fn bounding_sphere(&self) -> Sphere {
        let center = self.center();
        Sphere::new(center, nalgebra_glm::distance(&center, &self.min))
    }

    /// Finds the centerpoint of an AABB
    pub fn center(&self) -> nalgebra_glm::Vec3 {
        (self.max + self.min) * 0.5
    }

    /// Moves an AABB by a vector
    pub fn translate(&self, center: nalgebra_glm::Vec3) -> Self {
        Self {
            min: self.min + center,
            max: self.max + center,
        }
    }

    /// Scales an AABB by a certain amount
    pub fn scale(&self, factor: nalgebra_glm::Vec3) -> Self {
        Self {
            min: self.min.component_mul(&factor),
            max: self.max.component_mul(&factor),
        }
    }

    /// Expands an AABB to fit all points in some iterator of points. This is __slow__!
    pub fn expand_to_fit(&mut self, points: impl IntoIterator<Item = nalgebra_glm::Vec3>) {
        for corner in points.into_iter() {
            self.min = nalgebra_glm::min2(&self.min, &corner);
            self.max = nalgebra_glm::max2(&self.max, &corner);
        }
    }

    /// Finds the midpoint of the positive z plane. Useful for shadow mapping.
    pub fn pos_z_plane_midpoint(&self) -> nalgebra_glm::Vec4 {
        let bottom_left = nalgebra_glm::vec4(self.min.x, self.min.y, self.max.z, 1.0);
        let top_right = nalgebra_glm::vec4(self.max.x, self.max.y, self.max.z, 1.0);
        0.5 * (bottom_left + top_right)
    }

    /// Transforms the min and max points of an AABB by a Mat4
    pub fn transform(&mut self, matrix: nalgebra_glm::Mat4) {
        self.min = (matrix * nalgebra_glm::vec4(self.min.x, self.min.y, self.min.z, 1.0)).xyz();
        self.max = (matrix * nalgebra_glm::vec4(self.max.x, self.max.y, self.max.z, 1.0)).xyz();
    }

    // I dont think this one is used
    // pub fn intersect_z(&mut self, other: &AABB) {
    //     self.min.z = self.min.z.min(other.min.z);
    //     self.max.z = self.max.z.max(other.max.z);
    // }

    /// Determines whether two AABBs intersect
    pub fn intersects(&self, other: &AABB) -> bool {
        // Check for separation in the x-axis
        if self.max.x < other.min.x || self.min.x > other.max.x {
            return false;
        }
        // Check for separation in the y-axis
        if self.max.y < other.min.y || self.min.y > other.max.y {
            return false;
        }
        // Check for separation in the z-axis
        if self.max.z < other.min.z || self.min.z > other.max.z {
            return false;
        }

        // No separation found, the AABBs intersect
        true
    }

    /// Determines whether this AABB _fully_ contains the other AABB
    pub fn contains(&self, other: &AABB) -> bool {
        let mut result = true;
        result = result && self.min.x <= other.min.x;
        result = result && self.min.y <= other.min.y;
        result = result && self.min.z <= other.min.z;
        result = result && other.max.x <= self.max.x;
        result = result && other.max.y <= self.max.y;
        result = result && other.max.z <= self.max.z;
        result
    }

    /// Determines whether this AABB contains the point
    pub fn contains_point(&self, point: nalgebra_glm::Vec3) -> bool {
        self.min.x <= point.x
            && self.min.y <= point.y
            && self.min.z <= point.z
            && self.max.x <= point.x
            && self.max.y <= point.y
            && self.max.z <= point.z
    }

    /// Produces the corners of an AABB. This is _SLOW_!
    pub fn corners(&self) -> [nalgebra_glm::Vec3; 8] {
        [
            nalgebra_glm::Vec3::new(self.min.x, self.min.y, self.min.z),
            nalgebra_glm::Vec3::new(self.max.x, self.min.y, self.min.z),
            nalgebra_glm::Vec3::new(self.min.x, self.max.y, self.min.z),
            nalgebra_glm::Vec3::new(self.max.x, self.max.y, self.min.z),
            nalgebra_glm::Vec3::new(self.min.x, self.min.y, self.max.z),
            nalgebra_glm::Vec3::new(self.max.x, self.min.y, self.max.z),
            nalgebra_glm::Vec3::new(self.min.x, self.max.y, self.max.z),
            nalgebra_glm::Vec3::new(self.max.x, self.max.y, self.max.z),
        ]
    }

    /// Given a normal-plane, returns the further point from the plane along it's normal
    fn get_furthest_corner(&self, plane: &Plane) -> nalgebra_glm::Vec3 {
        nalgebra_glm::vec3(
            if plane.normal().x > 0.0 {
                self.max.x
            } else {
                self.min.x
            },
            // Y axis
            if plane.normal().y > 0.0 {
                self.max.y
            } else {
                self.min.y
            },
            // Z axis
            if plane.normal().z > 0.0 {
                self.max.z
            } else {
                self.min.z
            },
        )
    }
}
