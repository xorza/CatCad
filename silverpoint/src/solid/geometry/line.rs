//! A straight curve.

use glam::DVec3;

/// A straight line, parameterized by distance along itself.
///
/// The direction is unit, so the parameter is a length and the two ends of an
/// edge on it are how long that edge is apart. Nothing here trims it — where a
/// line starts and stops is the [`Edge`](crate::solid::topology::edge::Edge)'s,
/// which is what lets two edges share one line.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Line {
    pub(crate) origin: DVec3,
    /// Unit, the way the line runs.
    pub(crate) direction: DVec3,
}

impl Line {
    /// Where the parameter `t` lands.
    pub(crate) fn at(&self, t: f64) -> DVec3 {
        self.origin + self.direction * t
    }

    /// How far `at` stands from the line.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        let out = at - self.origin;
        (out - self.direction * out.dot(self.direction)).length()
    }
}
