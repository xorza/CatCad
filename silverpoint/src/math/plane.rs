//! A flat frame in space, and the two-way trip between it and the world.

use glam::{DVec2, DVec3};

/// A plane in the world: an origin, and the directions its own two axes run
/// along.
///
/// What answers where flat coordinates live. A sketch's are flat and say
/// nothing about the world, so something has to, and it is this — the only
/// place the two coordinate systems meet.
///
/// `f64`, like everything else here. The world a model is *drawn* in is a
/// renderer's and may well be `f32`, but this is the world it is *solved*
/// against, and rounding the frame rounds the geometry hung off it. Callers
/// crossing into a renderer convert where they cross, which is a boundary worth
/// being able to see.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    pub origin: DVec3,
    /// World direction of the plane's +x. Expected to be unit length.
    pub x: DVec3,
    /// World direction of the plane's +y. Expected to be unit length and square
    /// to [`Plane::x`] — [`Plane::flatten`] reads the two off with dot
    /// products, which inverts [`Plane::point`] only if they are.
    pub y: DVec3,
}

impl Plane {
    /// Where a point of the plane lands in the world.
    pub fn point(&self, point: DVec2) -> DVec3 {
        self.origin + self.x * point.x + self.y * point.y
    }

    /// Where a world position lands on the plane, in the plane's own
    /// coordinates.
    ///
    /// The inverse of [`Plane::point`] for anything already on it, and the
    /// nearest point of it for anything off — which is what a cursor ray
    /// resolved against a plane always is.
    pub fn flatten(&self, world: DVec3) -> DVec2 {
        let out = world - self.origin;
        DVec2::new(out.dot(self.x), out.dot(self.y))
    }

    /// The unit normal. Which face it points out of follows from the order of
    /// the axes, and matters to nothing that asks.
    pub fn normal(&self) -> DVec3 {
        self.x.cross(self.y).normalize()
    }
}
