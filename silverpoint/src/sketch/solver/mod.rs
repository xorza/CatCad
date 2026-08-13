//! Levenberg-Marquardt over the constraint residuals.
//!
//! Each iteration assembles the residual vector and its Jacobian, forms the
//! damped normal equations `(JᵀJ + λD) δ = -Jᵀr`, and takes the step if it
//! reduces the residual norm — raising the damping toward gradient descent
//! when it doesn't. Matrices are dense: a sketch has two parameters per point
//! plus one per circle, so the cost of sparsity bookkeeping would exceed what
//! it saves.

use crate::sketch::{PointId, Sketch};

/// Damping starts here and moves by these factors on an accepted or rejected
/// step. Rejections back off harder than acceptances close in, which keeps a
/// bad region from being re-entered repeatedly.
const INITIAL_DAMPING: f64 = 1e-6;
const DAMPING_DECAY: f64 = 0.3;
const DAMPING_GROWTH: f64 = 8.0;

/// Past this the step is numerically zero, so no further damping can help.
const MAX_DAMPING: f64 = 1e12;

/// Relative threshold below which a pivot counts as zero when measuring the
/// rank of the Jacobian.
const RANK_TOLERANCE: f64 = 1e-9;

/// What a solve achieved, and how determined the answer was.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SolveReport {
    /// Every residual landed within the solver's tolerance.
    pub converged: bool,
    pub iterations: u32,
    /// Largest absolute residual left over.
    pub max_residual: f64,
    /// Free parameters the constraints leave undetermined. Zero is a fully
    /// constrained sketch; higher means it can still be dragged, by exactly
    /// this many independent motions.
    pub degrees_of_freedom: usize,
    /// Equations beyond the rank of the system. On a converged solve these
    /// are consistent duplicates; on a failed one they are the conflict.
    pub redundant_equations: usize,
}

/// Everything a solve needs room for, kept between calls.
///
/// Sized by the sketch and refilled rather than rebuilt, so a second solve of
/// the same sketch — which is what dragging one is — allocates nothing at all.
/// Nothing in here survives a solve as *meaning*; only as room.
#[derive(Debug, Clone, Default)]
struct Workspace {
    residuals: Vec<f64>,
    /// Row-major, one row of `n` per equation.
    jacobian: Vec<f64>,
    trial_residuals: Vec<f64>,
    trial_jacobian: Vec<f64>,
    /// `JᵀJ + λD`, `n × n` row-major.
    normal: Vec<f64>,
    step: Vec<f64>,
    params: Vec<f64>,
    trial: Vec<f64>,
    /// Measuring rank destroys what it eliminates, so it runs on a copy of
    /// the Jacobian rather than on the Jacobian.
    elimination: Vec<f64>,
    /// Parameter indices the caller is holding, two per point. Kept as indices
    /// rather than handles so the inner loops compare integers.
    held: Vec<usize>,
}

impl Workspace {
    /// Size the fixed-length buffers to a sketch of `n` parameters and load
    /// its current values, keeping whatever room everything has grown to.
    ///
    /// The variable-length ones are left alone: `assemble` clears the two
    /// residual/Jacobian pairs as it fills them, and `trial` and `elimination`
    /// are cleared where they are written.
    fn reset(&mut self, sketch: &Sketch, n: usize, held: &[PointId]) {
        self.normal.clear();
        self.normal.resize(n * n, 0.0);
        self.step.clear();
        self.step.resize(n, 0.0);
        self.params.clear();
        sketch.write_params(&mut self.params);
        self.held.clear();
        self.held.reserve_exact(held.len() * 2);
        for &point in held {
            // A point's two parameters are adjacent, x first.
            let x = sketch.point_param(point);
            self.held.extend([x, x + 1]);
        }
    }

