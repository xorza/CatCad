//! What a settle leaves behind: how the run went, and what the sketch it
//! settled on can still be asked to do.

use crate::sketch::solver::freedom::Freedom;
use crate::sketch::{CircleId, ConstraintId, PointId, SegmentId, Sketch};

/// A slot past what was measured — the sketch having grown since — which is the
/// whole of what the lookups below can tell apart.
///
/// A handle whose slot is in range is answered, whether or not it still names
/// what it did when the measurement was taken: a slot is all of a handle that
/// reaches in here. [`Sketch::holds`] is where a caller unsure of one asks.
const UNMEASURED: &str = "this sketch has geometry the freedoms were not taken over";

/// Everything a settle leaves behind: how the run that got here went, how much
/// the sketch's constraints still leave undecided, and whose freedom that is.
///
/// Filled rather than returned, so a drawing settling every frame of a drag
/// keeps one of these instead of being handed a new one. Sized to the sketch and
/// refilled rather than rebuilt, so measuring one every frame allocates nothing.
///
/// One type rather than a report beside a table, because they describe one
/// moment. A caller holding the report of one settle beside the freedoms of
/// another would be painting a drawing in the colours of where it used to be,
/// and there is no way to take one without the other from here.
///
/// Defaults to what an unsolved sketch would read as — nothing converged, in
/// nought iterations, with no geometry to be asked about.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Outcome {
    /// By point slot, so a handle indexes straight in.
    pub(super) points: Vec<Freedom>,
    /// By segment slot: the looser of the edge's two ends, since an edge is only
    /// as settled as the end that can still travel.
    pub(super) segments: Vec<Freedom>,
    /// By circle slot: the looser of the centre and the radius, since a circle
    /// can move with one or grow on the other.
    pub(super) circles: Vec<Freedom>,
    /// By constraint slot, like the three above. A flag rather than a list, so a
    /// caller walking the constraints to draw them asks about each in turn
    /// rather than searching — which is the way round every reader wants it, and
    /// which says of a constraint worth two equations that it is redundant
    /// rather than saying so twice.
    pub(super) redundant: Vec<bool>,
    pub(super) converged: bool,
    pub(super) iterations: u32,
    pub(super) degrees_of_freedom: usize,
}

impl Outcome {
    /// Every residual landed within the solver's tolerance.
    ///
    /// Says nothing about whether the run *did* anything: an edit the
    /// constraints refused leaves the sketch exactly as it was found, and a
    /// sketch as it was found is a satisfied one — so a refusal reads the same
    /// as an edit that was taken and had nothing to solve. A caller that needs
    /// the two apart compares the sketch, which is the only thing that differs.
    pub fn converged(&self) -> bool {
        self.converged
    }

    /// How many steps the run kept.
    pub fn iterations(&self) -> u32 {
        self.iterations
    }

    /// Free parameters the constraints leave undetermined. Zero is a fully
    /// constrained sketch; higher means it can still be dragged, by exactly this
    /// many independent motions.
    ///
    /// The total the per-entity answers below break down. One rank decides both,
    /// so the count and the labels cannot disagree about what the sketch can do.
    pub fn degrees_of_freedom(&self) -> usize {
        self.degrees_of_freedom
    }

    /// How many constraints the system could do without where the sketch
    /// stands. On a satisfied sketch these are consistent duplicates; on an
    /// unsatisfiable one they are the conflict.
    ///
    /// Exactly the total of what [`Outcome::is_redundant`] flags, counted off
    /// those flags — so a drawing that lights a mark per flagged constraint
    /// lights this many. Not the rank deficiency of the system, which can be
    /// larger: a coincidence is worth two equations, and both of a duplicated
    /// one's rows dying names the one constraint once.
    pub fn redundant_constraints(&self) -> usize {
        self.redundant.iter().filter(|&&flagged| flagged).count()
    }

    /// What the constraints leave of a point.
    pub fn point(&self, id: PointId) -> Freedom {
        *self.points.get(id.slot()).expect(UNMEASURED)
    }

    /// What the constraints leave of an edge, which is the looser of its two
    /// ends: one end free to travel is an edge free to travel with it.
    pub fn segment(&self, id: SegmentId) -> Freedom {
        *self.segments.get(id.slot()).expect(UNMEASURED)
    }

    /// What the constraints leave of a circle, which is the looser of its centre
    /// and its radius — it can move with the one or grow on the other.
    pub fn circle(&self, id: CircleId) -> Freedom {
        *self.circles.get(id.slot()).expect(UNMEASURED)
    }

    /// Whether this constraint is one the system could do without where the
    /// sketch stands.
    ///
    /// What a drawing paints an over-constrained sketch by, and what an editor
    /// offers to delete. Redundant is not the same as wrong: on a satisfied
    /// sketch these are consistent duplicates, saying again what is already
    /// true; on an unsatisfiable one they are where the conflict shows.
    ///
    /// **Which** of a dependent group is flagged is not a fact about the sketch.
    /// The elimination takes the largest coefficient as its pivot, so of two
    /// constraints saying one thing the one flagged is whichever it did not
    /// choose — and moving the geometry can move the flag between them. What is
    /// stable is that the group is over-determined and that one of them is named
    /// for it; a caller telling a user *this constraint is the problem* would be
    /// claiming more than this knows.
    pub fn is_redundant(&self, id: ConstraintId) -> bool {
        *self.redundant.get(id.slot()).expect(UNMEASURED)
    }

    /// Size to `sketch` and start everything determined, ready to be told
    /// otherwise. Keeps whatever room it has grown to.
    ///
    /// By slot rather than by surviving entity, so a handle indexes straight in
    /// and a removal leaves a hole nobody asks about — the same width the
    /// solver's own parameter vector is laid out against.
    pub(super) fn reset(&mut self, sketch: &Sketch) {
        self.points.clear();
        self.points
            .resize(sketch.points.slot_count(), Freedom::Determined);
        self.segments.clear();
        self.segments
            .resize(sketch.segments.slot_count(), Freedom::Determined);
        self.circles.clear();
        self.circles
            .resize(sketch.circles.slot_count(), Freedom::Determined);
        self.redundant.clear();
        self.redundant
            .resize(sketch.constraints.slot_count(), false);
    }
}
