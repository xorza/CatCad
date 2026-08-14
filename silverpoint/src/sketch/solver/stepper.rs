//! Levenberg-Marquardt over the constraint residuals.
//!
//! Each iteration assembles the residual vector and its Jacobian, forms the
//! damped normal equations `(JᵀJ + λD) δ = -Jᵀr`, and takes the step if it
//! reduces the residual norm — raising the damping toward gradient descent when
//! it doesn't. Matrices are dense: a sketch has two parameters per point plus
//! one per circle, so the cost of sparsity bookkeeping would exceed what it
//! saves.

use crate::math::dense::solve_in_place;
use crate::sketch::solver::TOLERANCE;
use crate::sketch::solver::system::System;
use crate::sketch::{PointId, Sketch};

/// How many steps a run may take before it gives up.
///
/// A ceiling rather than a target: a sketch that has an answer reaches it in a
/// handful, and one that does not is stopped by the damping giving out or by
/// `STALLED` long before this. What it bounds is the pathological case, so that
/// a drag cannot hang a frame however badly conditioned the geometry is.
const MAX_ITERATIONS: u32 = 100;

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
/// Not [`TOLERANCE`], which asks whether the sketch is *solved*. This asks
/// whether solving it any further is possible — two different questions, which
/// is why the margin between them is worth stating and why neither moves. A
/// system the constraints
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

/// The room a Levenberg-Marquardt run works in.
///
/// Sized by the sketch and refilled rather than rebuilt, so a second run over
/// the same sketch — which is what dragging one is — allocates nothing at all.
///
/// All of it is private and none of it outlives a run. The trial and its
/// parameters are scratch; where the run ended up is in the [`System`] it was
/// handed and in the sketch itself, which is what everything afterwards reads.
#[derive(Debug, Default)]
pub(super) struct Stepper {
    /// What a step would make of the system, assembled under the same mask so
    /// the two can be swapped when the step is worth keeping.
    trial: System,
    /// `JᵀJ + λD`, `n × n` row-major, and only ever the lower triangle of it:
    /// the matrix is symmetric and the Cholesky it is handed to reads nothing
    /// above the diagonal, so the other half is left at the zero
    /// [`Stepper::iterate`] clears it to.
    normal: Vec<f64>,
    step: Vec<f64>,
    /// Which columns the Jacobian row being folded in has anything in, gathered
    /// once per row and read twice — once per pair of them.
    touched: Vec<usize>,
    /// Where the run stands, and where a step would put it — the two points the
    /// system it was handed and [`Stepper::trial`] are the assemblies of.
    params: Vec<f64>,
    trial_params: Vec<f64>,
}

impl Stepper {
    /// Take up to [`MAX_ITERATIONS`] steps, stopping early once every residual
    /// is within [`TOLERANCE`] of zero or the damping gives out, and answer how
    /// many were kept.
    ///
    /// `system` is held for `held` and left holding the sketch as the last kept
    /// step made it, so a caller judging the attempt has the assembly it needs
    /// without building one.
    ///
    /// Says nothing about what it left behind. What the sketch amounts to
    /// afterwards is the caller's to report, and it asks that of the geometry
    /// rather than of the run — so nothing here has to know that a `held` point
    /// is held only for the length of the drag.
    pub(super) fn iterate(
        &mut self,
        sketch: &mut Sketch,
        system: &mut System,
        held: &[PointId],
    ) -> u32 {
        system.hold(sketch, held);
        let n = system.width();
        // A sketch with no parameters can hold no constraints either, so there
        // would be nothing to step towards — and `chunks_exact` below panics on
        // a zero width. Said here rather than left to fall out of an empty
        // residual, which is how it used to hold.
        if n == 0 {
            return 0;
        }
        self.reset(sketch, system);
        let mut damping = INITIAL_DAMPING;
        let mut iterations = 0;

        system.assemble(sketch);
        // How far the residual vector reaches, carried rather than measured
        // afresh each round: after a step worth keeping it is the trial length
        // just taken, and after one that was not it has not moved.
        let mut magnitude = system.magnitude();
        while iterations < MAX_ITERATIONS && system.max_residual() > TOLERANCE {
            iterations += 1;
            self.normal.fill(0.0);
            self.step.fill(0.0);
            for (row, residual) in system.jacobian.chunks_exact(n).zip(&system.residuals) {
                // A constraint names two entities at most, so a row is four
                // cells of work in a sketch of any width — and every cell of
                // `JᵀJ` this row reaches is a pair of them. Gathered once,
                // because walking the row per nonzero costs the whole width
                // again to find the same four.
                self.touched.clear();
                self.touched.extend((0..n).filter(|&col| row[col] != 0.0));
                for (taken, &a) in self.touched.iter().enumerate() {
                    self.step[a] -= row[a] * residual;
                    // The lower triangle alone, since that is all the Cholesky
                    // reads. `touched` climbs, so `..=taken` is exactly `b <= a`.
                    for &b in &self.touched[..=taken] {
                        self.normal[a * n + b] += row[a] * row[b];
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
            let curvature = (0..n).fold(1.0f64, |acc, a| acc.max(self.normal[a * n + a]));
            for a in 0..n {
                if system.movable[a] {
                    self.normal[a * n + a] += damping * curvature;
                } else {
                    // A fixed parameter has an all-zero column, which would make
                    // the system singular. A unit diagonal against a zero
                    // gradient yields a zero step for it instead.
                    self.normal[a * n + a] = 1.0;
                }
            }

            if !solve_in_place(&mut self.normal, n, &mut self.step) {
                damping *= DAMPING_GROWTH;
                if damping > MAX_DAMPING {
                    break;
                }
                continue;
            }

            self.trial_params.clear();
            self.trial_params
                .extend(self.params.iter().zip(&self.step).map(|(p, d)| p + d));
            sketch.params_mut().set(&self.trial_params);
            self.trial.assemble(sketch);
            let trial = self.trial.magnitude();
            if trial < magnitude {
                // Swapped rather than assigned: the loser's buffers become the
                // next round's scratch, so nothing is ever rebuilt.
                std::mem::swap(&mut self.params, &mut self.trial_params);
                std::mem::swap(system, &mut self.trial);
                damping = (damping * DAMPING_DECAY).max(f64::MIN_POSITIVE);
                let reached = magnitude;
                magnitude = trial;
                // Kept, and the last worth taking: a step that improves on its
                // predecessor by nothing is one whose successors improve on it
                // by nothing either, and the residual test above can only stop a
                // system that has an answer to reach.
                if reached - trial <= STALLED * reached {
                    break;
                }
            } else {
                sketch.params_mut().set(&self.params);
                damping *= DAMPING_GROWTH;
                if damping > MAX_DAMPING {
                    break;
                }
            }
        }
        iterations
    }

    /// Size the buffers to the system ahead and load the sketch's current
    /// values, keeping whatever room everything has grown to.
    ///
    /// `trial_params` is left alone: it is cleared where it is written.
    fn reset(&mut self, sketch: &Sketch, system: &System) {
        let n = system.width();
        self.normal.clear();
        self.normal.resize(n * n, 0.0);
        self.step.clear();
        self.step.resize(n, 0.0);
        self.params.clear();
        sketch.params().write(&mut self.params);
        // The trial is compared against the system it will be swapped with, so
        // the two have to be assembled under the same mask.
        self.trial.movable.clone_from(&system.movable);
    }
}
