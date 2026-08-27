//! Which of two bodies' material a boolean keeps.

use crate::number::predicate;
use crate::solid::boolean::sounding::Standing;
use glam::DVec3;

/// What a boolean does with the two bodies it is given.
///
/// A field on the feature that names it rather than three features, because a
/// cut and a boss differ in one word and share a profile, a distance, a drag
/// handle, a form and a file record — see `.notes/KERNEL.md` §8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operation {
    /// Both, as one body.
    Join,
    /// The first, less the second.
    Cut,
    /// Only what both hold.
    Intersect,
}

impl Operation {
    /// Whether a region of the body at `first`, facing `facing` and standing
    /// where `standing` says, is kept.
    ///
    /// The whole of what tells the three apart, and it is a table rather than
    /// three routines because that is what it is: every stage before this one
    /// is the same work whichever operation asked for it.
    pub(super) fn keeps(self, standing: Standing, facing: DVec3, first: bool) -> bool {
        match (self, standing, first) {
            // What is outside the other body is the outside of a join, and
            // what is inside it is the inside of an intersection.
            (Self::Join, Standing::Outside, _) => true,
            (Self::Intersect, Standing::Inside, _) => true,
            // A cut keeps the first body's outside and the second's inside —
            // the second turned over, because the wall of a pocket faces the
            // way the tool's own wall faced away from.
            (Self::Cut, Standing::Outside, true) => true,
            (Self::Cut, Standing::Inside, false) => true,
            // **Flush against the other body.** The two faces pressed together
            // describe one piece of surface, so at most one of them survives —
            // and it is the first body's, always: keeping both would leave the
            // answer a doubled skin, and choosing between two copies of the
            // same surface is a choice without a difference.
            (_, Standing::On(_), false) => false,
            // Whether that one piece bounds anything is what is left. Held
            // against each other with the material on the same side, a join and
            // an intersection both still have material there and none opposite,
            // so the surface stands; a cut takes that material away and leaves
            // nothing for it to bound. Held back to back it is the other way
            // round — the join buries the surface in material and the
            // intersection in empty space, while the cut leaves the first
            // body's own face standing where it always was.
            (Self::Join | Self::Intersect, Standing::On(theirs), true) => agree(theirs, facing),
            (Self::Cut, Standing::On(theirs), true) => !agree(theirs, facing),
            // Inside for a join, outside for an intersection, and the halves
            // of a cut that belong to the other operand.
            (Self::Join, Standing::Inside, _)
            | (Self::Intersect, Standing::Outside, _)
            | (Self::Cut, Standing::Inside, true)
            | (Self::Cut, Standing::Outside, false) => false,
        }
    }

    /// Whether a kept region of the body at `first` faces the other way round
    /// in the answer than it did in the body it came from.
    pub(super) fn turns(self, first: bool) -> bool {
        matches!(self, Self::Cut) && !first
    }
}

/// Whether two faces pressed against each other hold their material on the same
/// side of the surface they share.
///
/// A sign test rather than a comparison against a tolerance, which is sound
/// only because the two are coplanar: a region whose interior touched a plane
/// of the other body would have been cut *by* that plane, and would have no
/// interior left on it to sound. So the two directions are parallel and the dot
/// product is ±1 — which is the case [`predicate::parallel`] tells a caller to
/// take the dot product for itself, and the assert is what says the reasoning
/// still holds.
fn agree(theirs: DVec3, facing: DVec3) -> bool {
    debug_assert!(
        predicate::parallel(theirs, facing),
        "{theirs:?} and {facing:?} are flush against each other and not parallel",
    );
    theirs.dot(facing) > 0.0
}
