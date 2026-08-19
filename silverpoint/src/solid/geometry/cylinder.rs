//! A surface at a fixed distance from a line.

use crate::math::quadratic;
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

    /// How far along a ray from `from` running `way` it meets this, in order.
    ///
    /// **Square to the axis and nothing else.** A cylinder is a circle extruded,
    /// so a ray meets it exactly where the ray's own shadow on the plane square
    /// to the axis meets that circle — and taking both the ray and the point out
    /// of the axis direction is the whole of the reduction. `None` where the ray
    /// runs along the axis, which has no shadow to speak of, and where it misses
    /// or merely grazes.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Option<[f64; 2]> {
        let off = |at: DVec3| at - self.axis.direction * at.dot(self.axis.direction);
        let (start, along) = (off(from - self.axis.origin), off(way));
        quadratic::roots(
            along.length_squared(),
            2.0 * start.dot(along),
            start.length_squared() - self.radius * self.radius,
        )
    }

    /// Which parameters `at` stands at, its angle read in `(-π, π]`.    /// Which parameters `at` stands at, its angle read in `(-π, π]`.
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
