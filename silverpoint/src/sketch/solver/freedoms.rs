//! What a sketch's constraints leave undecided, entity by entity.

use crate::sketch::constraint::ConstraintId;
use crate::sketch::{CircleId, PointId, Sketch};

/// A slot past what was measured — the sketch having grown since — which is the
/// whole of what the lookups below can tell apart.
///
/// A handle whose slot is in range is answered, whether or not it still names
/// what it did when the measurement was taken: a slot is all of a handle that
/// reaches in here. [`Sketch::holds`] is where a caller unsure of one asks.
const UNMEASURED: &str = "this sketch has geometry the freedoms were not taken over";

/// How many independent ways an entity can still move.
///
/// Counted as directions rather than as coordinates, so the answer does not
/// turn on which axes the sketch happens to be drawn against: a point sliding
/// along a diagonal is as constrained as one sliding along `x`, and both read
/// [`Freedom::Partly`].
///
/// Ordered by how much is left, so the weaker of two answers is their maximum
/// — which is what rolls a segment's two ends up into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Freedom {
    /// Nowhere left to go. Its constraints admit exactly one answer, so a drag
    /// on it can only be refused.
    Determined,
    /// On a track: it moves, but every way it can move is the same way. A point
    /// on a line, or one on a circle of stated radius — the second travels in
    /// both coordinates at once and is no freer for it, since a cursor that
    /// leaves the circle is still asking for the impossible.
    ///
    /// About the entity, not the sketch: a rigid arm has three degrees of
    /// freedom between them, and every point of it is [`Freedom::Free`].
    Partly,
    /// Free to be put wherever it is asked for.
    Free,
}

/// One label per entity, by slot.
///
/// The table behind [`Outcome`](crate::Outcome)'s per-entity answers, and no
/// wider than that: it holds whose freedom is whose, where the totals it breaks
/// down are the outcome's own. Sized to the sketch and refilled rather than
/// rebuilt, so a drawing measured every frame allocates nothing.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Freedoms {
    /// By point slot, so a handle indexes straight in.
    points: Vec<Freedom>,
    /// By circle slot. Never [`Freedom::Partly`] — a radius is one parameter,
    /// and one parameter is either decided or not.
    radii: Vec<Freedom>,
    /// By constraint slot, like the two above. A flag rather than a list, so a
    /// caller walking the constraints to draw them asks about each in turn
    /// rather than searching — which is the way round every reader wants it,
    /// and which says of a constraint worth two equations that it is redundant
    /// rather than saying so twice.
    redundant: Vec<bool>,
}

impl Freedoms {
    pub(super) fn point(&self, id: PointId) -> Freedom {
        *self.points.get(id.slot()).expect(UNMEASURED)
    }

    pub(super) fn radius(&self, id: CircleId) -> Freedom {
        *self.radii.get(id.slot()).expect(UNMEASURED)
    }

    pub(super) fn is_redundant(&self, id: ConstraintId) -> bool {
        *self.redundant.get(id.slot()).expect(UNMEASURED)
    }

    /// Size to `sketch` and start everything determined, ready to be told
    /// otherwise. Keeps whatever room it has grown to.
    pub(super) fn reset(&mut self, sketch: &Sketch) {
        self.points.clear();
        self.points
            .resize(sketch.point_slot_count(), Freedom::Determined);
        self.radii.clear();
        self.radii
            .resize(sketch.circle_slot_count(), Freedom::Determined);
        self.redundant.clear();
        self.redundant.resize(sketch.constraint_slot_count(), false);
    }

    pub(super) fn set_point(&mut self, id: PointId, freedom: Freedom) {
        self.points[id.slot()] = freedom;
    }

    pub(super) fn set_radius(&mut self, id: CircleId, freedom: Freedom) {
        self.radii[id.slot()] = freedom;
    }

    /// Flag a constraint the system could do without. Idempotent, because a
    /// constraint worth two equations can be named once per row that died.
    pub(super) fn set_redundant(&mut self, id: ConstraintId) {
        self.redundant[id.slot()] = true;
    }
}
