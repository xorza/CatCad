//! A round curve of fixed radius.

use crate::solid::geometry::axis::Axis;
use glam::DVec3;

/// A circle in space, parameterized by the angle round it.
///
/// The frame is an [`Axis`] like every curved surface's, so a circle cut out of
/// a cylinder by a plane can be given the cylinder's own frame and the two then
/// agree about which angle is which. Counterclockwise about
/// [`Axis::direction`], always — an edge walked the other way is a
/// [`Coedge`](crate::solid::topology::coedge::Coedge)'s business, and a curve
/// with one description cannot disagree with itself.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Circle {
    /// The centre is [`Axis::origin`] and the circle lies square to
    /// [`Axis::direction`].
    pub(crate) axis: Axis,
    pub(crate) radius: f64,
}

impl Circle {
    /// Where the angle `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        self.axis.origin + self.axis.radial(t) * self.radius
    }

    /// Which way it heads at `t` — the radius turned a quarter turn forward,
    /// so the walk runs counterclockwise about the axis.
    pub(crate) fn tangent(&self, t: f64) -> DVec3 {
        self.axis.radial(t + std::f64::consts::FRAC_PI_2)
    }
}
