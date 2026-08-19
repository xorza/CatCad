//! What an edge is a piece of.

use crate::math::arc;
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
