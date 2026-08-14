//! Satisfying a sketch's constraints, and reporting what that left.
//!
//! [`Solver`] is the whole of the way in. It holds the sketch's current
//! assembly and drives two things over it that never run at once: the
//! Levenberg-Marquardt run next door in [`Stepper`], which moves geometry, and
//! the reduction in [`Elimination`], which asks the sketch at rest what its
//! constraints still leave it free to do.

use crate::sketch::snapshot::Snapshot;
use crate::sketch::solver::elimination::Elimination;
use crate::sketch::solver::outcome::{Outcome, Settled};
use crate::sketch::solver::stepper::Stepper;
use crate::sketch::solver::system::System;
use crate::sketch::{PointId, Sketch};

/// How far a parameter may differ and still count as not having moved, in
/// sketch units.
///
/// Not [`Solver::tolerance`], which bounds *residuals*. A converged solve leaves
/// the geometry satisfying its constraints rather than sitting on any particular
/// point of the set that does, so free geometry drifts a little under the
/// arithmetic — measured on the demo's sketch, about a nanometre, four decades
/// looser than the residual bound that produced it.
///
/// Three decades above that drift, and far below the smallest drag anyone could
/// mean: a pointer moving one pixel across a drawing on screen covers
/// hundredths of a unit, never millionths.
const UNMOVED: f64 = 1e-6;

/// What a solve achieved.
///
/// How *determined* the answer was is not here: that is a property of the sketch
/// rather than of the run, and it rides beside this in the
/// [`Outcome`](crate::Outcome) every entry point fills — see
/// [`Freedoms::degrees_of_freedom`](crate::Freedoms::degrees_of_freedom).
/// Neither is *which ending* the run had, which is [`Settled`](crate::Settled)'s:
/// a refused edit leaves the sketch exactly as it was found, so it reports
/// converged in nought iterations and reads from here like an edit that was
/// taken and had nothing to do. Splitting the three is what stops them
/// describing different moments, which is what a report carrying a count
/// measured against a *held* system used to do.
///
/// Defaults to what an unsolved sketch would report — nothing converged, in
/// nought iterations — which is what a caller holding a report before it has one
/// to hold should read.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SolveReport {
    /// Every residual landed within the solver's tolerance.
    pub converged: bool,
    pub iterations: u32,
    /// Largest absolute residual left over.
    pub max_residual: f64,
}

/// Solves a [`Sketch`] in place.
///
/// Holds the sketch's assembly and the room the two phases work in, so one kept
/// alive across a drag pays for them once rather than once a frame. A throwaway
/// `Solver::default().solve(..)` still works and still allocates — the room is
/// only saved by keeping the solver.
#[derive(Debug)]
pub struct Solver {
    pub max_iterations: u32,
    /// Converged once every residual is within this of zero. Residuals are in
    /// sketch units (lengths) or their squares (angles), so this is an absolute
    /// tolerance on the geometry, not a relative one.
    pub tolerance: f64,
    /// The sketch as it currently stands, and what it was allowed to move
    /// getting there. The one thing both phases work on: the run steps it, and
    /// everything that describes the sketch afterwards reads it.
    system: System,
    stepper: Stepper,
    elimination: Elimination,
    /// The sketch as it stood before the edit being attempted, so one the
    /// constraints cannot take can be put back whole.
    before: Snapshot,
    /// The sketch as the edit left it, before any solve saw it — what the user
    /// actually asked for. [`Solver::edit_holding`] tries twice, and the second
    /// try has to start from here rather than from where the first gave up.
    asked: Snapshot,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-10,
            system: System::default(),
            stepper: Stepper::default(),
            elimination: Elimination::default(),
            before: Snapshot::default(),
            asked: Snapshot::default(),
        }
    }
}

impl Solver {
    /// Move the sketch's free geometry until its constraints are satisfied.
    ///
    /// The sketch is left at the best position found, converged or not — a
    /// failed solve still leaves it closer than it started, which is what a UI
    /// wants to draw.
    ///
    /// Holds nothing. Pinning geometry for the length of a gesture is
    /// [`Solver::edit_holding`]'s, which also refuses a hold the constraints
    /// cannot take rather than reporting the compromise it settled for.
    pub fn solve(&mut self, sketch: &mut Sketch, into: &mut Outcome) {
        let iterations = self.iterate(sketch, &[]);
        self.assemble_at_rest(sketch);
        self.read_at_rest(sketch, into, Settled::Freely, iterations);
    }

