//! What a settle leaves behind: how the run went, and what the sketch it
//! settled on can still be asked to do.

use crate::sketch::solver::freedoms::{Freedom, Freedoms};
use crate::sketch::{CircleId, ConstraintId, PointId};

/// Everything a settle leaves behind: how the run that got here went, how much
/// the sketch's constraints still leave undecided, and whose freedom that is.
///
/// Filled rather than returned, so a drawing settling every frame of a drag
/// keeps one of these instead of being handed a new one.
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
    pub(super) freedoms: Freedoms,
    pub(super) converged: bool,
    pub(super) iterations: u32,
    pub(super) degrees_of_freedom: usize,
    pub(super) redundant_equations: usize,
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

    /// Equations beyond the rank of the system. On a satisfied sketch these are
    /// consistent duplicates; on an unsatisfiable one they are the conflict.
    ///
    /// The total [`Outcome::is_redundant`] breaks down, and the two can differ:
    /// this counts equations, that flags constraints, and a coincidence is worth
    /// two equations.
    pub fn redundant_equations(&self) -> usize {
        self.redundant_equations
    }

    /// What the constraints leave of a point.
    pub fn point(&self, id: PointId) -> Freedom {
        self.freedoms.point(id)
    }

    /// What the constraints leave of a circle's radius — not of the circle,
    /// which also moves with its centre.
    pub fn radius(&self, id: CircleId) -> Freedom {
        self.freedoms.radius(id)
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
        self.freedoms.is_redundant(id)
    }
}