    /// Rank of the Jacobian over its free columns — the number of independent
    /// constraints actually acting on the sketch.
    fn rank(&mut self, n: usize, sketch: &Sketch) -> usize {
        if n == 0 || self.jacobian.is_empty() {
            return 0;
        }
        self.elimination.clear();
        self.elimination.extend_from_slice(&self.jacobian);
        let a = &mut self.elimination;
        let m = a.len() / n;
        let scale = a.iter().fold(0.0f64, |acc, v| acc.max(v.abs())).max(1.0);
        let tolerance = RANK_TOLERANCE * scale;
        let mut rank = 0;
        for col in 0..n {
            if !movable(sketch, &self.held, col) || rank == m {
                continue;
            }
            let mut pivot = rank;
            for row in rank..m {
                if a[row * n + col].abs() > a[pivot * n + col].abs() {
                    pivot = row;
                }
            }
            if a[pivot * n + col].abs() <= tolerance {
                continue;
            }
            if pivot != rank {
                for c in 0..n {
                    a.swap(pivot * n + c, rank * n + c);
                }
            }
            let diagonal = a[rank * n + col];
            for row in (rank + 1)..m {
                let factor = a[row * n + col] / diagonal;
                if factor == 0.0 {
                    continue;
                }
                for c in 0..n {
                    a[row * n + c] -= factor * a[rank * n + c];
                }
            }
            rank += 1;
        }
        rank
    }
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
    /// Where the geometry stood before the edit being attempted, so one the
    /// constraints cannot take can be put back. Outside [`Workspace`] because
    /// it outlives a solve rather than serving one.
    before: Vec<f64>,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-10,
            work: Workspace::default(),
            before: Vec::new(),
        }
    }
}

impl Solver {
    /// Move the sketch's free geometry until its constraints are satisfied.
    ///
    /// The sketch is left at the best position found, converged or not — a
    /// failed solve still leaves it closer than it started, which is what a
    /// UI wants to draw.
    pub fn solve(&mut self, sketch: &mut Sketch) -> SolveReport {
        self.solve_holding(sketch, &[])
    }

    /// Solve with `held` pinned where they are, whatever their own
    /// [`Sketch::is_fixed`] says.
    ///
    /// What dragging needs. The point under the cursor stays where the cursor
    /// put it and the rest of the sketch moves to accommodate it, which is the
    /// difference between dragging a drawing and watching it snap back.
    ///
    /// Held rather than fixed, because [`Sketch::fix`] is the user's statement
    /// about the drawing: a point does not become pinned because someone is
    /// holding it, and anything reading that flag — the marker it is drawn
    /// with, the degrees of freedom reported at rest — would be told it did.
    ///
    /// A sketch with nothing left to give reports `converged: false`. Holding
    /// a point of a fully-determined drawing asks for a motion its constraints
    /// forbid, and that it cannot be had is the honest answer rather than a
    /// failure.
    pub fn solve_holding(&mut self, sketch: &mut Sketch, held: &[PointId]) -> SolveReport {
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

        self.tally(sketch, iterations)
    }

    /// Move the sketch's geometry with `edit`, then settle the rest around it
    /// with `held` pinned — putting the sketch back exactly as it was found if
    /// the constraints cannot take the step.
    ///
    /// What a drag is made of, and the reason it is one call rather than an
    /// edit followed by [`Solver::solve_holding`]. Dragging geometry the
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
    /// `edit` may move geometry. It may not add or remove any: what is put back
    /// is the parameter vector, and a sketch that has grown or lost one is no
    /// longer described by the vector that was saved.
    pub fn edit_holding(
        &mut self,
        sketch: &mut Sketch,
        held: &[PointId],
        edit: impl FnOnce(&mut Sketch),
    ) -> SolveReport {
        let was = self.measure(sketch);
        self.before.clear();
        sketch.write_params(&mut self.before);

        edit(sketch);
        debug_assert_eq!(
            sketch.param_count(),
            self.before.len(),
            "an edit may move a sketch's geometry, not add to or remove from it"
        );

        let report = self.solve_holding(sketch, held);
        if report.converged || report.max_residual <= was.max_residual {
            return report;
        }
        sketch.set_params(&self.before);
        was
    }

