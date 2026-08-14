//! Levenberg-Marquardt over the constraint residuals.
//!
//! Each iteration assembles the residual vector and its Jacobian, forms the
//! damped normal equations `(JᵀJ + λD) δ = -Jᵀr`, and takes the step if it
//! reduces the residual norm — raising the damping toward gradient descent
//! when it doesn't. Matrices are dense: a sketch has two parameters per point
//! plus one per circle, so the cost of sparsity bookkeeping would exceed what
//! it saves.

use crate::math::dense::{max_abs, norm, solve_in_place};
use crate::sketch::snapshot::Snapshot;
use crate::sketch::solver::outcome::{Outcome, Settled};
use crate::sketch::solver::workspace::Workspace;
use crate::sketch::{PointId, Sketch};

/// Damping starts here and moves by these factors on an accepted or rejected
/// step. Rejections back off harder than acceptances close in, which keeps a
/// bad region from being re-entered repeatedly.
const INITIAL_DAMPING: f64 = 1e-6;
const DAMPING_DECAY: f64 = 0.3;
const DAMPING_GROWTH: f64 = 8.0;

/// Past this the step is numerically zero, so no further damping can help.
const MAX_DAMPING: f64 = 1e12;

/// Below this much of itself, the reduction an accepted step makes in the
/// residual counts as no reduction at all, and the iteration stops.
///
/// Not [`Solver::tolerance`], which asks whether the sketch is *solved*. This
/// asks whether solving it any further is possible. A system the constraints
/// cannot satisfy still has a least-squares answer, and its residual never
/// reaches any tolerance — so the loop's one test can never fire, and without
/// this it grinds on against a minimum it reached long ago until the damping
/// gives out. Measured on the demo's own drawing under a drag it has to refuse:
/// thirty-three steps taken, four of them useful.
///
/// Three decades above the noise an `f64` residual carries, and eleven below
/// the smallest reduction a converging solve was measured to make — a margin
/// wide enough that no sketch is going to fall in it.
const STALLED: f64 = 1e-12;

/// How far a parameter may differ and still count as not having moved, in
/// sketch units.
///
/// Not [`Solver::tolerance`], which bounds *residuals*. A converged solve
/// leaves the geometry satisfying its constraints rather than sitting on any
/// particular point of the set that does, so free geometry drifts a little
/// under the arithmetic — measured on the demo's sketch, about a nanometre,
/// four decades looser than the residual bound that produced it.
///
/// Three decades above that drift, and far below the smallest drag anyone could
/// mean: a pointer moving one pixel across a drawing on screen covers
/// hundredths of a unit, never millionths.
const UNMOVED: f64 = 1e-6;

/// What a solve achieved.
///
/// How *determined* the answer was is not here: that is a property of the
/// sketch rather than of the run, and it rides beside this in the
/// [`Outcome`](crate::Outcome) every entry point fills — see
/// [`Freedoms::degrees_of_freedom`](crate::Freedoms::degrees_of_freedom).
/// Neither is *which ending* the run had, which is
/// [`Settled`](crate::Settled)'s: a refused edit leaves the sketch exactly as
/// it was found, so it reports converged in nought iterations and reads from
/// here like an edit that was taken and had nothing to do. Splitting the three
/// is what stops them describing different moments, which is what a report
/// carrying a count measured against a *held* system used to do.
///
/// Defaults to what an unsolved sketch would report — nothing converged, in
/// nought iterations — which is what a caller holding a report before it has
/// one to hold should read.
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
/// Holds the buffers a solve works in, so one kept alive across a drag pays
/// for them once rather than once a frame. A throwaway
/// `Solver::default().solve(..)` still works and still allocates — the room
/// is only saved by keeping the solver.
#[derive(Debug)]
pub struct Solver {
    pub max_iterations: u32,
    /// Converged once every residual is within this of zero. Residuals are in
    /// sketch units (lengths) or their squares (angles), so this is an
    /// absolute tolerance on the geometry, not a relative one.
    pub tolerance: f64,
    work: Workspace,
    /// The sketch as it stood before the edit being attempted, so one the
    /// constraints cannot take can be put back whole. Outside [`Workspace`]
    /// because it outlives a solve rather than serving one.
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
            work: Workspace::default(),
            before: Snapshot::default(),
            asked: Snapshot::default(),
        }
    }
}

