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
    /// Which way is up is a convention rather than a fact, and the three
    /// constants here are the one place in the crate that takes a position on
    /// it — stated rather than left implicit, because a caller modelling the
    /// other way up wants to see that it has to build its own. The fields are
    /// open for that.
    ///
    /// Three, and no more. They are the three a modeller offers to start a
    /// drawing on, each square to the other two and each through the origin;
    /// which of them a document holds, what they are called and what may be
    /// measured off them is the caller's, because a plane here is a frame and
    /// knows nothing about being referred to.
    pub const GROUND: Self = Self {
        origin: DVec3::ZERO,
        x: DVec3::X,
        y: DVec3::NEG_Z,
    };

    /// The upright plane through the origin faced from +Z.
    ///
    /// Its own +x runs along world +X and its +y along world +Y, which puts the
    /// normal at +Z: what is drawn on it stands up, and reads the way it was
    /// drawn to somebody in front of the model.
    pub const FRONT: Self = Self {
        origin: DVec3::ZERO,
        x: DVec3::X,
        y: DVec3::Y,
    };

    /// The upright plane through the origin faced from +X.
    ///
    /// Its own +x runs along world −Z and its +y along world +Y, which puts the
    /// normal at +X. The −Z is what keeps its +x pointing to the *right*: from
    /// +X looking back at the origin, world −Z is the way right runs, so a
    /// drawing on this reads the way it was drawn like the two above rather
    /// than mirrored.
    pub const SIDE: Self = Self {
        origin: DVec3::ZERO,
        x: DVec3::NEG_Z,
        y: DVec3::Y,
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

    /// One world plane and the whole of what it promises.
    ///
    /// Written out rather than read off the constant, which is what makes the
    /// sweep below a test at all: a table taken from the fields would agree
    /// with itself however they were written.
    #[derive(Debug)]
    struct Promised {
        named: &'static str,
        plane: Plane,
        x: DVec3,
        y: DVec3,
        normal: DVec3,
        /// Where [`DRAWN`] lands in the world.
        at: DVec3,
    }

    /// The one point of each plane the sweep works in, chosen off both axes so
    /// that a pair swapped or a sign flipped moves it.
    const DRAWN: DVec2 = DVec2::new(3.0, 2.0);

    /// The three world planes lay their axes where they promise, and the round
    /// trip every frame owes holds on each.
    ///
    /// `flatten` reads the two axes off with dot products, which inverts
    /// `point` only because they are unit and square to each other — so the
    /// round trip is what says the fields are what they promise. For anything
    /// *off* the plane it answers the nearest point of it, which is what a
    /// cursor ray resolved against one always is.
    #[test]
    fn the_world_planes_lay_their_own_axes_where_they_promise() {
        let world = [
            Promised {
                named: "ground",
                plane: Plane::GROUND,
                x: DVec3::X,
                y: DVec3::NEG_Z,
                normal: DVec3::Y,
                at: DVec3::new(3.0, 0.0, -2.0),
            },
            Promised {
                named: "front",
                plane: Plane::FRONT,
                x: DVec3::X,
                y: DVec3::Y,
                normal: DVec3::Z,
                at: DVec3::new(3.0, 2.0, 0.0),
            },
            Promised {
                named: "side",
                plane: Plane::SIDE,
                x: DVec3::NEG_Z,
                y: DVec3::Y,
                normal: DVec3::X,
                at: DVec3::new(0.0, 2.0, -3.0),
            },
        ];

        for Promised {
            named,
            plane,
            x,
            y,
            normal,
            at,
        } in world
        {
            assert_eq!(plane.origin, DVec3::ZERO, "{named} misses the origin");
            assert_eq!(plane.x, x, "{named}'s +x");
            assert_eq!(plane.y, y, "{named}'s +y");
            assert_eq!(plane.x.length(), 1.0, "{named}'s +x is not unit");
            assert_eq!(plane.y.length(), 1.0, "{named}'s +y is not unit");
            assert_eq!(plane.x.dot(plane.y), 0.0, "{named}'s axes are not square");
            assert_eq!(plane.normal(), normal, "{named} faces the wrong way");
            assert_eq!(plane.point(DRAWN), at, "{named} put {DRAWN:?} elsewhere");
            assert_eq!(plane.flatten(at), DRAWN, "{named} did not read it back");
            // Five along the normal flattens to the same spot: that component
            // drops out of both dot products.
            assert_eq!(
                plane.flatten(at + normal * 5.0),
                DRAWN,
                "{named} did not answer with the nearest point of itself",
            );
        }

        // Square to each other as well as each to itself, which is what makes
        // the three a frame rather than three planes that happen to be flat.
        assert_eq!(Plane::GROUND.normal().dot(Plane::FRONT.normal()), 0.0);
        assert_eq!(Plane::FRONT.normal().dot(Plane::SIDE.normal()), 0.0);
        assert_eq!(Plane::SIDE.normal().dot(Plane::GROUND.normal()), 0.0);
    }

    /// A plane elsewhere carries what is drawn on it along.
    #[test]
    fn a_plane_off_the_origin_carries_its_drawing_with_it() {
        let raised = Plane {
            origin: DVec3::new(0.0, 5.0, 0.0),
            ..Plane::GROUND
        };
        let world = DVec3::new(1.0, 5.0, -1.0);
        assert_eq!(raised.point(DVec2::new(1.0, 1.0)), world);
        assert_eq!(raised.flatten(world), DVec2::new(1.0, 1.0));
        assert_eq!(raised.flatten(world + DVec3::Y * 5.0), DVec2::new(1.0, 1.0));
    }
}
