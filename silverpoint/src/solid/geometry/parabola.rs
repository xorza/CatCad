//! The curve a plane parallel to a ruling cuts out of a cone.

use crate::solid::geometry::axis::Axis;
use glam::DVec3;

/// A parabola in space, parameterized from its own vertex.
///
/// **What a plane parallel to one of a cone's rulings cuts.** A plane leaning
/// less than that meets both of the rulings in its principal plane and cuts an
/// ellipse; one leaning more meets them either side of the apex and cuts a
/// hyperbola; the parabola is the one lean between, where one ruling is met
/// nowhere at all. See [`Meeting::of`](crate::solid::meeting::Meeting).
///
/// **`f·t²` along the reference and `2f·t` across it**, which is `y² = 4fx` in
/// the frame's own coordinates. Chosen for the second derivative, which is the
/// constant `2f`: a chord over a step of `t` strays the same wherever along the
/// curve it is taken, so how finely to cut one is a number rather than a walk —
/// see [`Curve::steps`](super::curve::Curve). The parameter is not an arc
/// length and not an angle, and nothing downstream reads it as either.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Parabola {
    /// The vertex is [`Axis::origin`], the curve lies square to
    /// [`Axis::direction`], and it opens along [`Axis::reference`].
    pub(crate) axis: Axis,
    /// How far the focus stands from the vertex, which is a quarter of the
    /// distance to the directrix.
    pub(crate) focal: f64,
}

impl Parabola {
    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        self.axis.origin
            + self.axis.reference * (self.focal * t * t)
            + self.axis.quarter() * (2.0 * self.focal * t)
    }

    /// Which parameter puts it at `at`, which is [`Parabola::at`] read
    /// backwards.
    ///
    /// Off the coordinate across the axis, which is linear in `t` where the one
    /// along it is a square and would answer two parameters for one place.
    pub(crate) fn along(&self, at: DVec3) -> f64 {
        (at - self.axis.origin).dot(self.axis.quarter()) / (2.0 * self.focal)
    }

    /// The semi-latus rectum, which is the half-width of the curve at the
    /// focus and what says how hard it bends about its vertex.
    ///
    /// `2f` for a parabola, where a hyperbola's is `b²/a` — the one number the
    /// two share, and what a cut across either reads them by.
    pub(crate) fn latus(&self) -> f64 {
        2.0 * self.focal
    }
}
