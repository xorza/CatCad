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
use crate::sketch::solver::freedoms::Freedoms;
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

/// What a solve achieved.
///
/// How *determined* the answer was is not here: that is a property of the
/// sketch rather than of the run, and it comes back in the [`Freedoms`] every
/// entry point fills — see [`Freedoms::degrees_of_freedom`]. Splitting them is
/// what stops the two describing different moments, which is what a report
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
#[derive(Debug, Clone)]
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
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-10,
            work: Workspace::default(),
            before: Snapshot::default(),
        }
    }
}

impl Solver {
    /// Move the sketch's free geometry until its constraints are satisfied.
    ///
    /// The sketch is left at the best position found, converged or not — a
    /// failed solve still leaves it closer than it started, which is what a
    /// UI wants to draw.
    pub fn solve(&mut self, sketch: &mut Sketch, into: &mut Freedoms) -> SolveReport {
        self.solve_holding(sketch, &[], into)
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
        into: &mut Freedoms,
    ) -> SolveReport {
        let iterations = self.iterate(sketch, held);
        self.measure_taking(sketch, into, iterations)
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
        let n = sketch.param_count();
        let work = &mut self.work;
        work.reset(sketch, n, held);
        let mut damping = INITIAL_DAMPING;
        let mut iterations = 0;

        assemble(sketch, &work.held, &mut work.residuals, &mut work.jacobian);
        while iterations < max_iterations && max_abs(&work.residuals) > tolerance {
            iterations += 1;
            work.normal.fill(0.0);
            work.step.fill(0.0);
            for (row, residual) in work.jacobian.chunks_exact(n).zip(&work.residuals) {
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
                if movable(sketch, &work.held, a) {
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
            sketch.set_params(&work.trial);
            assemble(
                sketch,
                &work.held,
                &mut work.trial_residuals,
                &mut work.trial_jacobian,
            );
            if norm(&work.trial_residuals) < norm(&work.residuals) {
                // Swapped rather than assigned: the loser's buffer becomes the
                // next round's scratch, so neither pair is ever rebuilt.
                std::mem::swap(&mut work.params, &mut work.trial);
                std::mem::swap(&mut work.residuals, &mut work.trial_residuals);
                std::mem::swap(&mut work.jacobian, &mut work.trial_jacobian);
                damping = (damping * DAMPING_DECAY).max(f64::MIN_POSITIVE);
            } else {
                sketch.set_params(&work.params);
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
    /// Judged on the residual rather than on convergence alone, so a sketch
    /// whose constraints already conflict can still be dragged: what is refused
    /// is a step that leaves the sketch *less* satisfied than it was, not one
    /// that merely fails to finish the job.
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
        into: &mut Freedoms,
        edit: impl FnOnce(&mut Sketch),
    ) -> SolveReport {
        // Only the residual, so the pre-edit look costs one assembly and no
        // elimination: nothing is being reported about the sketch as it stands,
        // only judged against what the edit leaves.
        let was = self.residual_at_rest(sketch);
        sketch.snapshot_into(&mut self.before);

        edit(sketch);
        debug_assert!(
            self.before.fits(sketch),
            "an edit may move a sketch's geometry, not add to or remove from it"
        );

        let iterations = self.iterate(sketch, held);
        let report = self.measure_taking(sketch, into, iterations);
        if report.converged || report.max_residual <= was {
            return report;
        }
        sketch.restore(&self.before);
        // Measured again, because `into` now describes the attempt rather than
        // what survived it. A refused edit has to leave the caller holding the
        // sketch it still has.
        self.measure_taking(sketch, into, 0)
    }

    /// Which of the sketch's geometry its constraints leave anything to
    /// decide, and which they pin down completely.
    ///
    /// The breakdown behind [`SolveReport::degrees_of_freedom`]: that counts
    /// the freedoms a sketch has left and this says whose they are, which is
    /// what lets a drawing show the difference rather than only total it.
    ///
    /// Measured where the sketch currently stands, and so only as good as that:
    /// determinacy is a property of the constraints *linearised here*, and a
    /// mechanism folded flat against itself reads determined at exactly the
    /// pose where it is momentarily unable to move.
    ///
    /// Fills `into` rather than returning it, so a drawing measuring itself
    /// after every edit keeps one buffer instead of being handed a new one.
    pub fn measure(&mut self, sketch: &Sketch, into: &mut Freedoms) -> SolveReport {
        self.measure_taking(sketch, into, 0)
    }

    /// The whole of what the sketch says about itself where it stands, with
    /// `iterations` recorded as how it got there.
    ///
    /// One assembly and one elimination for all of it. The rank the null space
    /// is read from is the same rank the degrees of freedom are counted
    /// against, so the total and the per-entity labels are two resolutions of
    /// one answer rather than two answers that have to be kept in step.
    ///
    /// Always at rest, whatever a solve was holding. Determinacy is a property
    /// of the sketch and not of the drag being attempted on it, and a count
    /// taken with a point held would say the sketch had less freedom than it
    /// does for as long as someone was holding it.
    fn measure_taking(
        &mut self,
        sketch: &Sketch,
        into: &mut Freedoms,
        iterations: u32,
    ) -> SolveReport {
        let n = sketch.param_count();
        self.assemble_at_rest(sketch);
        let rank = self.work.null_space(n, sketch);

        let free_params = (0..n)
            .filter(|&p| movable(sketch, &self.work.held, p))
            .count();
        into.reset(sketch, free_params - rank, self.work.residuals.len() - rank);
        for (id, _) in sketch.points() {
            // A point's two parameters are adjacent, x first.
            let x = sketch.point_param(id);
            into.set_point(id, self.work.spread(x, x + 1));
        }
        for (id, _) in sketch.circles() {
            into.set_radius(id, self.work.travel(sketch.radius_param(id)));
        }

        let max_residual = max_abs(&self.work.residuals);
        SolveReport {
            converged: max_residual <= self.tolerance,
            iterations,
            max_residual,
        }
    }

    /// The largest residual where the sketch stands, and nothing else.
    ///
    /// One assembly, no elimination — for a caller judging whether an edit left
    /// the sketch better or worse satisfied, which is a question the residuals
    /// answer on their own.
    fn residual_at_rest(&mut self, sketch: &Sketch) -> f64 {
        self.assemble_at_rest(sketch);
        max_abs(&self.work.residuals)
    }

    /// Build the residuals and Jacobian for the sketch as it stands, with
    /// nothing held — the state every question about the sketch itself, rather
    /// than about a drag on it, has to be asked of.
    fn assemble_at_rest(&mut self, sketch: &Sketch) {
        self.work.held.clear();
        assemble(
            sketch,
            &self.work.held,
            &mut self.work.residuals,
            &mut self.work.jacobian,
        );
    }
}

/// Build the residual vector and its row-major Jacobian for the sketch as it
/// currently stands. Columns of fixed parameters are zeroed, which is what
/// holds those points still.
fn assemble(sketch: &Sketch, held: &[usize], residuals: &mut Vec<f64>, jacobian: &mut Vec<f64>) {
    let n = sketch.param_count();
    residuals.clear();
    jacobian.clear();
    for constraint in sketch.constraints() {
        for equation in constraint.equations() {
            let start = jacobian.len();
            jacobian.resize(start + n, 0.0);
            let row = &mut jacobian[start..];
            residuals.push(equation.evaluate(sketch, row));
            for (param, partial) in row.iter_mut().enumerate() {
                if !movable(sketch, held, param) {
                    *partial = 0.0;
                }
            }
        }
    }
}

/// Whether the solve may move this parameter: free in the sketch, and not
/// pinned for the duration of a drag.
///
/// The one place the two reasons a column stays put are put together, so the
/// Jacobian, the damping and the rank all describe the same system.
fn movable(sketch: &Sketch, held: &[usize], param: usize) -> bool {
    sketch.param_is_free(param) && !held.contains(&param)
}

pub(crate) mod freedoms;
mod workspace;

#[cfg(feature = "bench")]
pub(crate) mod bench;

#[cfg(test)]
pub(crate) mod internals {
    use crate::sketch::solver::{Solver, assemble, movable};
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
            let n = sketch.param_count();
            self.work.reset(sketch, n, held);
            assemble(
                sketch,
                &self.work.held,
                &mut self.work.residuals,
                &mut self.work.jacobian,
            );
            let rank = self.work.rank(n, sketch);
            (0..n)
                .filter(|&p| movable(sketch, &self.work.held, p))
                .count()
                - rank
        }
    }
}

#[cfg(test)]
mod tests;
