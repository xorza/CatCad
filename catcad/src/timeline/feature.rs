//! One step of a timeline, and the kinds there are.

use silverpoint::{Operation, Plane, Sketch};

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
    ///
    /// **What it does with the solid standing before it is a field and not a
    /// third kind of step.** A cut and a boss differ in one word and share a
    /// profile, a distance, a drag handle, a form and a file record — see
    /// `.notes/KERNEL.md` §8 — so the word is what varies and everything else
    /// is written once. The first extrude of a document has nothing standing
    /// before it, which makes a join the whole of itself and the other two
    /// nothing at all.
    Extrude {
        profile: Profile,
        distance: f64,
        operation: Operation,
    },
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
            Feature::Extrude {
                profile,
                distance,
                operation,
            } => Feature::Extrude {
                profile: profile.clone(),
                distance: *distance,
                operation: *operation,
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
                Feature::Extrude {
                    profile,
                    distance,
                    operation,
                },
                Feature::Extrude {
                    profile: from,
                    distance: to,
                    operation: doing,
                },
            ) => {
                profile.clone_from(from);
                *distance = *to;
                *operation = *doing;
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
            // *Which* plane, because a world plane is not somewhere anybody
            // put one and the slip about it is its own: a caller asking one to
            // move has mistaken a plane for the world, and "a plane rather than
            // a plane" would tell them nothing.
            Feature::Plane(Datum::World(_)) => "a world plane",
            Feature::Plane(Datum::Offset { .. }) => "a plane",
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
    /// One of the three the world comes with, which depends on nothing and so
    /// is where every chain of planes ends.
    World(World),
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
            Datum::World(_) => None.into_iter(),
            Datum::Offset { from, .. } => Some(*from).into_iter(),
        }
    }
}

/// Which of the three planes the world comes with.
///
/// One arm of [`Datum`] rather than three, because nothing reading a datum
/// tells them apart: what a plane is measured off, whether it can be moved and
/// what may be built on it are the same answers for all three. What differs is
/// the frame each stands for, and that is the whole of what this holds.
///
/// Here rather than in silverpoint, which holds the frames themselves. Which
/// three a modeller offers and what they are called is this crate's business; a
/// plane there is a frame and knows nothing about being referred to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum World {
    /// The horizontal one, faced from above.
    Ground,
    /// The upright one across the model, faced from the front.
    Front,
    /// The upright one along it, faced from the right.
    Side,
}

impl World {
    /// The frame it stands for.
    pub(crate) fn plane(self) -> Plane {
        match self {
            World::Ground => Plane::GROUND,
            World::Front => Plane::FRONT,
            World::Side => Plane::SIDE,
        }
    }

    /// What to call it where a person reads it.
    ///
    /// **The pair of world axes it spans**, which is what every modeller labels
    /// these three by. A role — "Ground", "Front" — reads as a claim about which
    /// way up the model is, and nothing here decides that: a part is as often
    /// modelled lying down as standing, so the plane called the front would be
    /// the top of half the documents that used it. The axes are true whatever
    /// the part turns out to be.
    ///
    /// Two letters and never three, which is why they are worth the room: at a
    /// square this size the name is longer than the thing it names, so a word
    /// would be a label with a square attached rather than the other way about.
    pub(crate) fn named(self) -> &'static str {
        match self {
            World::Ground => "XZ",
            World::Front => "XY",
            World::Side => "YZ",
        }
    }
}
