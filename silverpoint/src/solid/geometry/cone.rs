//! A surface whose distance from a line grows with the distance along it.

use crate::math::quadratic;
use crate::number::predicate;
use crate::number::tolerance::PLACED;
use crate::solid::geometry::axis::Axis;
use glam::{DVec2, DVec3};

/// Everywhere at `half_angle` from the line [`Cone::axis`] runs along, measured
/// at its apex.
///
/// **Both nappes**, as the quadric is: the surface reaches either side of the
/// apex, and which side a face covers is the sign of its `v`. A kernel that
/// held one nappe would have a surface that is not a quadric, and every
/// intersection with it would need a case for that.
///
/// Parameterized by angle and distance from the apex along the axis, in that
/// order — `u` round and `v` along, like every curved surface here.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Cone {
    /// The apex is [`Axis::origin`], so `v` is measured from it.
    pub(crate) axis: Axis,
    /// Half the angle at the apex, in `(0, π/2)`. At zero the surface is a
    /// line and at a right angle it is a plane; neither is a cone.
    pub(crate) half_angle: f64,
}

impl Cone {
    /// The cone a line through two places sweeps about `axis`, where `along`
    /// says how far each stands down the line and `radius` how far off it.
    ///
    /// **The apex is where that line meets the axis**, which is what a cone is
    /// measured from, and the frame is turned to run out of the apex toward the
    /// two places — so both stand at a positive `v` and a face on the surface
    /// lies in one nappe.
    ///
    /// **The angle is read off the place standing further out**, which is what
    /// a pair with one end *at* the apex needs: that end gives a rise of
    /// nothing and an angle of nought over nought.
    ///
    /// `None` where the two stand at one radius or at one place along the axis.
    /// The first sweeps a cylinder and the second a disc, and a caller wanting
    /// either asks for it by name.
    pub(crate) fn through(axis: Axis, along: [f64; 2], radius: [f64; 2]) -> Option<Self> {
        let rise = along[1] - along[0];
        let run = radius[0] - radius[1];
        if predicate::touching(rise.abs(), PLACED) || predicate::touching(run.abs(), PLACED) {
            return None;
        }
        let apex = along[0] + radius[0] * rise / run;
        let far = usize::from(radius[1] > radius[0]);
        let reach = along[far] - apex;
        Some(Self {
            axis: Axis::new(
                axis.origin + axis.direction * apex,
                axis.direction * reach.signum(),
                axis.reference,
            ),
            half_angle: (radius[far] / reach.abs()).atan(),
        })
    }

    /// Where the parameters `uv` land: `u` radians round, `v` from the apex.
    pub(crate) fn at(&self, uv: DVec2) -> DVec3 {
        let radius = uv.y * self.half_angle.tan();
        self.axis.origin + self.axis.direction * uv.y + self.axis.radial(uv.x) * radius
    }

    /// How far along a ray from `from` running `way` it meets this, in order.
    ///
    /// **Both nappes**, like everything else here reads this surface: a place
    /// is on the cone where the angle it makes with the axis is the half angle,
    /// and that says nothing about which side of the apex it fell. A caller
    /// that wants one nappe reads the `v` of what it got and drops the negative
    /// — see [`Cone::uv`], which is where that sign lives.
    ///
    /// Squared, so the cosine is squared and the sign goes with it, which is
    /// exactly why both nappes come back: `(p·w)² = cos²θ · |p|²` about the
    /// apex.
    pub(crate) fn met_by(&self, from: DVec3, way: DVec3) -> Option<[f64; 2]> {
        let start = from - self.axis.origin;
        let (leaning, aimed) = (start.dot(self.axis.direction), way.dot(self.axis.direction));
        let narrow = self.half_angle.cos() * self.half_angle.cos();
        quadratic::roots(
            aimed * aimed - narrow * way.length_squared(),
            2.0 * (leaning * aimed - narrow * start.dot(way)),
            leaning * leaning - narrow * start.length_squared(),
        )
    }

    /// Which parameters `at` stands at, its angle read in `(-π, π]`.
    ///
    /// Read off the axis rather than off the surface: a point on the cone has
    /// one `v`, and one off it is answered for by the parameters of the point
    /// of the axis beside it. The apex says nothing, like a cylinder's axis.
    ///
    /// **The far nappe reaches out on the other side of the axis**, because a
    /// negative `v` scales the radius negative as well. So the direction to a
    /// point there is the reverse of the one its own angle names, and reading
    /// the angle off the reversed direction is what keeps this the inverse of
    /// [`Cone::at`] on both nappes rather than only on one.
    pub(crate) fn uv(&self, at: DVec3) -> DVec2 {
        let along = self.axis.along(at);
        let out = if along < 0.0 {
            self.axis.origin - at
        } else {
            at - self.axis.origin
        };
        DVec2::new(self.axis.bearing(out), along)
    }

    /// Which way the surface faces at `uv` — the direction its own parameters
    /// wind about, which is out of the nappe and back towards the apex.
    ///
    /// It turns over across the apex, because the parameterization does: `∂u`
    /// reverses with the sign of `v` and `∂v` does not. A face lies in one
    /// nappe, so no face ever meets the turn.
    pub(crate) fn normal(&self, uv: DVec2) -> DVec3 {
        let facing = self.axis.radial(uv.x) * self.half_angle.cos()
            - self.axis.direction * self.half_angle.sin();
        if uv.y < 0.0 { -facing } else { facing }
    }

    /// How far `at` stands from the surface.
    ///
    /// The perpendicular distance to the ruling nearest it, which is the
    /// distance to the cone itself everywhere but in the wedge behind the apex
    /// — where the nearest place on the surface is the apex, and this reads
    /// further. What asks is a check on geometry meant to be *on* the surface,
    /// so that wedge is not a case it meets.
    ///
    /// The radius is taken off the magnitude of `v`, so both nappes read zero.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        let radius = self.axis.along(at).abs() * self.half_angle.tan();
        (self.axis.off(at) - radius).abs() * self.half_angle.cos()
    }
}
