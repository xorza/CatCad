//! A surface at a fixed distance from a circle.
//!
//! **The first of the fitted tier** — see `.notes/KERNEL.md` §4.1. Everything
//! before it is a quadric, and two quadrics meet in a curve that can be written
//! down; a torus is a quartic surface, and what it meets anything in is marched
//! and fitted rather than parameterized.

use crate::inline::Inline;
use crate::math::quartic;
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

    /// How far along a ray from `from` running `way` it meets this, in order.
    ///
    /// **Four, where every quadric answers two**, and that is the whole of what
    /// makes a torus the first of the fitted tier: a ray through the hole of
    /// one crosses the tube twice on the way in and twice on the way out.
    ///
    /// **Squared once to reach the surface's own equation.** How far a place
    /// stands from the axis has a square root in it, and the torus is written
    /// about that distance — so `|x|² + R² − r² = 2R·s` is the equation with
    /// the root still in it, and squaring both sides leaves
    /// `(|x|² + R² − r²)² = 4R²(|x|² − (x·d)²)`, which is a quartic in the ray's
    /// own parameter and has no root anywhere.
    ///
    /// A graze counts for none, as it does for every other surface here — see
    /// [`quartic::roots`], which is where that is argued.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Inline<f64, 4> {
        let start = from - self.axis.origin;
        let (leaning, aimed) = (start.dot(self.axis.direction), way.dot(self.axis.direction));
        // `|x(t)|²` gathered into `sweep·t² + across·t + out`.
        let (sweep, across, out) = (
            way.length_squared(),
            2.0 * start.dot(way),
            start.length_squared(),
        );
        let held = out + self.major * self.major - self.minor * self.minor;
        let turning = 4.0 * self.major * self.major;
        quartic::roots(
            sweep * sweep,
            2.0 * sweep * across,
            across * across + 2.0 * sweep * held + turning * (aimed * aimed - sweep),
            2.0 * across * held + turning * (2.0 * aimed * leaning - across),
            held * held + turning * (leaning * leaning - out),
        )
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
