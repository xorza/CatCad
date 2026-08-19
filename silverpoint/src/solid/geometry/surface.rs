//! What a face is a piece of.

use crate::math::plane::Plane;
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

    /// How far `at` stands from the surface, never signed.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Plane(plane) => (at - plane.origin).dot(plane.normal()).abs(),
            Self::Cylinder(cylinder) => cylinder.off(at),
            Self::Cone(cone) => cone.off(at),
            Self::Sphere(sphere) => sphere.off(at),
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
