//! One step of a timeline, and the kinds there are.

use silverpoint::Sketch;

use crate::timeline::FeatureId;

/// One thing that was done to build the document.
///
/// What a step *is* rather than what it made: a sketch feature holds the
/// geometry someone drew, and says nothing about where that geometry ends up in
/// the world — which follows from the plane it names, and is worked out afresh
/// every time the timeline is replayed.
///
/// The sketch is held by value and its plane by reference, and that asymmetry
/// is the whole of why a plane can be moved. A sketch's coordinates are its
/// plane's own, so they are the same coordinates wherever that plane goes;
/// storing the plane here instead would mean two places to keep in step, and
/// moving one would silently leave the other where it was.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Feature {
    /// A flat frame to draw on.
    Plane(Datum),
    /// A sketch, and the plane it is drawn on.
    Sketch { on: FeatureId, sketch: Sketch },
}

impl Feature {
    /// The steps this one is built on, which have to be earlier than it.
    ///
    /// Named for [`Constraint::referents`](silverpoint::Constraint), which
    /// answers the same question one level down — what a thing names, so that
    /// whatever holds them both can check it is still there.
    pub(crate) fn referents(&self) -> impl Iterator<Item = FeatureId> {
        match self {
            Feature::Plane(datum) => datum.referents(),
            Feature::Sketch { on, .. } => Some(*on).into_iter(),
        }
    }

    /// The sketch this step holds, or `None` where it is not a sketch.
    pub(crate) fn sketch(&self) -> Option<&Sketch> {
        match self {
            Feature::Sketch { sketch, .. } => Some(sketch),
            Feature::Plane(_) => None,
        }
    }
}

/// Where a plane gets its frame from.
///
/// One kind so far, and the enum is here for the second: a datum offset from
/// another plane is what makes moving one worth doing, and it arrives as an arm
/// added to this and to the walk that resolves it. Anything richer — a plane
/// free to sit anywhere, at any angle — is deliberately not planned: it is the
/// same shape with six numbers instead of one, and the gesture that edits it is
/// a gizmo rather than a drag.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Datum {
    /// The world's own ground, which depends on nothing and so is where every
    /// chain of planes ends.
    Ground,
}

impl Datum {
    /// The planes this one is measured from.
    fn referents(&self) -> std::option::IntoIter<FeatureId> {
        match self {
            Datum::Ground => None.into_iter(),
        }
    }
}
