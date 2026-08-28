//! One thing in the drawing that can be pointed at.

use silverpoint::{Entity, Grown};

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
/// The sketch is named per variant rather than hoisted alongside the enum, and
/// what that leaves room for is here: [`Part::Step`] is a step of the timeline
/// in its own right and belongs to no sketch at all. A common field would have
/// had to be made optional for it, which is the same thing spelt so that every
/// arm carrying one has to say it does not mean it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Part {
    /// A point, edge, rim or relation of one sketch, named by the handle that
    /// sketch keeps for it — which survives the drawing being laid out again.
    Entity { sketch: FeatureId, entity: Entity },
    /// A region one sketch's curves enclose, named by where it falls in the
    /// order they are walked.
    ///
    /// A position rather than a handle, because a region has none. It holds
    /// while the drawing's *topology* does — see
    /// [`Arrangement::faces`](silverpoint::Arrangement) — so a drag that moves
    /// geometry leaves a region where it was in the list, and something that
    /// changes what crosses what may not.
    ///
    /// A *region* rather than a face, which is what it used to be called. A face
    /// is what a solid has, and once there are solids to point at the two need
    /// telling apart: this is the flat thing a drawing shuts in, and
    /// [`Part::Solid`] is the thing grown off one.
    Region { sketch: FeatureId, at: usize },
    /// One face of a solid, named by the step that grew it and what that face
    /// was grown from.
    ///
    /// A [`Grown`] rather than a position, and the difference from the region
    /// above is the whole of why solids can be built on later: a face of a solid
    /// is named in the same vocabulary the region it came from was, so the name
    /// holds across an edit rather than across a frame. What a *feature* would
    /// keep is this plus the step, which is exactly what is here.
    Solid { of: FeatureId, face: Grown },
    /// A step of the timeline itself, named by the handle that is it.
    ///
    /// **The whole step and not something in it**, which is what makes this the
    /// one part that belongs to no sketch. A plane is what sketches are drawn
    /// *on*; a sketch step is the drawing rather than anything drawn in it; an
    /// extrude is a solid rather than a face of one. Each is a row of the
    /// feature tree, and each is what a delete takes whole.
    ///
    /// One arm for the three kinds rather than one apiece, because nothing that
    /// reads a part needs to tell them apart — what may be dragged, what may be
    /// deleted, what picking one opens are all questions with the timeline's own
    /// answers, and asking it is cheaper than carrying a second copy of what it
    /// already says. See [`Timeline::movable`](crate::timeline::Timeline), which
    /// answers `None` for a step nothing can drag.
    Step(FeatureId),
    /// The depth of a solid still being decided, which is drawn from an open
    /// form rather than from anything the document holds.
    ///
    /// Names no step, because there is none: the arrow that carries it is a
    /// control over a reading — see [`Asking::Extrude`](crate::prompt::Asking) —
    /// and what a drag on it writes is the form's own draft. It goes here all
    /// the same, because a tag names a `Part` and the arrow has to be grabbable.
    Growing,
    /// How much of a turn a solid still being decided sweeps, likewise.
    ///
    /// Its own arm rather than a field on the one above, because the two are
    /// two handles: they stand in different places, travel on different
    /// motions, and write different fields of the form. What they share is
    /// naming no step, and that is the half neither has to say.
    Turning,
}

impl Part {
    /// Which sketch this belongs to, or `None` where it belongs to none.
    ///
    /// Two things here are not part of a sketch, and the `None` is what says so:
    /// a step of the timeline is not *in* a drawing, and a face of a solid was
    /// grown off one rather than drawn in it.
    ///
    /// **A sketch step answers `None` too**, which is not the contradiction it
    /// reads as: this asks what a part belongs to, and a sketch belongs to
    /// nothing. What sketch picking one puts you *in* is a different question,
    /// and one this cannot answer — telling a sketch step from a plane wants the
    /// timeline. See [`Models::opens`](crate::model::Models).
    pub(crate) fn sketch(self) -> Option<FeatureId> {
        match self {
            Part::Entity { sketch, .. } | Part::Region { sketch, .. } => Some(sketch),
            Part::Step(_) | Part::Solid { .. } | Part::Growing | Part::Turning => None,
        }
    }

    /// The sketch entity this names, or `None` where it names anything else.
    ///
    /// What everything the sketch itself answers goes through: constraining,
    /// deleting and building all want a handle, and nothing below the first arm
    /// has one to give.
    pub(crate) fn entity(self) -> Option<Entity> {
        match self {
            Part::Entity { entity, .. } => Some(entity),
            Part::Region { .. }
            | Part::Step(_)
            | Part::Solid { .. }
            | Part::Growing
            | Part::Turning => None,
        }
    }
}
