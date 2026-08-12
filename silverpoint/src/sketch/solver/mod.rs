//! Levenberg-Marquardt over the constraint residuals.
//!
//! Each iteration assembles the residual vector and its Jacobian, forms the
//! damped normal equations `(JᵀJ + λD) δ = -Jᵀr`, and takes the step if it
//! reduces the residual norm — raising the damping toward gradient descent
//! when it doesn't. Matrices are dense: a sketch has two parameters per point
//! plus one per circle, so the cost of sparsity bookkeeping would exceed what
//! it saves.

use crate::sketch::Sketch;

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

/// Solves a [`Sketch`] in place.
#[derive(Debug, Clone, Copy)]
pub struct Solver {
    pub max_iterations: u32,
    /// Converged once every residual is within this of zero. Residuals are in
    /// sketch units (lengths) or their squares (angles), so this is an
    /// absolute tolerance on the geometry, not a relative one.
    pub tolerance: f64,
}

impl Default for Solver {
    fn default() -> Self {
        Self {
            max_iterations: 100,
            tolerance: 1e-10,
        }
    }
}

impl Solver {
    /// Move the sketch's free geometry until its constraints are satisfied.
    ///
    /// The sketch is left at the best position found, converged or not — a
    /// failed solve still leaves it closer than it started, which is what a
    /// UI wants to draw.
    pub fn solve(&self, sketch: &mut Sketch) -> SolveReport {
        let n = sketch.param_count();
        let mut residuals = Vec::new();
        let mut jacobian = Vec::new();
        let mut trial_residuals = Vec::new();
        let mut trial_jacobian = Vec::new();
        let mut normal = vec![0.0; n * n];
        let mut step = vec![0.0; n];
        let mut params = sketch.params();
        let mut damping = INITIAL_DAMPING;
        let mut iterations = 0;

        assemble(sketch, &mut residuals, &mut jacobian);
        while iterations < self.max_iterations && max_abs(&residuals) > self.tolerance {
            iterations += 1;
            normal.fill(0.0);
            step.fill(0.0);
            for (row, residual) in jacobian.chunks_exact(n).zip(&residuals) {
                for a in 0..n {
                    if row[a] == 0.0 {
                        continue;
                    }
                    step[a] -= row[a] * residual;
                    for b in 0..n {
                        normal[a * n + b] += row[a] * row[b];
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
            let curvature = (0..n).fold(1.0f64, |acc, a| acc.max(normal[a * n + a]));
            for a in 0..n {
                if sketch.param_is_free(a) {
                    normal[a * n + a] += damping * curvature;
                } else {
                    // A fixed parameter has an all-zero column, which would
                    // make the system singular. A unit diagonal against a zero
                    // gradient yields a zero step for it instead.
                    normal[a * n + a] = 1.0;
                }
            }

            if !solve_in_place(&mut normal, n, &mut step) {
                damping *= DAMPING_GROWTH;
                if damping > MAX_DAMPING {
                    break;
                }
                continue;
            }

            let trial: Vec<f64> = params.iter().zip(&step).map(|(p, d)| p + d).collect();
            sketch.set_params(&trial);
            assemble(sketch, &mut trial_residuals, &mut trial_jacobian);
            if norm(&trial_residuals) < norm(&residuals) {
                params = trial;
                std::mem::swap(&mut residuals, &mut trial_residuals);
                std::mem::swap(&mut jacobian, &mut trial_jacobian);
                damping = (damping * DAMPING_DECAY).max(f64::MIN_POSITIVE);
            } else {
                sketch.set_params(&params);
                damping *= DAMPING_GROWTH;
                if damping > MAX_DAMPING {
                    break;
                }
            }
        }

        let free_params = (0..n).filter(|&p| sketch.param_is_free(p)).count();
        let rank = rank(&jacobian, n, sketch);
        SolveReport {
            converged: max_abs(&residuals) <= self.tolerance,
            iterations,
            max_residual: max_abs(&residuals),
            degrees_of_freedom: free_params - rank,
            redundant_equations: residuals.len() - rank,
        }
    }
}

/// Build the residual vector and its row-major Jacobian for the sketch as it
/// currently stands. Columns of fixed parameters are zeroed, which is what
/// holds those points still.
fn assemble(sketch: &Sketch, residuals: &mut Vec<f64>, jacobian: &mut Vec<f64>) {
    let n = sketch.param_count();
    residuals.clear();
    jacobian.clear();
    for constraint in sketch.constraints() {
        for equation in 0..constraint.equation_count() {
            let start = jacobian.len();
            jacobian.resize(start + n, 0.0);
            let row = &mut jacobian[start..];
            residuals.push(constraint.evaluate(sketch, equation, row));
            for (param, partial) in row.iter_mut().enumerate() {
                if !sketch.param_is_free(param) {
                    *partial = 0.0;
                }
            }
        }
    }
}

/// Gaussian elimination with partial pivoting, overwriting `a` and solving
/// into `b`. False when the matrix is singular to working precision.
fn solve_in_place(a: &mut [f64], n: usize, b: &mut [f64]) -> bool {
    for col in 0..n {
        let mut pivot = col;
        for row in (col + 1)..n {
            if a[row * n + col].abs() > a[pivot * n + col].abs() {
                pivot = row;
            }
        }
        if a[pivot * n + col] == 0.0 {
            return false;
        }
        if pivot != col {
            for c in 0..n {
                a.swap(pivot * n + c, col * n + c);
            }
            b.swap(pivot, col);
        }
        let diagonal = a[col * n + col];
        for row in (col + 1)..n {
            let factor = a[row * n + col] / diagonal;
            if factor == 0.0 {
                continue;
            }
            for c in col..n {
                a[row * n + c] -= factor * a[col * n + c];
            }
            b[row] -= factor * b[col];
        }
    }
    for col in (0..n).rev() {
        let mut sum = b[col];
        for c in (col + 1)..n {
            sum -= a[col * n + c] * b[c];
        }
        b[col] = sum / a[col * n + col];
    }
    b.iter().all(|value| value.is_finite())
}

/// Rank of the Jacobian over its free columns — the number of independent
/// constraints actually acting on the sketch.
fn rank(jacobian: &[f64], n: usize, sketch: &Sketch) -> usize {
    if n == 0 || jacobian.is_empty() {
        return 0;
    }
    let mut a = jacobian.to_vec();
    let m = a.len() / n;
    let scale = a.iter().fold(0.0f64, |acc, v| acc.max(v.abs())).max(1.0);
    let tolerance = RANK_TOLERANCE * scale;
    let mut rank = 0;
    for col in 0..n {
        if !sketch.param_is_free(col) || rank == m {
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

fn max_abs(values: &[f64]) -> f64 {
    values.iter().fold(0.0f64, |acc, v| acc.max(v.abs()))
}

fn norm(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum::<f64>().sqrt()
}

#[cfg(test)]
mod tests;