impl Solver {
    /// Move the sketch's free geometry until its constraints are satisfied.
    ///
    /// The sketch is left at the best position found, converged or not — a
    /// failed solve still leaves it closer than it started, which is what a
    /// UI wants to draw.
    pub fn solve(&mut self, sketch: &mut Sketch, into: &mut Outcome) {
        self.solve_holding(sketch, &[], into);
    }

    /// Solve with `held` pinned where they are, whatever their own
    /// [`Sketch::is_fixed`] says.
    ///
    /// The settling half of a drag: the point under the cursor stays where the
    /// cursor put it and the rest of the sketch moves to accommodate it, which
    /// is the difference between dragging a drawing and watching it snap back.
    ///
    /// Held rather than fixed, because [`Sketch::fix`] is the user's statement
    /// about the drawing: a point does not become pinned because someone is
    /// holding it, and anything reading that flag — the marker it is drawn
    /// with, the degrees of freedom reported at rest — would be told it did.
    ///
    /// A sketch with nothing left to give reports `converged: false`, and is
    /// left at the compromise that reached it. Only half an answer, which is
    /// why this is not the crate's way of dragging one: what a caller wants is
    /// [`Solver::edit_holding`], which throws that compromise away.
    pub(crate) fn solve_holding(
        &mut self,
        sketch: &mut Sketch,
        held: &[PointId],
        into: &mut Outcome,
    ) {
        let iterations = self.iterate(sketch, held);
        let settled = if held.is_empty() {
            Settled::Freely
        } else {
            Settled::Holding
        };
        self.measure_taking(sketch, into, settled, iterations);
    }

    /// Take Levenberg-Marquardt steps until the residuals are inside tolerance
    /// or the damping gives out, and answer how many were kept.
    ///
    /// Says nothing about what it left behind. What the sketch amounts to
    /// afterwards is [`Solver::measure_taking`]'s to report, and it asks that
    /// of the geometry rather than of the run — so nothing here has to know
    /// that a `held` point is held only for the length of the drag.
    fn iterate(&mut self, sketch: &mut Sketch, held: &[PointId]) -> u32 {
        let max_iterations = self.max_iterations;
        let tolerance = self.tolerance;
        let n = sketch.params().count();
        let work = &mut self.work;
        work.reset(sketch, held);
        let mut damping = INITIAL_DAMPING;
        let mut iterations = 0;

        work.system.assemble(sketch, &work.movable);
        while iterations < max_iterations && max_abs(&work.system.residuals) > tolerance {
            iterations += 1;
            work.normal.fill(0.0);
            work.step.fill(0.0);
            for (row, residual) in work
                .system
                .jacobian
                .chunks_exact(n)
                .zip(&work.system.residuals)
            {
                for a in 0..n {
                    if row[a] == 0.0 {
                        continue;
                    }
                    work.step[a] -= row[a] * residual;
                    for b in 0..n {
                        work.normal[a * n + b] += row[a] * row[b];
                    }
                }
            }
            // One scalar for the whole matrix, so the damping stays a multiple
            // of the identity. Per-parameter scaling would be better
            // conditioned, but it drags an under-constrained sketch along its
            // own null space: the null direction stops being an eigenvector,
            // and the resulting sideways drift is O(1) in the damping rather
            // than vanishing with it. Uniform damping keeps the step
            // minimum-norm, so free geometry stays where the user left it.
            let curvature = (0..n).fold(1.0f64, |acc, a| acc.max(work.normal[a * n + a]));
            for a in 0..n {
                if work.movable[a] {
                    work.normal[a * n + a] += damping * curvature;
                } else {
                    // A fixed parameter has an all-zero column, which would
                    // make the system singular. A unit diagonal against a zero
                    // gradient yields a zero step for it instead.
                    work.normal[a * n + a] = 1.0;
                }
            }

            if !solve_in_place(&mut work.normal, n, &mut work.step) {
                damping *= DAMPING_GROWTH;
                if damping > MAX_DAMPING {
                    break;
                }
                continue;
            }

            work.trial.clear();
            work.trial
                .extend(work.params.iter().zip(&work.step).map(|(p, d)| p + d));
            sketch.params_mut().set(&work.trial);
            work.trial_system.assemble(sketch, &work.movable);
            let (residual, trial) = (
                norm(&work.system.residuals),
                norm(&work.trial_system.residuals),
            );
            if trial < residual {
                // Swapped rather than assigned: the loser's buffers become the
                // next round's scratch, so nothing is ever rebuilt.
                std::mem::swap(&mut work.params, &mut work.trial);
                std::mem::swap(&mut work.system, &mut work.trial_system);
                damping = (damping * DAMPING_DECAY).max(f64::MIN_POSITIVE);
                // Kept, and the last worth taking: a step that improves on its
                // predecessor by nothing is one whose successors improve on it
                // by nothing either, and the residual test above can only stop a
                // system that has an answer to reach.
                if residual - trial <= STALLED * residual {
                    break;
                }
            } else {
                sketch.params_mut().set(&work.params);
                damping *= DAMPING_GROWTH;
                if damping > MAX_DAMPING {
                    break;
                }
            }
        }
        iterations
    }