    /// What the sketch amounts to where it stands, moving nothing.
    ///
    /// The measurements a solve ends with, taken without taking a step — what
    /// a sketch already at rest reports about itself, and so also what to
    /// report for one an edit has just been undone on. Its `iterations` is
    /// zero because none were kept.
    fn measure(&mut self, sketch: &Sketch) -> SolveReport {
        self.work.held.clear();
        assemble(
            sketch,
            &self.work.held,
            &mut self.work.residuals,
            &mut self.work.jacobian,
        );
        self.tally(sketch, 0)
    }

    /// Sum up the residuals and Jacobian as they currently stand.
    ///
    /// Reads the `held` they were assembled against, so what it reports as free
    /// is freedom of the same system that produced them.
    fn tally(&mut self, sketch: &Sketch, iterations: u32) -> SolveReport {
        let n = sketch.param_count();
        let free_params = (0..n)
            .filter(|&p| movable(sketch, &self.work.held, p))
            .count();
        let rank = self.work.rank(n, sketch);
        let max_residual = max_abs(&self.work.residuals);
        SolveReport {
            converged: max_residual <= self.tolerance,
            iterations,
            max_residual,
            degrees_of_freedom: free_params - rank,
            redundant_equations: self.work.residuals.len() - rank,
        }
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

/// Solve `a x = b` in place for a symmetric positive definite `a`, overwriting
/// `a`'s lower triangle with its Cholesky factor and `b` with `x`. False when
/// `a` turns out not to be positive definite to working precision.
///
/// Cholesky rather than Gaussian elimination because the normal equations are
/// SPD by construction: `JᵀJ` is positive semi-definite, damping adds a
/// strictly positive amount to the diagonal of every free parameter, and a
/// fixed one gets an identity row over a column [`assemble`] has already
/// zeroed. That halves the arithmetic, and more to the point it removes
/// pivoting — an SPD matrix needs none, so there is no pivot search and no row
/// swapping here to disagree with the one in [`rank`].
///
/// A non-positive pivot means the damping is too low to hold the matrix
/// definite against rounding. That is the same answer a singular matrix used to
/// give, and the caller answers it the same way, by damping harder and retrying.
fn solve_in_place(a: &mut [f64], n: usize, b: &mut [f64]) -> bool {
    // Cholesky–Banachiewicz: row `i` is built from the rows above it, so every
    // read runs along a row of the layout the matrix is already stored in.
    for i in 0..n {
        for j in 0..=i {
            let mut sum = a[i * n + j];
            for k in 0..j {
                sum -= a[i * n + k] * a[j * n + k];
            }
            if i == j {
                if sum <= 0.0 {
                    return false;
                }
                a[i * n + i] = sum.sqrt();
            } else {
                a[i * n + j] = sum / a[j * n + j];
            }
        }
    }
    for i in 0..n {
        let mut sum = b[i];
        for k in 0..i {
            sum -= a[i * n + k] * b[k];
        }
        b[i] = sum / a[i * n + i];
    }
    // Back through the transpose, which is the same factor read down a column
    // — the one place this layout costs anything, and it is the O(n²) half.
    for i in (0..n).rev() {
        let mut sum = b[i];
        for k in (i + 1)..n {
            sum -= a[k * n + i] * b[k];
        }
        b[i] = sum / a[i * n + i];
    }
    b.iter().all(|value| value.is_finite())
}

/// Whether the solve may move this parameter: free in the sketch, and not
/// pinned for the duration of a drag.
///
/// The one place the two reasons a column stays put are put together, so the
/// Jacobian, the damping and the rank all describe the same system.
fn movable(sketch: &Sketch, held: &[usize], param: usize) -> bool {
    sketch.param_is_free(param) && !held.contains(&param)
}

fn max_abs(values: &[f64]) -> f64 {
    values.iter().fold(0.0f64, |acc, v| acc.max(v.abs()))
}

fn norm(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum::<f64>().sqrt()
}

#[cfg(feature = "bench")]
pub(crate) mod bench;

#[cfg(test)]
mod tests;
