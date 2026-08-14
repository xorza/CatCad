//! Satisfying a sketch's constraints, and reporting what that left.
//!
//! [`Solver`] is the whole of the way in. It holds the sketch's current
//! assembly and drives two things over it that never run at once: the
//! Levenberg-Marquardt run next door in [`Stepper`], which moves geometry, and
//! the reduction in [`Elimination`], which asks the sketch at rest what its
//! constraints still leave it free to do.

use crate::sketch::snapshot::Snapshot;
use crate::sketch::solver::elimination::Elimination;
use crate::sketch::solver::outcome::Outcome;
use crate::sketch::solver::stepper::Stepper;
use crate::sketch::solver::system::System;
use crate::sketch::{PointId, Sketch};

/// Converged once every residual is within this of zero.
///
/// Residuals are in sketch units (lengths) or their squares (angles), so this is
/// an absolute tolerance on the geometry, not a relative one.
///
/// Fixed rather than a knob a caller turns, because [`UNMOVED`] below and
/// `STALLED` next door are both stated as margins against it — decades of room,
/// measured once. A caller free to raise this would be free to invalidate the
/// pair of them, and neither has anything to say when it happened.
const TOLERANCE: f64 = 1e-10;

/// How far a parameter may differ and still count as not having moved, in
/// sketch units.
///
/// Not [`TOLERANCE`], which bounds *residuals*. A converged solve leaves
/// the geometry satisfying its constraints rather than sitting on any particular
/// point of the set that does, so free geometry drifts a little under the
/// arithmetic — measured on the demo's sketch, about a nanometre, four decades
/// looser than the residual bound that produced it.
///
/// Three decades above that drift, and far below the smallest drag anyone could
/// mean: a pointer moving one pixel across a drawing on screen covers
/// hundredths of a unit, never millionths.
const UNMOVED: f64 = 1e-6;

