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

    /// The point the surface closes to.
    ///
    /// Named, because [`Axis::origin`] is where it is kept and not what it is
    /// called — a reader of `cone.axis.origin` has to know that a cone's frame
    /// is hung off its apex before the line means anything. The same reading
    /// [`Sphere::centre`](super::sphere::Sphere::centre) is for a sphere.
    pub(crate) fn apex(&self) -> DVec3 {
        self.axis.origin
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

    /// The place on this nearest `at`.
    ///
    /// **Not [`Cone::at`] of [`Cone::uv`]**, which those two are not: `uv` reads
    /// the axial coordinate rather than the ruling, so `at` of it lands at the
    /// same *height* as a place off the surface and not at the foot of the
    /// perpendicular from it. Every other surface here has the two agree, which
    /// is why the projection is written down only for this one.
    ///
    /// **The foot of the perpendicular onto the nearest ruling**, which is what
    /// a cone's nearest place is: the surface is a line turned about the axis,
    /// so dropping onto it is dropping onto that line in the half plane `at`
    /// stands in. Both nappes, like everything else here reads this surface — a
    /// place behind the apex drops onto the far one, at the angle across from
    /// its own.
    pub(crate) fn nearest(&self, at: DVec3) -> DVec3 {
        let (sin, cos) = self.half_angle.sin_cos();
        let reach = self.axis.off(at) * sin + self.axis.along(at) * cos;
        self.axis.origin
            + self.axis.direction * (reach * cos)
            + self.axis.radial(self.axis.angle_of(at)) * (reach * sin)
    }

    /// The cone everywhere `by` off this one, along its own normal.
    ///
    /// **A cone offsets to a cone**, which is what lets a blend down a rim of
    /// one be laid by the same statement every other pair is — see
    /// `.notes/KERNEL.md` §7.5. The half angle does not move and neither does
    /// the axis: what moves is the apex, back along the line by `by / sin θ`,
    /// which is where a line offset in its own plane crosses the axis.
    ///
    /// No refusal, unlike a cylinder's: a cone offset any distance either way
    /// is a cone, the apex merely sliding.
    pub(crate) fn offset(&self, by: f64) -> Self {
        Self {
            axis: Axis::new(
                self.axis.origin - self.axis.direction * (by / self.half_angle.sin()),
                self.axis.direction,
                self.axis.reference,
            ),
            half_angle: self.half_angle,
        }
    }

    /// The place `by` along a ruling from `at`.
    ///
    /// **A ruling is straight and lies on the surface**, a cone being what a
    /// line turned about a line makes — so a walk down one is the walk through
    /// space. `None` for any other way, which is a spiral and what nothing
    /// wants: a setback off a rim runs down a ruling and nowhere else.
    pub(crate) fn walked(&self, at: DVec3, way: DVec3, by: f64) -> Option<DVec3> {
        let down = (at - self.axis.origin).try_normalize()?;
        predicate::parallel(way, down).then(|| at + way * by)
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
    /// **Not [`Cone::nearest`] measured**, and the wedge is the whole of the
    /// difference: that one drops onto the far nappe where this reads past the
    /// apex. Both are the same drop everywhere a caller of either goes.
    ///
    /// The radius is taken off the magnitude of `v`, so both nappes read zero.
    pub(crate) fn off(&self, at: DVec3) -> f64 {
        let radius = self.axis.along(at).abs() * self.half_angle.tan();
        (self.axis.off(at) - radius).abs() * self.half_angle.cos()
    }
}
