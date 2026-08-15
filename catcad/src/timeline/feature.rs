//! One step of a timeline, and the kinds there are.

use silverpoint::Sketch;

use crate::profile::Profile;
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
    /// A solid grown off a region of a sketch, that far along the plane the
    /// sketch is drawn on.
    ///
    /// Holds the region by name and not the plane it lies in, for the reason the
    /// sketch above holds neither: where an extrude lands follows from the plane
    /// its profile's sketch names, worked out afresh every time. So a plane that
    /// moves carries the solids grown off it along, and there is no second copy
    /// of where they are for the move to leave stale.
    ///
    /// Signed, which is what makes which way it grows the one number rather than
    /// a second field: a negative distance is the same extrude on the other side
    /// of the plane, and that is what a modeller offers as a flip.
    Extrude { profile: Profile, distance: f64 },
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
            Feature::Extrude { profile, distance } => Feature::Extrude {
                profile: profile.clone(),
                distance: *distance,
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
            (
                Feature::Extrude { profile, distance },
                Feature::Extrude {
                    profile: from,
                    distance: to,
                },
            ) => {
                profile.clone_from(from);
                *distance = *to;
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
            // The sketch the region is of, and nothing else. The plane is not a
            // referent of this step but of that one — see [`Feature::Extrude`],
            // on why an extrude names no plane of its own.
            Feature::Extrude { profile, .. } => Some(profile.sketch()).into_iter(),
        }
    }

    /// What to call this where a mistaken caller will read it.
    ///
    /// Which kind it *is*, rather than which kind was wanted: a caller that
    /// asked a sketch for its frame and one that asked a plane for its geometry
    /// have made opposite mistakes, and the half neither can work out for itself
    /// is what it actually named. See
    /// [`wrong_kind`](crate::timeline::wrong_kind), which is the only caller and
    /// the only reason this is a noun phrase rather than a word.
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Feature::Plane(_) => "a plane",
            Feature::Sketch { .. } => "a sketch",
            Feature::Extrude { .. } => "an extrude",
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
