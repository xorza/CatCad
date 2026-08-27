//! What an edge is a piece of.

use crate::math::arc;
use crate::solid::buckets::Key;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::ellipse::Ellipse;
use crate::solid::geometry::line::Line;
use glam::DVec3;

/// One of the curves an edge may lie on.
///
/// Three of them, because three is what the exact tier can currently *make*: a
/// sketch draws lines and circles, and where two of the surfaces raised off one
/// meet reducibly the answer is a line, a circle or an ellipse. The quartic a
/// general pair of quadrics gives arrives with the routine that parameterizes
/// it — see `.notes/KERNEL.md` §7.3.
///
/// Untrimmed, like a [`Surface`](super::surface::Surface): where a curve starts
/// and stops belongs to the [`Edge`](crate::solid::topology::edge::Edge) on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Curve {
    Line(Line),
    Circle(Circle),
    Ellipse(Ellipse),
}

impl Curve {
    /// The key several of these are filed under — see
    /// [`Buckets`](crate::solid::buckets::Buckets).
    ///
    /// Over the numbers the curve is made of, and sound for the reason
    /// [`Surface::key`](super::surface::Surface::key) is: a crossing met from
    /// either side is one call answering the identical value both times, so
    /// the two key alike bit for bit.
    pub(crate) fn key(&self) -> u64 {
        match self {
            Self::Line(line) => Key::default()
                .word(0)
                .place(line.origin)
                .place(line.direction)
                .done(),
            Self::Circle(circle) => circle
                .axis
                .keyed(Key::default().word(1))
                .float(circle.radius)
                .done(),
            Self::Ellipse(ellipse) => ellipse
                .axis
                .keyed(Key::default().word(2))
                .float(ellipse.major)
                .float(ellipse.minor)
                .done(),
        }
    }

    /// Which parameter puts the curve at `at`, which is [`Curve::at`] read
    /// backwards.
    ///
    /// **The place has to be on the curve**, which every caller has: what asks
    /// is an edge being given the stretch of curve it covers, and the two
    /// places it runs between are places the curve was cut at. A point off the
    /// curve answers with the parameter of the nearest place on it that shares
    /// its bearing, which is a wrong answer rather than no answer — so this is
    /// not a projection and must not be used as one.
    pub(crate) fn along(&self, at: DVec3) -> f64 {
        match self {
            Self::Line(line) => (at - line.origin).dot(line.direction),
            Self::Circle(circle) => circle.axis.angle_of(at),
            // **Not the bearing**, which is what an axis answers and what a
            // circle's parameter happens to be. An ellipse sweeps its frame —
            // see [`Ellipse`] — so the parameter is the bearing of the place
            // with each half divided out, and reading the bearing itself would
            // give a `t` that [`Curve::at`] sends somewhere else entirely.
            Self::Ellipse(ellipse) => {
                let out = at - ellipse.axis.origin;
                (out.dot(ellipse.axis.quarter()) / ellipse.minor)
                    .atan2(out.dot(ellipse.axis.reference) / ellipse.major)
            }
        }
    }

    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        match self {
            Self::Line(line) => line.at(t),
            Self::Circle(circle) => circle.at(t),
            Self::Ellipse(ellipse) => ellipse.at(t),
        }
    }

    /// The unit direction the curve heads at `t`, along its own parameter.
    pub(crate) fn tangent(&self, t: f64) -> DVec3 {
        match self {
            Self::Line(line) => line.tangent(t),
            Self::Circle(circle) => circle.tangent(t),
            Self::Ellipse(ellipse) => ellipse.tangent(t),
        }
    }

    /// How many straight pieces a stretch of `span` parameter is worth,
    /// flattened no further than `sagitta` from the true curve.
    ///
    /// Straight is exact however coarsely it is cut, so only a round curve is
    /// asked — see [`arc::chords`], which is where the rule lives and why it is
    /// one rule rather than one per caller.
    ///
    /// **An ellipse is asked with its longer half**, which is the radius of the
    /// circle it bends no harder than: how far a chord over a parameter step
    /// strays is set by the second derivative, and an ellipse's is at most its
    /// major semi-axis. So the same rule bounds it, conservatively at the flat
    /// ends and exactly at the sharp ones.
    pub(crate) fn steps(&self, span: f64, sagitta: f64) -> usize {
        match self {
            Self::Line(_) => 1,
            Self::Circle(circle) => arc::chords(circle.radius, span, sagitta),
            Self::Ellipse(ellipse) => arc::chords(ellipse.major, span, sagitta),
        }
    }
}
