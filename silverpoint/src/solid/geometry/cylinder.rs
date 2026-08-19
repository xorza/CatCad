//! A surface at a fixed distance from a line.

use crate::solid::geometry::axis::Axis;
use glam::{DVec2, DVec3};

/// Everywhere `radius` from the line [`Cylinder::axis`] runs along.
///
/// Parameterized by angle and height, in that order, so that `u` is what runs
/// round it — the convention every curved surface here keeps, and what lets one
/// rule say which faces may wrap.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cylinder {
    pub(crate) axis: Axis,
    pub(crate) radius: f64,
}

impl Cylinder {
    /// Where the parameters `uv` land: `u` radians round, `v` along the axis.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        self.axis.origin + self.axis.radial(uv.x) * self.radius + self.axis.direction * uv.y
    }

    /// Which parameters `at` stands at, its angle read in `(-π, π]`.
    ///
    /// Exact for anything on the surface and the nearest point of it for
    /// anything off, which is what makes this an inversion rather than a
    /// projection with conditions. The one place it says nothing is the axis
    /// itself, where every angle is as near as every other.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        DVec2::new(self.axis.angle_of(at), self.axis.along(at))
    }

    /// Which way the surface faces at `uv` — away from the axis, which is the
    /// direction its own parameters wind about.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        self.axis.radial(uv.x)
    }

    /// How far `at` stands from the surface.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        (self.axis.off(at) - self.radius).abs()
    }
}
