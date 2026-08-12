//! A half-line through the world — what a pointer becomes once it leaves the
//! screen.

use glam::Vec3;

/// A start and a unit direction.
///
/// The unit length is the point: a distance along the ray is a distance in the
/// world, so hits from different primitives are comparable without knowing how
/// each was found.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray {
    pub origin: Vec3,
    /// Unit length. [`Ray::new`] is what guarantees it.
    pub direction: Vec3,
}

impl Ray {
    /// A ray from `origin` along `direction`, which need not arrive
    /// normalized.
    pub fn new(origin: Vec3, direction: Vec3) -> Self {
        Self {
            origin,
            direction: direction.normalize(),
        }
    }

    /// The point `distance` along the ray.
    pub fn at(&self, distance: f32) -> Vec3 {
        self.origin + self.direction * distance
    }
}
