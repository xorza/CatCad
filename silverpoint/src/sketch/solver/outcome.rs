//! What a settle leaves behind, and which ending it had.

use crate::sketch::solver::SolveReport;
use crate::sketch::solver::freedoms::Freedoms;

/// Which ending a settle had.
///
/// The one thing [`SolveReport`] cannot say. A refused edit leaves the sketch
/// exactly as it was found, and a sketch as it was found is a *satisfied* one —
/// so a refusal reports `converged` in nought iterations, and an edit that was
/// taken but had nothing to solve reports exactly the same. Reading the two
/// apart is what this is for.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Settled {
    /// Measured where it stood: nothing was asked of the constraints, and
    /// nothing moved. What [`Solver::measure`](crate::Solver::measure) answers,
    /// and what an [`Outcome`] nothing has been written into yet reads as.
    #[default]
    AtRest,
    /// Settled with nothing pinned, so the sketch is wherever the constraints
    /// took it. What [`Solver::solve`](crate::Solver::solve) answers, and what
    /// an edit falls back to when holding proves impossible.
    Freely,
    /// Settled with the edit's geometry pinned exactly where it was put, the
    /// rest of the sketch moved to accommodate it. What an ordinary drag
    /// answers.
    Holding,
    /// The constraints could take neither attempt, so the sketch is exactly as
    /// it was found and nothing was done.
    Refused,
}

/// Everything a settle leaves behind: which ending it had, what the sketch's
/// constraints leave undecided, and how the run that got there went.
///
/// Filled rather than returned, for the reason [`Freedoms`] alone was: a
/// drawing settling every frame of a drag keeps one of these instead of being
/// handed a new one.
///
/// The three together rather than separately, because they describe one moment.
/// A caller holding the report of one settle beside the freedoms of another
/// would be painting a drawing in the colours of where it used to be, and there
/// is no way to take one without the other from here.
#[derive(Debug, Default, PartialEq)]
pub struct Outcome {
    pub(super) freedoms: Freedoms,
    pub(super) report: SolveReport,
    pub(super) settled: Settled,
}

impl Outcome {
    /// Which ending the settle had.
    pub fn settled(&self) -> Settled {
        self.settled
    }

    /// What the constraints leave undecided, entity by entity.
    pub fn freedoms(&self) -> &Freedoms {
        &self.freedoms
    }

    /// How the run that got here went.
    pub fn report(&self) -> SolveReport {
        self.report
    }
}
