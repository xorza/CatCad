//! A line in the world to spin about, and what a place reads about one.

use glam::DVec3;

/// A line in the world to spin about: a point on it, and the unit way it runs.
///
/// [`Axle`](super::axle::Axle) borne onto the plane its drawing lies on — see
/// [`Axle::borne`](super::axle::Axle::borne).
/// Named apart from that one because the two are read in different frames, and
/// a reader holding the wrong one would spin a solid about a line of the
/// drawing's own two coordinates.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Spindle {
    pub(crate) origin: DVec3,
    /// Unit, unlike [`Axle::along`](super::axle::Axle::along), which the kernel
    /// normalizes for itself.
    pub(crate) direction: DVec3,
}

impl Spindle {
    /// Where `at` stands about the line: how far along it, how far out, and at
    /// what angle from `reference`.
    ///
    /// The angle is measured the way a revolve sweeps — right-handed about the
    /// direction — so what it hands back is the sector's own vocabulary.
    pub(crate) fn reads(self, reference: DVec3, at: DVec3) -> Reading {
        let across = self.across(at);
        Reading {
            up: self.direction.dot(at - self.origin),
            radius: across.length(),
            angle: across
                .dot(self.square(reference))
                .atan2(across.dot(reference)),
        }
    }

    /// Where the place `up` along the line and `radius` out from it stands at
    /// `angle` from `reference`.
    pub(crate) fn spun(self, reference: DVec3, reading: Reading, angle: f64) -> DVec3 {
        let round = reference * angle.cos() + self.square(reference) * angle.sin();
        self.origin + self.direction * reading.up + round * reading.radius
    }

    /// The way the spin goes at `angle`, unit — which is the circle's own
    /// tangent there, and so the way a handle riding it points.
    pub(crate) fn tangent(self, reference: DVec3, angle: f64) -> DVec3 {
        self.square(reference) * angle.cos() - reference * angle.sin()
    }

    /// The unit way out to `at`, or `None` where it stands *on* the line and
    /// there is no way out to it.
    pub(crate) fn out(self, at: DVec3) -> Option<DVec3> {
        self.across(at).try_normalize()
    }

    /// How far out from the line `at` stands, as a direction and a length in
    /// one.
    fn across(self, at: DVec3) -> DVec3 {
        let out = at - self.origin;
        out - self.direction * self.direction.dot(out)
    }

    /// A quarter turn on from `reference`, which is what an angle about the
    /// line is read against and turned through.
    fn square(self, reference: DVec3) -> DVec3 {
        self.direction.cross(reference)
    }
}

/// Where a place stands about a [`Spindle`].
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Reading {
    pub(crate) up: f64,
    pub(crate) radius: f64,
    pub(crate) angle: f64,
}