    /// Move the sketch's geometry with `edit`, then settle the rest around it
    /// with `held` pinned — putting the sketch back exactly as it was found if
    /// the constraints cannot take the step.
    ///
    /// What a drag is made of, and the reason it is one call rather than an
    /// edit followed by a solve. Dragging geometry the
    /// constraints already determine asks for a motion they forbid, and least
    /// squares answers with a compromise: the drawing deforms under the cursor.
    /// Keeping that would be bad enough on its own, but it is worse than it
    /// looks — the compromise is held together only by what the drag pins, so
    /// the *next* solve, holding something else, lets go of it and the drawing
    /// springs back. Deform under one drag, snap on the next. Undoing the step
    /// whole is what makes a drag the constraints refuse simply not move
    /// anything, which is the truth about it.
    ///
    /// Two attempts, because holding is a demand and not a preference. Held,
    /// the grabbed point does not move at all and the rest of the sketch swings
    /// under it — which is what makes an ordinary drag track the pointer
    /// exactly, and what a caller wants whenever the constraints can take it.
    /// When they cannot, the same demand is what would freeze the drawing: a
    /// point tied to an edge is never *exactly* under a cursor, so held it can
    /// never move at all. So the second attempt asks the same edit holding
    /// nothing, and lets the geometry settle back onto what the constraints
    /// allow — which lands it as near what was asked for as it may go. A point
    /// on an edge slides along it; an arm the pointer has outrun reaches as far
    /// as it can.
    ///
    /// The second attempt is a free solve from where the cursor asked, so it
    /// may in principle settle on a different branch of a mechanism that admits
    /// more than one. It runs only where the first was refused, which is where
    /// nothing moved at all, so nothing that works today can be made worse by
    /// it.
    ///
    /// `edit` may move geometry. It may not add or remove any: `held` and the
    /// residual this is judged against were both taken of the sketch as it
    /// arrived, so an edit that changed what the sketch *is* would be settled
    /// against a system that no longer exists. Adding geometry is
    /// [`Solver::solve`]'s, with nothing held.
    pub fn edit_holding(
        &mut self,
        sketch: &mut Sketch,
        held: &[PointId],
        into: &mut Outcome,
        edit: impl FnOnce(&mut Sketch),
    ) {
        // Only the residual, so the pre-edit look costs one assembly and no
        // elimination: nothing is being reported about the sketch as it stands,
        // only judged against what the edit leaves.
        let was = self.assemble_at_rest(sketch);
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
        // Judged before it is described: an attempt that is not kept is one
        // nothing will be asked about, and working out what the sketch could
        // still do would be working it out for a sketch about to be put back.
        let iterations = self.iterate(sketch, held);
        if self.takes(sketch, was) {
            return self.read_at_rest(sketch, into, Settled::Holding, iterations);
        }

        // Held, that was impossible — the cursor asked for somewhere the
        // constraints do not reach. So ask again without holding anything, from
        // what the edit asked for rather than from where the first attempt gave
        // up: the geometry is then free to settle back onto what the
        // constraints allow, which lands it as near the cursor as it may go. A
        // point held to an edge slides along it, and an arm outrun by the
        // pointer reaches as far as it can rather than freezing where it was.
        // Holding nothing is what the first attempt already did, so there is no
        // second answer to be had — and asking again would be the same solve
        // run twice for the same refusal.
        if !held.is_empty() {
            sketch.restore(&self.asked);
            let iterations = self.iterate(sketch, &[]);
            // Kept only where it moved the sketch at all. One with nowhere to
            // go answers this attempt by putting everything back where it
            // always had to be — and arriving there a second time by way of a
            // solve is not arriving there in the same *bits*, so a caller
            // comparing snapshots would read a step to take back that nobody
            // took. The whole sketch rather than the held points, because what
            // a drag moves need not be one: driving a radius holds the circle's
            // centre, and the centre staying put says nothing about whether the
            // circle grew.
            let moved = !self.before.within(sketch, UNMOVED);
            if moved && self.takes(sketch, was) {
                return self.read_at_rest(sketch, into, Settled::Freely, iterations);
            }
        }

        sketch.restore(&self.before);
        // Described afresh, because the sketch being described is now the one
        // that was there all along rather than either attempt on it — and
        // [`Settled::Refused`] is the only part of this that a report of a
        // sketch standing at its own solution could not say.
        self.measure_taking(sketch, into, Settled::Refused, 0);
    }

