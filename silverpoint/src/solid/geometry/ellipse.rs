//! A round curve whose radius varies.

use crate::solid::geometry::axis::Axis;
use glam::DVec3;

/// An ellipse in space, parameterized by the angle round its own frame.
///
/// What a plane cuts a cylinder in when it meets the axis obliquely, and what
/// two equal cylinders on meeting axes cross in — so it is the first curve here
/// that arrives from an *intersection* rather than from a sketch. See
/// `.notes/KERNEL.md` §7.3.
///
/// **The parameter is not the angle at the centre.** `t` sweeps the frame, so a
/// point is `major·cos t` along the reference and `minor·sin t` across it — the
/// eccentric angle, which is what makes evaluation two multiplications rather
/// than a square root. Nothing downstream reads it as a bearing.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Ellipse {
    /// The centre is [`Axis::origin`], the ellipse lies square to
    /// [`Axis::direction`], and its longer half runs along
    /// [`Axis::reference`].
    pub(crate) axis: Axis,
    /// The longer half. At least [`Ellipse::minor`], which is what makes the
    /// reference direction mean something.
    pub(crate) major: f64,
    pub(crate) minor: f64,
}

impl Ellipse {
    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        let (across, along) = t.sin_cos();
        self.axis.origin
            + self.axis.reference * (self.major * along)
            + self.axis.quarter() * (self.minor * across)
    }
}
