//! What a face is a piece of.

use crate::math::bounds::Bounds;
use crate::math::plane::Plane;
use crate::number::predicate;
use crate::number::tolerance::ALIGNED;
use crate::solid::geometry::cone::Cone;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::sphere::Sphere;
use glam::{DVec2, DVec3};

/// One of the surfaces a face may lie on.
///
/// The four *natural quadrics*, and they arrive together because they are one
/// algebra: a pencil of quadrics does not care which of the four it was handed,
/// so plane-meets-cone is not separate work from plane-meets-cylinder. The set
/// is also exactly what extruding and revolving a drawing of lines and circles
/// can make, which is why it is the set a kernel for this application wants.
///
/// A closed enum rather than a trait, because intersection dispatches on a
/// *pair* of surfaces. That is a matrix, and a matrix wants a `match` on the
/// pair; a trait needs double dispatch to express it and then cannot be
/// exhaustive. Adding a surface here is a compile error at every site that
/// takes a pair apart, which is the reminder wanted.
///
/// Every one of them is exact: the parameters below are the surface, not a fit
/// to one, and nothing evaluated off them carries a tolerance. See
/// `.notes/KERNEL.md` §4.1 for where that stops.
/// Where a ray met a surface, as distances along it.
///
/// Inline rather than a list, for the reason
/// [`Curves`](crate::solid::meeting::Curves) beside it is one: a boolean sounds
/// a body once per region and a document is rebuilt on every frame of a drag
/// through the drawing under it, so an answer that reached the heap would reach
/// it a few thousand times a second.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub(crate) struct Crossings {
    along: [f64; 2],
    count: u8,
}

impl Crossings {
    /// How far along the ray each was, in order.
    pub(crate) fn along(&self) -> &[f64] {
        &self.along[..self.count as usize]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Surface {
    /// The same [`Plane`] a sketch is carried into the world by, which is what
    /// lets an extrusion's base face literally hold the drawing's own frame.
    Plane(Plane),
    Cylinder(Cylinder),
    Cone(Cone),
    Sphere(Sphere),
}

impl Surface {
    /// Where the parameters `uv` land in the world.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Plane(plane) => plane.point(uv),
            Self::Cylinder(cylinder) => cylinder.at(uv),
            Self::Cone(cone) => cone.at(uv),
            Self::Sphere(sphere) => sphere.at(uv),
        }
    }

    /// Which parameters `at` stands at, and the nearest place on the surface
    /// for anything off it.
    ///
    /// Closed form for all four, which is the whole reason there are no
    /// parameter-space curves anywhere in this kernel: a curve that already has
    /// a description in space does not get a second one that could disagree
    /// with it. See `.notes/KERNEL.md` §4.7.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        match self {
            Self::Plane(plane) => plane.flatten(at),
            Self::Cylinder(cylinder) => cylinder.uv(at),
            Self::Cone(cone) => cone.uv(at),
            Self::Sphere(sphere) => sphere.uv(at),
        }
    }

    /// The unit normal at `uv`, pointing the way the surface's own parameters
    /// wind about.
    ///
    /// Which is to say `∂u × ∂v`, normalized. Stating it that way is what makes
    /// the winding of a mesh decidable: a triangle wound counterclockwise in
    /// the parameters is wound counterclockwise about this, so a face that
    /// knows whether material is on this side knows which way to hand its
    /// triangles out.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        match self {
            Self::Plane(plane) => plane.normal(),
            Self::Cylinder(cylinder) => cylinder.normal(uv),
            Self::Cone(cone) => cone.normal(uv),
            Self::Sphere(sphere) => sphere.normal(uv),
        }
    }

    /// How far along a ray from `from` running `way` it meets this, in order,
    /// and how many times.
    ///
    /// **At most twice, because every surface here is a quadric.** A plane is
    /// the degenerate one and answers once; the other three answer twice or
    /// not at all — a graze counts as not at all, for the reason
    /// [`roots`](crate::math::quadratic::roots) gives.
    ///
    /// Distances along `way` rather than places, because what asks is a ray
    /// cast counting crossings ahead of where it started: which of them are
    /// ahead is a comparison on this and nothing else. Unnormalized `way` is
    /// fine and the answer is in units of it.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Crossings {
        let two = |along: Option<[f64; 2]>| match along {
            Some(along) => Crossings { along, count: 2 },
            None => Crossings::default(),
        };
        match self {
            Self::Plane(plane) => {
                let leaning = way.dot(plane.normal());
                // Along the plane, so it meets it nowhere or everywhere — and
                // a ray *in* a plane crosses out of nothing, which is the
                // answer either way.
                //
                // **To within [`ALIGNED`] rather than exactly**, which is not
                // caution but arithmetic: a ray a hair off parallel crosses the
                // plane genuinely, at a distance of one over that hair, and a
                // place read off the ray that far out has no significant digits
                // left in it. The crossing is real and unusable, so it is not
                // reported. A caller that leans on this is a caller casting
                // rays along a body's own faces, which is what four directions
                // are for.
                if predicate::touching(leaning.abs(), ALIGNED) {
                    return Crossings::default();
                }
                Crossings {
                    along: [(plane.origin - from).dot(plane.normal()) / leaning, 0.0],
                    count: 1,
                }
            }
            Self::Cylinder(cylinder) => two(cylinder.met_by(from, way)),
            Self::Cone(cone) => two(cone.met_by(from, way)),
            Self::Sphere(sphere) => two(sphere.met_by(from, way)),
        }
    }

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Plane(plane) => (at - plane.origin).dot(plane.normal()).abs(),
            Self::Cylinder(cylinder) => cylinder.off(at),
            Self::Cone(cone) => cone.off(at),
            Self::Sphere(sphere) => sphere.off(at),
        }
    }

    /// The box a face on this surface fills, given the box its boundary fills.
    ///
    /// **The boundary is enough for three of the four**, on one argument. Every
    /// world coordinate of a plane, a cylinder or a cone runs monotonically
    /// along one of its two parameters — a plane along both, a cylinder along
    /// its height, a cone along its ruling — so the extreme of one over a region
    /// is taken somewhere on that region's edge. Where a coordinate is *not*
    /// monotone, which is one square to a cylinder's axis peaking at a single
    /// angle, the region's boundary crosses that angle anyway: a region spanning
    /// it is connected, so its edge is somewhere on every angle it covers.
    ///
    /// A sphere has no such parameter and the argument fails on it — the top of
    /// a dome is interior, and the box of the rim below misses it entirely. So a
    /// face on one is given the whole sphere, which is coarse and is not wrong.
    pub(crate) fn fills(&self, boundary: Bounds) -> Bounds {
        match self {
            Self::Plane(_) | Self::Cylinder(_) | Self::Cone(_) => boundary,
            Self::Sphere(sphere) => Bounds::about(sphere.centre(), sphere.radius),
        }
    }

    /// Whether the first parameter runs round the surface, so that a face on it
    /// could wrap.
    ///
    /// What the split in `.notes/KERNEL.md` §4.4 is decided by, and what tells
    /// a mesher whether an angle traced along a loop has to be unwrapped.
    pub(crate) fn round(&self) -> bool {
        match self {
            Self::Plane(_) => false,
            Self::Cylinder(_) | Self::Cone(_) | Self::Sphere(_) => true,
        }
    }
}