    /// Which of the sketch's geometry its constraints leave anything to
    /// decide, and which they pin down completely.
    ///
    /// The breakdown behind
    /// [`Freedoms::degrees_of_freedom`](crate::Freedoms::degrees_of_freedom):
    /// that counts the freedoms a sketch has left and this says whose they are,
    /// which is what lets a drawing show the difference rather than only total
    /// it.
    ///
    /// Measured where the sketch currently stands, and so only as good as that:
    /// determinacy is a property of the constraints *linearised here*, and a
    /// mechanism folded flat against itself reads determined at exactly the
    /// pose where it is momentarily unable to move.
    ///
    /// Fills `into` rather than returning it, so a drawing measuring itself
    /// after every edit keeps one buffer instead of being handed a new one.
    pub fn measure(&mut self, sketch: &Sketch, into: &mut Outcome) {
        self.measure_taking(sketch, into, Settled::AtRest, 0);
    }

    /// Assemble the sketch at rest and read the whole of what it says about
    /// itself, with `iterations` recorded as how it got there.
    ///
    /// The two halves together, which is what every entry point that is not
    /// judging an edit wants. An edit has to decide whether an attempt is worth
    /// keeping before it is worth describing, so it calls them separately.
    fn measure_taking(
        &mut self,
        sketch: &Sketch,
        into: &mut Outcome,
        settled: Settled,
        iterations: u32,
    ) {
        self.assemble_at_rest(sketch);
        self.read_at_rest(sketch, into, settled, iterations);
    }

