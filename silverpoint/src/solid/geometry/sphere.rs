//! A surface at a fixed distance from a point.

use crate::solid::geometry::axis::Axis;
use glam::{DVec2, DVec3};

/// Everywhere `radius` from the point [`Sphere::axis`] runs through.
///
/// Framed by an [`Axis`] rather than by a bare centre, because a sphere still
/// has to say which way its parameters run: `u` is the angle round the axis and
/// `v` the angle up from square to it, so the two poles sit where the axis
/// leaves the surface. That the frame is arbitrary is exactly why it is stored
/// — two spheres in the same place with different frames name the same points
/// by different parameters, and a face has to know which.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Sphere {
    /// The centre is [`Axis::origin`].
    pub(crate) axis: Axis,
    pub(crate) radius: f64,
}

impl Sphere {
    /// Where it stands.
    ///
    /// Named, because [`Axis::origin`] is where it is kept and not what it is
    /// called — a reader of `sphere.axis.origin` has to know that a sphere's
    /// frame is hung off its centre before the line means anything.
    pub(crate) fn centre(&self) -> DVec3 {
        self.axis.origin
    }

    /// Where the parameters `uv` land: `u` radians round, `v` up from the
    /// equator towards the pole the axis points at.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        let (up, out) = uv.y.sin_cos();
        self.axis.origin + (self.axis.radial(uv.x) * out + self.axis.direction * up) * self.radius
    }

    /// Which parameters `at` stands at, its angle round read in `(-π, π]` and
    /// its angle up in `[-π/2, π/2]`.
    ///
    /// The nearest point of the surface for anything off it, which is what
    /// makes this the inverse of [`Sphere::at`] rather than a projection with
    /// conditions. The centre says nothing, and the poles say nothing about
    /// `u`.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        let out = at - self.axis.origin;
        let reach = out.length();
        let up = if reach > 0.0 {
            (out.dot(self.axis.direction) / reach)
                .clamp(-1.0, 1.0)
                .asin()
        } else {
            0.0
        };
        DVec2::new(self.axis.angle_of(at), up)
    }

    /// Which way the surface faces at `uv` — away from the centre, which is the
    /// direction its own parameters wind about.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let (up, out) = uv.y.sin_cos();
        self.axis.radial(uv.x) * out + self.axis.direction * up
    }

    /// How far `at` stands from the surface.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        (at.distance(self.axis.origin) - self.radius).abs()
    }
}
