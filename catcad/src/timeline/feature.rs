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
#[derive(Debug, PartialEq)]
pub(crate) enum Feature {
    /// A flat frame to draw on.
    Plane(Datum),
    /// A sketch, and the plane it is drawn on.
    Sketch { on: FeatureId, sketch: Sketch },
}

// Written out for `clone_from`, which `derive(Clone)` leaves at the trait's
// default — `*self = source.clone()`, a fresh sketch every call. The history
// rewrites one step's far end sixty times a second for as long as a drag lasts,
// and the arenas a sketch holds are what make that worth not re-allocating.
impl Clone for Feature {
    fn clone(&self) -> Self {
        match self {
            Feature::Plane(datum) => Feature::Plane(*datum),
            Feature::Sketch { on, sketch } => Feature::Sketch {
                on: *on,
                sketch: sketch.clone(),
            },
        }
    }

    fn clone_from(&mut self, source: &Self) {
        match (self, source) {
            (
                Feature::Sketch { on, sketch },
                Feature::Sketch {
                    on: from,
                    sketch: source,
                },
            ) => {
                *on = *from;
                sketch.clone_from(source);
            }
            // A plane is a handful of numbers, and two steps of different kinds
            // share nothing there would be any point writing over.
            (this, source) => *this = source.clone(),
        }
    }
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
}

/// Where a plane gets its frame from.
///
/// Two kinds, and deliberately no more. Anything richer — a plane free to sit
/// anywhere, at any angle — is the same shape with six numbers instead of one,
/// and the gesture that edits it is a gizmo rather than a drag; a plane on the
/// face of a solid waits for there to be solids to put one on.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Datum {
    /// The world's own ground, which depends on nothing and so is where every
    /// chain of planes ends.
    Ground,
    /// Parallel to another, this far along its normal.
    ///
    /// One number, and moving a plane is retyping it. What that costs is the
    /// whole reason a sketch names its plane rather than carrying one: the
    /// sketches hanging off this land somewhere else afterwards and say exactly
    /// what they said before.
    Offset { from: FeatureId, by: f64 },
}

impl Datum {
    /// The planes this one is measured from.
    fn referents(&self) -> std::option::IntoIter<FeatureId> {
        match self {
            Datum::Ground => None.into_iter(),
            Datum::Offset { from, .. } => Some(*from).into_iter(),
        }
    }
}
