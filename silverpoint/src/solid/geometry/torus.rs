//! A surface at a fixed distance from a circle.
//!
//! **The first of the fitted tier** — see `.notes/KERNEL.md` §4.1. Everything
//! before it is a quadric, and two quadrics meet in a curve that can be written
//! down; a torus is a quartic surface, and what it meets anything in is marched
//! and fitted rather than parameterized.
//!
//! **Not a [`Surface`](super::surface::Surface) arm yet**, and the reason is
//! worth stating: that enum's `met_by` answers at most two crossings, because
//! every surface in it so far is a quadric. A ray meets a torus in *four*. So
//! the arm wants a quartic root solve and a wider answer, and both belong with
//! the `Natural`/`Fitted` split (§4.6) rather than ahead of it.
#![allow(dead_code)]

use crate::solid::geometry::axis::Axis;
use glam::{DVec2, DVec3};

/// Everywhere `minor` from the circle of radius `major` about
/// [`Torus::axis`].
///
/// Parameterized by two angles, in the order every curved surface here keeps:
/// `u` runs round the axis and `v` round the tube. Both wrap, so a face on one
/// may cover neither in full — the rule §4.4 states about wrapping applies twice
/// over here, where a cylinder only has one way to close.
///
/// **A ring torus and no other**, which is `minor < major`: at equal radii the
/// tube closes on the axis and the surface pinches, and past that it passes
/// through itself. Neither is a boundary a solid can be made of, and a caller
/// holding one has a modelling error rather than a shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Torus {
    /// The tube's centre circle lies in the plane square to this, about
    /// [`Axis::origin`].
    pub(crate) axis: Axis,
    /// How far the tube's centre stands from the axis.
    pub(crate) major: f64,
    /// How far the surface stands from that centre.
    pub(crate) minor: f64,
}

impl Torus {
    /// Where the parameters `uv` land: `u` radians round the axis, `v` round
    /// the tube from the outside.
    ///
    /// `v` of nought is the outer equator and `v` of π the inner one, which is
    /// what makes the surface's own normal at `v = 0` point away from the axis
    /// — the same way every other curved surface here faces out of the solid
    /// its parameters wind about.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        let (up, out) = uv.y.sin_cos();
        self.axis.origin
            + self.axis.radial(uv.x) * (self.major + self.minor * out)
            + self.axis.direction * (self.minor * up)
    }

    /// Which parameters `at` stands at, both angles read in `(-π, π]`.
    ///
    /// The nearest place of the surface for anything off it, which is what
    /// makes this the inverse of [`Torus::at`] rather than a projection with
    /// conditions. The axis itself says nothing about `u`, and the tube's own
    /// centre circle says nothing about `v`.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        let of = self.tube(at);
        DVec2::new(self.axis.angle_of(at), of.y.atan2(of.x))
    }

    /// Which way the surface faces at `uv` — away from the tube's own centre,
    /// which is the direction its second parameter winds about.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let (up, out) = uv.y.sin_cos();
        self.axis.radial(uv.x) * out + self.axis.direction * up
    }

    /// How far `at` stands from the surface.
    ///
    /// A [`Cylinder::off`](super::cylinder::Cylinder) one level up: that one
    /// takes its radius off the distance from a *line*, and this takes its own
    /// off the distance from a *circle*.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        (self.tube(at).length() - self.minor).abs()
    }

    /// Where `at` stands in the plane of the tube's own circle: out from that
    /// circle first, then along the axis.
    ///
    /// **Two dimensions and not three**, which is the whole of why a torus is
    /// measured as cheaply as a cylinder. The nearest place of the centre
    /// circle to anything is the one at the same angle round the axis, so
    /// turning that angle away leaves a plane the tube is a circle in.
    fn tube(&self, at: DVec3) -> DVec2 {
        DVec2::new(self.axis.off(at) - self.major, self.axis.along(at))
    }
}