/// Solves a [`Sketch`] in place.
///
/// Holds the sketch's assembly and the room the two phases work in, so one kept
/// alive across a drag pays for them once rather than once a frame. A throwaway
/// `Solver::default().solve(..)` still works and still allocates — the room is
/// only saved by keeping the solver.
#[derive(Debug, Default)]
pub struct Solver {
    /// The sketch as it currently stands, and what it was allowed to move
    /// getting there. The one thing both phases work on: the run steps it, and
    /// everything that describes the sketch afterwards reads it.
    system: System,
    /// The sketch as it stood before the edit being attempted, assembled, and
    /// set aside for as long as the attempt lasts.
    ///
    /// A refused edit leaves the sketch exactly as it was found, so what it has
    /// to report is what this already holds — and swapping it out of the way is
    /// what keeps the attempt from overwriting it. Two pointer swaps against the
    /// assembly they save, which is every constraint's residual and its
    /// derivatives evaluated afresh over geometry that never moved.
    spare: System,
    stepper: Stepper,
    elimination: Elimination,
    /// The sketch as it stood before the edit being attempted, so one the
    /// constraints cannot take can be put back whole.
    before: Snapshot,
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
        self.describe(sketch, into, iterations);
    }

    /// Step the sketch's geometry with `held` pinned, and answer how many steps
    /// were kept. Leaves [`Solver::system`] assembled for `held`, which is not
    /// what anything describing the sketch afterwards wants — see
    /// [`Solver::assemble_at_rest`].
    fn iterate(&mut self, sketch: &mut Sketch, held: &[PointId]) -> u32 {
        self.stepper.iterate(sketch, &mut self.system, held)
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
    /// as it can.
    ///
    /// The second attempt is a free solve from where the cursor asked, so it may
    /// in principle settle on a different branch of a mechanism that admits more
    /// than one. It runs only where the first was refused, which is where
    /// nothing moved at all.
    ///
    /// `edit` may move geometry. It may not add or remove any: `held` and the
    /// residual this is judged against were both taken of the sketch as it
    /// arrived. Adding geometry is [`Solver::solve`]'s, with nothing held.
    ///
    /// It may also be run twice — once for each attempt — and both runs are
    /// given the sketch exactly as it arrived, so one that reads the sketch and
    /// moves geometry *relative* to what it finds would compound. Say where
    /// geometry goes, not how far it moves: a drag knows where the cursor is.
    pub fn edit_holding(
        &mut self,
        sketch: &mut Sketch,
        held: &[PointId],
        into: &mut Outcome,
        edit: impl Fn(&mut Sketch),
    ) {
        // Built straight into the one buffer the attempt ahead will not touch,
        // because a refusal has to describe exactly this sketch and this is
        // already its assembly. Only the residual is wanted now — nothing is
        // being reported about the sketch as it stands, only judged against what
        // the edit leaves — but keeping the rest costs nothing and saves
        // building it again over geometry that never moved.
        self.spare.assemble_holding(sketch, &[]);
        let was = self.spare.max_residual();
        sketch.snapshot_into(&mut self.before);

        edit(sketch);
        debug_assert!(
            self.before.fits(sketch),
            "an edit may move a sketch's geometry, not add to or remove from it"
        );

        // Held first, which is what makes an ordinary drag track the pointer
        // exactly: the grabbed point does not move, and the rest of the sketch
        // swings to accommodate it.
        let iterations = self.iterate(sketch, held);
        if self.take(sketch, was, into, iterations) {
            return;
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
            // Put back and asked for again, rather than kept: what the edit
            // wanted is the edit run on the sketch it was run on, and both of
            // those are already here. Keeping it instead would be a second whole
            // sketch copied on every frame of every drag, to spare a call that
            // is usually one coordinate written.
            sketch.restore(&self.before);
            edit(sketch);
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
            if !self.before.within(sketch, UNMOVED) && self.take(sketch, was, into, iterations) {
                return;
            }
        }

        sketch.restore(&self.before);
        self.describe_as_found(sketch, into);
    }

    /// Which of the sketch's geometry its constraints leave anything to decide,
    /// and which they pin down completely.
    ///
    /// The breakdown behind
    /// [`Outcome::degrees_of_freedom`](crate::Outcome::degrees_of_freedom):
    /// that counts the freedoms a sketch has left and this says whose they are,
    /// which is what lets a drawing show the difference rather than only total
    /// it.
    ///
    /// Fills `into` rather than returning it, so a drawing measuring itself
    /// after every edit keeps one buffer instead of being handed a new one.
    pub fn measure(&mut self, sketch: &Sketch, into: &mut Outcome) {
        self.describe(sketch, into, 0);
    }

    /// Assemble the sketch as it stands with nothing held.
    ///
    /// The state every question about the sketch itself, rather than about a
    /// drag on it, has to be asked of — so [`Solver::take`] both judges what
    /// this leaves and describes it, off the one assembly.
    ///
    /// Always at rest, whatever a run was holding. Determinacy is a property of
    /// the sketch and not of the drag being attempted on it, and a count taken
    /// with a point held would say the sketch had less freedom than it does for
    /// as long as someone was holding it.
    fn assemble_at_rest(&mut self, sketch: &Sketch) {
        self.system.assemble_holding(sketch, &[]);
    }

    /// Assemble the sketch at rest and write the whole of what it says about
    /// itself into `into`.
    ///
    /// The two halves are only ever right together — what
    /// [`Solver::read_at_rest`] reduces is whichever assembly the solver happens
    /// to be holding, and this is what puts the sketch's own there. Every way of
    /// describing a sketch is one of the three calls that pair them: this,
    /// [`Solver::take`] and [`Solver::describe_as_found`].
    fn describe(&mut self, sketch: &Sketch, into: &mut Outcome, iterations: u32) {
        self.assemble_at_rest(sketch);
        self.read_at_rest(sketch, into, iterations);
    }

    /// Assemble the sketch at rest, describe it if it is one to keep, and answer
    /// whether it was.
    ///
    /// Kept when it is satisfied outright, or at least no less satisfied than it
    /// was at `was`. Judged on the residual rather than on convergence alone, so
    /// a sketch whose constraints already conflict can still be edited: what is
    /// refused is a step that leaves it *less* satisfied than it was, not one
    /// that merely fails to finish the job.
    ///
    /// Judged and described off the one assembly, and never described unless it
    /// is kept: working out what a refused attempt could still do would be
    /// working it out for a sketch about to be put back.
    fn take(&mut self, sketch: &Sketch, was: f64, into: &mut Outcome, iterations: u32) -> bool {
        self.assemble_at_rest(sketch);
        let residual = self.system.max_residual();
        if residual > TOLERANCE && residual > was {
            return false;
        }
        self.read_at_rest(sketch, into, iterations);
        true
    }

    /// Describe the sketch as it was found, off the assembly set aside before
    /// the edit rather than one built again.
    ///
    /// Taken back rather than rebuilt: [`Solver::spare`] holds the assembly of
    /// the sketch as it arrived, and a refusal restores that sketch to the bit —
    /// so reducing the one is reducing the other. The sketch must be back where
    /// it started, which on the one path that calls this it is.
    ///
    /// What comes out is a report of a sketch standing at its own solution,
    /// because that is what the sketch now is — a caller telling a refusal from
    /// an edit that had nothing to do compares the sketch, which is the only
    /// thing the two differ in.
    fn describe_as_found(&mut self, sketch: &Sketch, into: &mut Outcome) {
        std::mem::swap(&mut self.system, &mut self.spare);
        self.read_at_rest(sketch, into, 0);
    }

    /// Read the whole of what the assembly in hand says the sketch can do.
    ///
    /// Reduces whichever assembly the solver is holding rather than building
    /// one, so a caller that has already judged the residual pays for the
    /// reduction alone — and so this is never called on its own. Getting the
    /// right assembly there is [`Solver::describe`], [`Solver::take`] and
    /// [`Solver::describe_as_found`]'s, one of which every description goes
    /// through; the assert below is the half of their promise that is cheap
    /// enough to check.
    fn read_at_rest(&mut self, sketch: &Sketch, into: &mut Outcome, iterations: u32) {
        debug_assert_eq!(
            self.system.width(),
            sketch.params().count(),
            "the assembly being described is of another sketch"
        );
        self.elimination.measure(sketch, &self.system, into);
        into.converged = self.system.max_residual() <= TOLERANCE;
        into.iterations = iterations;
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
    use crate::sketch::solver::outcome::Outcome;
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
            let mut outcome = Outcome::default();
            self.elimination.measure(sketch, &self.system, &mut outcome);
            outcome.degrees_of_freedom()
        }
    }
}

#[cfg(test)]
mod tests;
