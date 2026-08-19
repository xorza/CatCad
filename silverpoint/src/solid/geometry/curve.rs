//! What an edge is a piece of.

use crate::math::arc;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::line::Line;
use glam::DVec3;

/// One of the curves an edge may lie on.
///
/// Two of them, because two is what the exact tier can currently *make*: a
/// plane meets a plane in a line and everything else here meets a plane or a
/// coaxial neighbour in a circle. The ellipse, and the quartic that a general
/// pair of quadrics gives, arrive with the intersection routines that produce
/// them — see `.notes/KERNEL.md` §7.3.
///
/// Untrimmed, like a [`Surface`](super::surface::Surface): where a curve starts
/// and stops belongs to the [`Edge`](crate::solid::topology::edge::Edge) on it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Curve {
    Line(Line),
    Circle(Circle),
}

impl Curve {
    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        match self {
            Self::Line(line) => line.at(t),
            Self::Circle(circle) => circle.at(t),
        }
    }

    /// The unit direction the curve heads at `t`, along its own parameter.
    pub(crate) fn tangent(&self, t: f64) -> DVec3 {
        match self {
            Self::Line(line) => line.tangent(t),
            Self::Circle(circle) => circle.tangent(t),
        }
    }

    /// How far `at` stands from the curve.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        match self {
            Self::Line(line) => line.off(at),
            Self::Circle(circle) => circle.off(at),
        }
    }

    /// How many straight pieces a stretch of `span` parameter is worth,
    /// flattened no further than `sagitta` from the true curve.
    ///
    /// Straight is exact however coarsely it is cut, so only a circle is asked
    /// — see [`arc::chords`], which is where the rule lives and why it is one
    /// rule rather than one per caller.
    pub(crate) fn steps(&self, span: f64, sagitta: f64) -> usize {
        match self {
            Self::Line(_) => 1,
            Self::Circle(circle) => arc::chords(circle.radius, span, sagitta),
        }
    }
}
