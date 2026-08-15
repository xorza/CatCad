//! One thing in the drawing that can be pointed at.

use silverpoint::Entity;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    /// A point, edge, rim or relation, named by the handle the sketch keeps for
    /// it — which survives the drawing being laid out again.
    Entity(Entity),
    /// A region the drawing's curves enclose, named by where it falls in the
    /// order they are walked.
    ///
    /// A position rather than a handle, because a face has none. It holds while
    /// the drawing's *topology* does — see
    /// [`Arrangement::faces`](silverpoint::Arrangement) — so a drag that moves
    /// geometry leaves a face where it was in the list, and something that
    /// changes what crosses what may not.
    Face(usize),
}

impl Part {
    /// The sketch entity this names, or `None` where it names a face.
    ///
    /// What everything the sketch itself answers goes through: constraining,
    /// deleting and building all want a handle, and a face has none to give.
    pub(crate) fn entity(self) -> Option<Entity> {
        match self {
            Part::Entity(entity) => Some(entity),
            Part::Face(_) => None,
        }
    }
}

impl From<Entity> for Part {
    fn from(entity: Entity) -> Self {
        Part::Entity(entity)
    }
}
