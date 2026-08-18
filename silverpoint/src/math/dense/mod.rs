//! Arithmetic over plain `f64` slices.
//!
//! Nothing here knows what a sketch is. A dense symmetric solve and the one
//! norm that reads a row of the null space — both would read the same in any
//! crate that had a matrix.

/// Solve `a x = b` in place for a symmetric positive definite `a`, overwriting
/// `a`'s lower triangle with its Cholesky factor and `b` with `x`. False when
/// `a` turns out not to be positive definite to working precision.
///
/// Cholesky rather than Gaussian elimination because the normal equations are
/// SPD by construction: `JᵀJ` is positive semi-definite, damping adds a
/// strictly positive amount to the diagonal of every free parameter, and a
/// fixed one gets an identity row over a column the caller has already zeroed.
/// That halves the arithmetic, and more to the point it removes pivoting — an
/// SPD matrix needs none, so there is no pivot search here to disagree with the
/// one a rank-revealing elimination makes.
///
/// `first[i]` is the leftmost column of row `i` that may hold anything — the
/// row's own envelope, which the caller knows because it knows which columns
/// each equation touched. **Everything left of it is skipped rather than
/// multiplied by zero**, and that is exact rather than approximate: a Cholesky
/// factor has the same envelope as the matrix it came from, so nothing is ever
/// written outside one and nothing outside one is ever read.
///
/// What it buys is the difference between the arithmetic growing as `n³` and as
/// `n` times the square of how far a row reaches. A sketch's equations are
/// local — a constraint names at most two entities, so a row touches at most
/// five columns — and where the parameters of things drawn near each other are
/// numbered near each other, that reach stays small however large the drawing
/// is. Where it does not, the envelope fills and this comes back to the dense
/// cost it started at rather than to anything worse. A caller with no envelope
/// to offer passes zeroes and gets exactly the dense factorisation.
///
/// A non-positive pivot means the damping is too low to hold the matrix
/// definite against rounding. That is the same answer a singular matrix used to
/// give, and the caller answers it the same way, by damping harder and retrying.
pub(crate) fn solve_in_place(a: &mut [f64], n: usize, first: &[usize], b: &mut [f64]) -> bool {
    debug_assert_eq!(first.len(), n, "an envelope of another matrix");
    // Cholesky–Banachiewicz: row `i` is built from the rows above it, so every
    // read runs along a row of the layout the matrix is already stored in.
    for i in 0..n {
        let from = first[i];
        debug_assert!(from <= i, "row {i} reaches past its own diagonal to {from}");
        for j in from..=i {
            let mut sum = a[i * n + j];
            // From the wider of the two envelopes: left of either, one of the
            // two factors is zero and the product cannot contribute.
            for k in from.max(first[j])..j {
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
        for k in first[i]..i {
            sum -= a[i * n + k] * b[k];
        }
        b[i] = sum / a[i * n + i];
    }
    // Back through the transpose, scattering rather than gathering. The
    // gathering form reads a *column* of the factor — every row below `i`, to
    // find the few that reach column `i` — and that is a walk of the whole
    // remainder whatever the envelope says, which on a narrow one costs many
    // times the factorisation it follows. Scattering reads rows instead: once
    // `x[i]` is known it is subtracted from everything row `i` reaches, which is
    // exactly its envelope. Same arithmetic, same terms, in the order the layout
    // already stores.
    for i in (0..n).rev() {
        b[i] /= a[i * n + i];
        let resolved = b[i];
        for k in first[i]..i {
            b[k] -= a[i * n + k] * resolved;
        }
    }
    b.iter().all(|value| value.is_finite())
}

/// Sum of squares — a norm with its square root left off, for a caller
/// comparing against a squared threshold rather than reading a length.
pub(crate) fn square_norm(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum()
}

#[cfg(test)]
mod tests;