    /// Step the sketch's geometry with `held` pinned, and answer how many steps
    /// were kept. Leaves [`Solver::system`] assembled for `held`, which is not
    /// what anything describing the sketch afterwards wants — see
    /// [`Solver::assemble_at_rest`].
    fn iterate(&mut self, sketch: &mut Sketch, held: &[PointId]) -> u32 {
        self.stepper.iterate(
            sketch,
            &mut self.system,
            held,
            self.max_iterations,
            self.tolerance,
        )
    }

    /// Move the sketch's geometry with `edit`, then settle the rest around it
    /// with `held` pinned — putting the sketch back exactly as it was found if
    /// the constraints cannot take the step.
    ///
    /// What a drag is made of, and one call rather than an edit followed by a
    /// solve because a refused drag has to move *nothing*. Least squares would
    /// otherwise answer an impossible motion with a compromise, and the
    /// compromise is held together only by what the drag pins — so the next
    /// solve, holding something else, lets go of it and the drawing springs
    /// back. Deform under one drag, snap on the next.
    ///
    /// Two attempts. Held, the grabbed point does not move at all and the rest
    /// swings under it, which is what makes an ordinary drag track the pointer
    /// exactly. Where the constraints cannot take that, the same demand would
    /// freeze the drawing — a point tied to an edge is never *exactly* under a
    /// cursor — so the second attempt asks the same edit holding nothing, and
    /// lets the geometry settle as near what was asked for as it may go. A point
    /// on an edge slides along it; an arm the pointer has outrun reaches as far
    /// as it can. [`Outcome::settled`](crate::Outcome) says which happened.
    ///
    /// The second attempt is a free solve from where the cursor asked, so it may
    /// in principle settle on a different branch of a mechanism that admits more
    /// than one. It runs only where the first was refused, which is where
    /// nothing moved at all.
    ///
    /// `edit` may move geometry. It may not add or remove any: `held` and the
    /// residual this is judged against were both taken of the sketch as it
    /// arrived. Adding geometry is [`Solver::solve`]'s, with nothing held.
    pub fn edit_holding(
        &mut self,
        sketch: &mut Sketch,
        held: &[PointId],
        into: &mut Outcome,
        edit: impl FnOnce(&mut Sketch),
    ) {
        // Only the residual, so the pre-edit look costs one assembly and no
        // reduction: nothing is being reported about the sketch as it stands,
        // only judged against what the edit leaves.
        self.assemble_at_rest(sketch);
        let was = self.system.max_residual();
        sketch.snapshot_into(&mut self.before);

        edit(sketch);
        debug_assert!(
            self.before.fits(sketch),
            "an edit may move a sketch's geometry, not add to or remove from it"
        );

        // Only what a second attempt would start from, and only a held edit has
        // one — so an edit holding nothing does not pay for the copy.
        if !held.is_empty() {
            sketch.snapshot_into(&mut self.asked);
        }

        // Held first, which is what makes an ordinary drag track the pointer
        // exactly: the grabbed point does not move, and the rest of the sketch
        // swings to accommodate it.
        //
        // Then assembled at rest, judged, and only then described. The assembly
        // is what both of the calls after it read, which is why it is taken here
        // rather than inside either: an attempt that is not kept is one nothing
        // will be asked about, and working out what the sketch could still do
        // would be working it out for a sketch about to be put back.
        let iterations = self.iterate(sketch, held);
        self.assemble_at_rest(sketch);
        if self.takes(was) {
            return self.read_at_rest(sketch, into, Settled::Holding, iterations);
        }

        // Held, that was impossible — the cursor asked for somewhere the
        // constraints do not reach. So ask again without holding anything, from
        // what the edit asked for rather than from where the first attempt gave
        // up: the geometry is then free to settle back onto what the constraints
        // allow, which lands it as near the cursor as it may go. A point held to
        // an edge slides along it, and an arm outrun by the pointer reaches as
        // far as it can rather than freezing where it was. Holding nothing is
        // what the first attempt already did, so there is no second answer to be
        // had — and asking again would be the same solve run twice for the same
        // refusal.
        if !held.is_empty() {
            sketch.restore(&self.asked);
            let iterations = self.iterate(sketch, &[]);
            // Kept only where it moved the sketch at all. One with nowhere to go
            // answers this attempt by putting everything back where it always
            // had to be — and arriving there a second time by way of a solve is
            // not arriving there in the same *bits*, so a caller comparing
            // snapshots would read a step to take back that nobody took. The
            // whole sketch rather than the held points, because what a drag
            // moves need not be one: driving a radius holds the circle's centre,
            // and the centre staying put says nothing about whether the circle
            // grew.
            if !self.before.within(sketch, UNMOVED) {
                self.assemble_at_rest(sketch);
                if self.takes(was) {
                    return self.read_at_rest(sketch, into, Settled::Freely, iterations);
                }
            }
        }

        sketch.restore(&self.before);
        // Described afresh, because the sketch being described is now the one
        // that was there all along rather than either attempt on it — and
        // [`Settled::Refused`] is the only part of this that a report of a sketch
        // standing at its own solution could not say.
        self.assemble_at_rest(sketch);
        self.read_at_rest(sketch, into, Settled::Refused, 0);
    }

