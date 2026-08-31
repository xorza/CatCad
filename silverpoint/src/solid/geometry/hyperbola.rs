//! The curve a plane leaning past a cone's own rulings cuts out of it.

use crate::solid::geometry::axis::Axis;
use glam::DVec3;

/// One branch of a hyperbola in space, parameterized from its own vertex.
///
/// **A branch and not the pair**, which is what every reader here needs: a cut
/// divides a face into two sides, and the whole of a hyperbola divides a plane
/// into three. The two branches of one meeting are two curves of it — see
/// [`Curves`](crate::solid::meeting::Curves) — and they are the same numbers
/// with the reference reversed.
///
/// **What a plane leaning past a cone's rulings cuts.** It meets the two
/// rulings of its principal plane on opposite sides of the apex, so the section
/// reaches both nappes and each vertex is on one of them. A plane parallel to
/// the axis is the plainest case of it, and milling a flat down a taper is
/// where a document reaches one.
///
/// **`a·cosh t` along the reference and `b·sinh t` across it**, which is
/// `x²/a² − y²/b² = 1` in the frame's own coordinates, and the branch the
/// reference points into. The parameter is not an angle and not an arc length;
/// how hard the branch bends grows with it — see [`Hyperbola::bending`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hyperbola {
    /// The centre is [`Axis::origin`], the curve lies square to
    /// [`Axis::direction`], and this branch's vertex stands [`Hyperbola::major`]
    /// along [`Axis::reference`].
    pub(crate) axis: Axis,
    /// Half the distance between the two vertices.
    pub(crate) major: f64,
    /// Half the conjugate axis, which is `b` in the asymptotes `y = ±bx/a`.
    /// No place of the curve stands on that axis — what does at `b` across the
    /// centre is where the asymptotes stand.
    pub(crate) minor: f64,
}

impl Hyperbola {
    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        self.axis.origin
            + self.axis.reference * (self.major * t.cosh())
            + self.axis.quarter() * (self.minor * t.sinh())
    }

    /// Which parameter puts it at `at`, which is [`Hyperbola::at`] read
    /// backwards.
    ///
    /// Off the coordinate across the axis, whose sine is one to one where the
    /// cosine along it is even and would answer two parameters for one place.
    pub(crate) fn along(&self, at: DVec3) -> f64 {
        ((at - self.axis.origin).dot(self.axis.quarter()) / self.minor).asinh()
    }

    /// How hard it bends at its hardest over the stretch `bounds`.
    ///
    /// **The second derivative rather than a radius**, which is what
    /// [`arc::chords`](crate::math::arc::chords) reads whatever it is called:
    /// how far a chord over a step of parameter strays goes as an eighth of the
    /// step squared times this. A circle read round its angle answers its own
    /// radius, and a branch read this way answers `hypot(a cosh t, b sinh t)` —
    /// which grows without bound, so the stretch decides and a span alone
    /// cannot.
    ///
    /// Taken at whichever end stands further from the vertex, the reading
    /// growing with `|t|`.
    pub(crate) fn bending(&self, bounds: [f64; 2]) -> f64 {
        let far = bounds[0].abs().max(bounds[1].abs());
        (self.major * far.cosh()).hypot(self.minor * far.sinh())
    }

    /// The semi-latus rectum, which is the half-width of the branch at its
    /// focus and what says how hard it bends about its vertex.
    ///
    /// `b²/a`, where a parabola's is `2f` — the one number the two share, and
    /// what a cut across either reads them by.
    pub(crate) fn latus(&self) -> f64 {
        self.minor * self.minor / self.major
    }
}
