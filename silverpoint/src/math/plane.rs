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
    /// The horizontal plane through the origin, in a world where +Y is up.
    ///
    /// Its own +x runs along world +X and its +y along world −Z, which puts the
    /// normal at +Y: seen from above, what is drawn on it reads the way it was
    /// drawn, and anything modelled from it rises out of it.
    ///
    /// Which way is up is a convention rather than a fact, and this constant is
    /// the one place in the crate that takes a position on it — stated here
    /// rather than left implicit, because a caller modelling the other way up
    /// wants to see that it has to build its own. The fields are open for that.
    pub const GROUND: Self = Self {
        origin: DVec3::ZERO,
        x: DVec3::X,
        y: DVec3::NEG_Z,
    };

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

#[cfg(test)]
mod tests {
    use super::*;

    /// The ground plane's orientation, and the round trip every frame owes.
    ///
    /// `flatten` reads the two axes off with dot products, which inverts
    /// `point` only because they are unit and square to each other — so the
    /// round trip is what says the fields are what they promise. For anything
    /// *off* the plane it answers the nearest point of it, which is what a
    /// cursor ray resolved against one always is.
    #[test]
    fn the_ground_plane_lays_its_own_y_along_negative_z() {
        assert_eq!(Plane::GROUND.point(DVec2::ZERO), DVec3::ZERO);
        assert_eq!(
            Plane::GROUND.point(DVec2::new(3.0, 0.0)),
            DVec3::new(3.0, 0.0, 0.0)
        );
        // Its +y runs away from a viewer above it, so what is drawn lies flat
        // rather than standing on edge.
        assert_eq!(
            Plane::GROUND.point(DVec2::new(0.0, 2.0)),
            DVec3::new(0.0, 0.0, -2.0)
        );
        assert_eq!(Plane::GROUND.normal(), DVec3::Y, "the ground faces up");

        // A plane elsewhere carries what is drawn on it along.
        let raised = Plane {
            origin: DVec3::new(0.0, 5.0, 0.0),
            ..Plane::GROUND
        };
        let world = DVec3::new(1.0, 5.0, -1.0);
        assert_eq!(raised.point(DVec2::new(1.0, 1.0)), world);
        assert_eq!(raised.flatten(world), DVec2::new(1.0, 1.0));
        // Five above it flattens to the same spot: the normal component drops
        // out of both dot products.
        assert_eq!(raised.flatten(world + DVec3::Y * 5.0), DVec2::new(1.0, 1.0));
    }
}