    /// Which of the sketch's geometry its constraints leave anything to decide,
    /// and which they pin down completely.
    ///
    /// The breakdown behind
    /// [`Freedoms::degrees_of_freedom`](crate::Freedoms::degrees_of_freedom):
    /// that counts the freedoms a sketch has left and this says whose they are,
    /// which is what lets a drawing show the difference rather than only total
    /// it.
    ///
    /// Fills `into` rather than returning it, so a drawing measuring itself
    /// after every edit keeps one buffer instead of being handed a new one.
    pub fn measure(&mut self, sketch: &Sketch, into: &mut Outcome) {
        self.assemble_at_rest(sketch);
        self.read_at_rest(sketch, into, Settled::AtRest, 0);
    }

    /// Assemble the sketch as it stands with nothing held.
    ///
    /// The state every question about the sketch itself, rather than about a
    /// drag on it, has to be asked of. [`Solver::takes`] judges the assembly
    /// this leaves and [`Solver::read_at_rest`] describes it, so a caller that
    /// does both pays for one assembly and reduces it once.
    ///
    /// Always at rest, whatever a run was holding. Determinacy is a property of
    /// the sketch and not of the drag being attempted on it, and a count taken
    /// with a point held would say the sketch had less freedom than it does for
    /// as long as someone was holding it.
    fn assemble_at_rest(&mut self, sketch: &Sketch) {
        self.system.assemble_holding(sketch, &[]);
    }

    /// Whether the assembly in hand is one to keep: satisfied outright, or at
    /// least no less satisfied than it was at `was`.
    ///
    /// Reads the system where it stands rather than assembling one, so the
    /// caller says which moment is being judged — and the assembly it answers
    /// about is the same one [`Solver::read_at_rest`] goes on to describe.
    ///
    /// Judged on the residual rather than on convergence alone, so a sketch
    /// whose constraints already conflict can still be edited: what is refused
    /// is a step that leaves it *less* satisfied than it was, not one that
    /// merely fails to finish the job.
    fn takes(&self, was: f64) -> bool {
        let residual = self.system.max_residual();
        residual <= self.tolerance || residual <= was
    }

    /// Read the whole of what the assembly in hand says the sketch can do.
    ///
    /// Carries on from [`Solver::assemble_at_rest`] rather than assembling
    /// again, so a caller that has already judged the residual pays for the
    /// reduction alone. Nothing may have moved the sketch in between: the rows
    /// being reduced are the ones that assembly built.
    fn read_at_rest(
        &mut self,
        sketch: &Sketch,
        into: &mut Outcome,
        settled: Settled,
        iterations: u32,
    ) {
        self.elimination
            .measure(sketch, &self.system, &mut into.freedoms);
        let max_residual = self.system.max_residual();
        into.report = SolveReport {
            converged: max_residual <= self.tolerance,
            iterations,
            max_residual,
        };
        into.settled = settled;
    }
}

mod elimination;
pub(crate) mod freedoms;
pub(crate) mod outcome;
mod stepper;
mod system;

#[cfg(feature = "bench")]
pub(crate) mod bench;

#[cfg(test)]
pub(crate) mod internals {
    use crate::sketch::solver::Solver;
    use crate::sketch::solver::freedoms::Freedoms;
    use crate::sketch::{PointId, Sketch};

    impl Solver {
        /// How many degrees of freedom the sketch has left with `held` pinned.
        ///
        /// Against the system a drag on those points would solve, which nothing
        /// in the API reports: what a caller wants to know is what the *sketch*
        /// can do, not what it could do while someone is holding it.
        ///
        /// What this is for is checking the per-entity labels against the total
        /// they break down. Both come out of the same reduction, but under
        /// different masks, so they are two calculations rather than one asked
        /// twice: how far a point travels along the null space is a Gram
        /// determinant over two of its rows, and what pinning it costs is the
        /// rank of the whole system with those two columns struck out. Where
        /// they agree, each says the other is right.
        pub(super) fn freedom_holding(&mut self, sketch: &Sketch, held: &[PointId]) -> usize {
            self.system.assemble_holding(sketch, held);
            let mut freedoms = Freedoms::default();
            self.elimination
                .measure(sketch, &self.system, &mut freedoms);
            freedoms.degrees_of_freedom()
        }
    }
}

#[cfg(test)]
mod tests;