    /// Read what the assembly already in the workspace says the sketch can do.
    ///
    /// Carries on from [`Solver::assemble_at_rest`] rather than assembling
    /// again, so a caller that has already judged the residual pays for the
    /// elimination alone. Nothing may have moved the sketch in between: the
    /// rows being reduced are the ones that assembly built.
    ///
    /// One elimination for all of it. The rank the null space is read from is
    /// the same rank the degrees of freedom are counted against, so the total
    /// and the per-entity labels are two resolutions of one answer rather than
    /// two answers that have to be kept in step.
    ///
    /// Always at rest, whatever a solve was holding. Determinacy is a property
    /// of the sketch and not of the drag being attempted on it, and a count
    /// taken with a point held would say the sketch had less freedom than it
    /// does for as long as someone was holding it.
    fn read_at_rest(
        &mut self,
        sketch: &Sketch,
        into: &mut Outcome,
        settled: Settled,
        iterations: u32,
    ) {
        let n = sketch.params().count();
        let rank = self.work.null_space(n);

        let free_params = self.work.movable.iter().filter(|&&free| free).count();
        into.freedoms.reset(
            sketch,
            free_params - rank,
            self.work.system.residuals.len() - rank,
        );
        for (id, _) in sketch.points() {
            // A point's two parameters are adjacent, x first.
            let x = sketch.params().of_point(id);
            into.freedoms.set_point(id, self.work.spread(x, x + 1));
        }
        for (id, _) in sketch.circles() {
            into.freedoms
                .set_radius(id, self.work.travel(sketch.params().of_radius(id)));
        }
        // The same rank read the other way round: the rows it left behind are
        // the equations the rest of the system already implies, and each one
        // names the constraint that wrote it.
        for id in self.work.dependent() {
            into.freedoms.set_redundant(id);
        }

        let max_residual = max_abs(&self.work.system.residuals);
        into.report = SolveReport {
            converged: max_residual <= self.tolerance,
            iterations,
            max_residual,
        };
        into.settled = settled;
    }

    /// Build the residuals and Jacobian for the sketch as it stands with
    /// nothing held, and answer the largest residual left over.
    ///
    /// The state every question about the sketch itself, rather than about a
    /// drag on it, has to be asked of. One assembly and no elimination, which
    /// is the whole of what judging an edit needs — and the assembly stays in
    /// the workspace, so a caller that decides to keep the answer reads the
    /// rest of it off with [`Solver::read_at_rest`] instead of building it
    /// again.
    fn assemble_at_rest(&mut self, sketch: &Sketch) -> f64 {
        self.work.hold(sketch, &[]);
        self.work.system.assemble(sketch, &self.work.movable);
        max_abs(&self.work.system.residuals)
    }

    /// Whether the sketch as it stands is one to keep: satisfied outright, or
    /// at least no less satisfied than it was at `was`.
    ///
    /// Judged on the residual rather than on convergence alone, so a sketch
    /// whose constraints already conflict can still be edited: what is refused
    /// is a step that leaves it *less* satisfied than it was, not one that
    /// merely fails to finish the job.
    fn takes(&mut self, sketch: &Sketch, was: f64) -> bool {
        let residual = self.assemble_at_rest(sketch);
        residual <= self.tolerance || residual <= was
    }
}

pub(crate) mod freedoms;
pub(crate) mod outcome;
mod system;
mod workspace;

#[cfg(feature = "bench")]
pub(crate) mod bench;

#[cfg(test)]
pub(crate) mod internals {
    use crate::sketch::solver::Solver;
    use crate::sketch::{PointId, Sketch};

    impl Solver {
        /// How many degrees of freedom the sketch has left with `held` pinned.
        ///
        /// Against the system a drag on those points would solve, which nothing
        /// in the API reports any more: what a caller wants to know is what the
        /// *sketch* can do, not what it could do while someone is holding it.
        /// Kept because holding a point and asking again is a second route to
        /// the answer the freedoms give, and two routes agreeing is what says
        /// either is right.
        pub(crate) fn freedom_holding(&mut self, sketch: &Sketch, held: &[PointId]) -> usize {
            let n = sketch.params().count();
            self.work.reset(sketch, held);
            self.work.system.assemble(sketch, &self.work.movable);
            let rank = self.work.null_space(n);
            self.work.movable.iter().filter(|&&free| free).count() - rank
        }
    }
}

#[cfg(test)]
mod tests;
