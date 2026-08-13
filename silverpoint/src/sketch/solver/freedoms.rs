//! What a sketch's constraints leave undecided, entity by entity.

use crate::sketch::{CircleId, PointId, Sketch};

/// Stale answers are a caller mistake rather than bad data: the sketch that was
/// measured is the sketch that has to be asked.
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

/// What a sketch's constraints leave undecided, entity by entity — the
/// breakdown behind [`SolveReport::degrees_of_freedom`], which counts the same
/// freedoms without saying whose they are.
///
/// Taken by [`Solver::freedoms`], which fills one rather than returning one so
/// that a drawing measured every frame keeps its own instead of being handed a
/// new one.
///
/// [`SolveReport::degrees_of_freedom`]: crate::SolveReport::degrees_of_freedom
/// [`Solver::freedoms`]: crate::Solver::freedoms
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Freedoms {
    /// By point slot, so a handle indexes straight in.
    points: Vec<Freedom>,
    /// By circle slot. Never [`Freedom::Partly`] — a radius is one parameter,
    /// and one parameter is either decided or not.
    radii: Vec<Freedom>,
    degrees_of_freedom: usize,
    redundant_equations: usize,
}

impl Freedoms {
    /// Free parameters the constraints leave undetermined. Zero is a fully
    /// constrained sketch; higher means it can still be dragged, by exactly
    /// this many independent motions.
    ///
    /// The total the labels below break down, and here rather than on a solve's
    /// report because it is the same measurement read at a coarser resolution.
    /// One rank decides both, so the count and the labels cannot disagree about
    /// what the sketch can do.
    pub fn degrees_of_freedom(&self) -> usize {
        self.degrees_of_freedom
    }

    /// Equations beyond the rank of the system. On a satisfied sketch these are
    /// consistent duplicates; on an unsatisfiable one they are the conflict.
    pub fn redundant_equations(&self) -> usize {
        self.redundant_equations
    }

    /// What the constraints leave of a point.
    pub fn point(&self, id: PointId) -> Freedom {
        *self.points.get(id.slot()).expect(UNMEASURED)
    }

    /// What the constraints leave of a circle's radius — not of the circle,
    /// which also moves with its centre.
    pub fn radius(&self, id: CircleId) -> Freedom {
        *self.radii.get(id.slot()).expect(UNMEASURED)
    }

    /// Size to `sketch` and start everything determined, ready to be told
    /// otherwise. Keeps whatever room it has grown to.
    pub(crate) fn reset(
        &mut self,
        sketch: &Sketch,
        degrees_of_freedom: usize,
        redundant_equations: usize,
    ) {
        self.degrees_of_freedom = degrees_of_freedom;
        self.redundant_equations = redundant_equations;
        self.points.clear();
        self.points
            .resize(sketch.point_slot_count(), Freedom::Determined);
        self.radii.clear();
        self.radii
            .resize(sketch.circle_slot_count(), Freedom::Determined);
    }

    pub(crate) fn set_point(&mut self, id: PointId, freedom: Freedom) {
        self.points[id.slot()] = freedom;
    }

    pub(crate) fn set_radius(&mut self, id: CircleId, freedom: Freedom) {
        self.radii[id.slot()] = freedom;
    }
}
