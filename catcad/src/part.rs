//! One thing in the drawing that can be pointed at.

use silverpoint::Entity;

use crate::timeline::FeatureId;

/// Anything a cursor can land on and a command can act on.
///
/// Wider than [`Entity`] by exactly one case, and that case is the whole reason
/// the type exists: a face is not something the sketch *holds*. It is what the
/// sketch's curves enclose, worked out afresh whenever they move — so unlike a
/// point or an edge there is no handle to name it by, and the two have to be
/// told apart by anything that keeps hold of either.
///
/// Here rather than in silverpoint, because a face is not the sketch's to
/// answer for. Nothing can be constrained to one, nothing can be built on one,
/// and deleting one would mean deleting whatever draws it — so widening
/// [`Entity`] would have widened every match that decides those, each of which
/// would have had to refuse a face by hand.
/// The sketch is named per variant rather than hoisted alongside the enum,
/// though both arms carry one today. What comes next does not: a datum plane is
/// a step of the timeline in its own right and belongs to no sketch, so a
/// common field would have to be made optional the moment one can be picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    /// A point, edge, rim or relation of one sketch, named by the handle that
    /// sketch keeps for it — which survives the drawing being laid out again.
    Entity { sketch: FeatureId, entity: Entity },
    /// A region one sketch's curves enclose, named by where it falls in the
    /// order they are walked.
    ///
    /// A position rather than a handle, because a face has none. It holds while
    /// the drawing's *topology* does — see
    /// [`Arrangement::faces`](silverpoint::Arrangement) — so a drag that moves
    /// geometry leaves a face where it was in the list, and something that
    /// changes what crosses what may not.
    Face { sketch: FeatureId, at: usize },
    /// A datum plane, named by the step that put it there.
    ///
    /// The one part that belongs to no sketch. A plane is a step of the
    /// timeline in its own right — what sketches are drawn *on* rather than
    /// anything drawn — which is why the sketch below is an answer some parts
    /// do not have.
    Plane(FeatureId),
}

impl Part {
    /// Which sketch this belongs to, or `None` where it belongs to none.
    ///
    /// A plane is the one thing here that is not part of a sketch, and the
    /// `None` is what says so: it is what a sketch is drawn on.
    pub(crate) fn sketch(self) -> Option<FeatureId> {
        match self {
            Part::Entity { sketch, .. } | Part::Face { sketch, .. } => Some(sketch),
            Part::Plane(_) => None,
        }
    }

    /// The sketch entity this names, or `None` where it names a face.
    ///
    /// What everything the sketch itself answers goes through: constraining,
    /// deleting and building all want a handle, and a face has none to give.
    pub(crate) fn entity(self) -> Option<Entity> {
        match self {
            Part::Entity { entity, .. } => Some(entity),
            Part::Face { .. } | Part::Plane(_) => None,
        }
    }
}
