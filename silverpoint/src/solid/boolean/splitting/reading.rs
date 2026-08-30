//! The face being cut, and the curves its boundary runs along.

use crate::solid::boolean::imprints::Imprints;
use crate::solid::geometry::carried::Carried;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use glam::DVec2;

/// What a crossing on a curved stretch of boundary is solved against.
///
/// **What keeps a crossing on the curve rather than on the chord between two of
/// its samples.** A region's boundary is a flattening — a circle imprinted on a
/// face arrives as a hundred corners, see [`ROUNDED`](super::cut::ROUNDED) — so
/// a cut met between two of those corners and solved on the straight run
/// between them lands a whole sagitta off the place the two curves cross.
///
/// That place is a corner of *three* surfaces, and each of the three faces
/// meeting there works it out its own way. A face whose boundary there is
/// straight in its own parameters gets it exactly; a face reading it off a
/// chord does not. The sewing then finds two vertices a sagitta apart where it
/// wanted one, and refuses a body that is perfectly good. Which face gets it
/// wrong depends on the order the surfaces happened to be met in — so it is a
/// place to solve properly rather than a tolerance to widen.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Reading<'a> {
    /// The face being cut, in whose parameters every corner stands.
    pub(crate) on: Surface,
    pub(crate) imprints: &'a Imprints,
    pub(crate) carried: &'a Carried,
}

impl Reading<'_> {
    /// The curve the run at `run` walks, or `None` where it is straight.
    ///
    /// A straight run is left to the chord, the chord being the run — and its
    /// parameter is a length where every other is an angle, which the near way
    /// round in [`Cut::crossing`](super::cut::Cut) has no meaning for.
    pub(super) fn curved(self, run: u32) -> Option<Curve> {
        match self.imprints.curve(run) {
            Curve::Line(_) => None,
            curve => Some(curve),
        }
    }

    /// Where on `curve` the place at `at` stands.
    pub(super) fn along(self, curve: Curve, at: DVec2) -> f64 {
        curve.along(self.on.at(at), self.carried)
    }

    /// Where `curve` stands at `along`, in the face's own parameters, on the
    /// branch `near` was read on.
    pub(super) fn at(self, curve: Curve, along: f64, near: DVec2) -> DVec2 {
        let place = curve.at(along, self.carried);
        self.on.carried(self.on.uv(place), near)
    }
}
