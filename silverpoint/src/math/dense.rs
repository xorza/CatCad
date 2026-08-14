//! Arithmetic over plain `f64` slices.
//!
//! Nothing here knows what a sketch is. A dense symmetric solve, and the two
//! norms that judge what goes into one — the solver reaches for them every
//! iteration, but they would read the same in any crate that had a matrix.

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
/// A non-positive pivot means the damping is too low to hold the matrix
/// definite against rounding. That is the same answer a singular matrix used to
/// give, and the caller answers it the same way, by damping harder and retrying.
pub(crate) fn solve_in_place(a: &mut [f64], n: usize, b: &mut [f64]) -> bool {
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

pub(crate) fn max_abs(values: &[f64]) -> f64 {
    values.iter().fold(0.0f64, |acc, v| acc.max(v.abs()))
}

/// Sum of squares — a norm with its square root left off, for a caller
/// comparing against a squared threshold rather than reading a length.
pub(crate) fn square_norm(values: &[f64]) -> f64 {
    values.iter().map(|v| v * v).sum()
}

pub(crate) fn norm(values: &[f64]) -> f64 {
    square_norm(values).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Worked by hand against a matrix whose factor is exact in binary, so the
    /// assertions can be equalities rather than tolerances.
    ///
    /// Built by choosing `L` and multiplying out, so every entry of the factor is
    /// non-zero — a zero below the diagonal would let an index slip in the
    /// inner product go unnoticed.
    ///
    /// ```text
    /// L = [2 0 0]     L Lᵀ = A = [ 4 2  6]
    ///     [1 2 0]                [ 2 5  5]
    ///     [3 1 2]                [ 6 5 14]
    /// ```
    ///
    /// `L₀₀ = √4 = 2`, `L₁₀ = 2/2 = 1`, `L₁₁ = √(5−1) = 2`, `L₂₀ = 6/2 = 3`,
    /// `L₂₁ = (5−3·1)/2 = 1`, `L₂₂ = √(14−9−1) = 2`. With `x = [1 2 3]`, `A x` is
    /// `[26 27 58]`; forward substitution gives `y = [13 7 6]` and back
    /// substitution returns `x`.
    #[test]
    fn cholesky_factors_and_solves_a_known_system() {
        let mut a = [4.0, 2.0, 6.0, 2.0, 5.0, 5.0, 6.0, 5.0, 14.0];
        let mut b = [26.0, 27.0, 58.0];
        assert!(solve_in_place(&mut a, 3, &mut b));
        assert_eq!(b, [1.0, 2.0, 3.0]);

        // The lower triangle is the factor itself, which pins the arithmetic
        // rather than only the answer it happened to produce.
        let lower = [a[0], a[3], a[4], a[6], a[7], a[8]];
        assert_eq!(lower, [2.0, 1.0, 2.0, 3.0, 1.0, 2.0]);

        // Indefinite: `L₁₁² = 1 − 2² = −3`, so there is no real factor and the
        // caller is told to damp harder rather than handed a wrong step.
        let mut indefinite = [1.0, 2.0, 2.0, 1.0];
        assert!(!solve_in_place(&mut indefinite, 2, &mut [1.0, 1.0]));

        // Singular sits on the same boundary: `L₁₁² = 1 − 1 = 0`.
        let mut singular = [1.0, 1.0, 1.0, 1.0];
        assert!(!solve_in_place(&mut singular, 2, &mut [1.0, 1.0]));

        // A diagonal system is the case with no off-diagonal work to do at all.
        let mut diagonal = [4.0, 0.0, 0.0, 0.25];
        let mut rhs = [2.0, 1.0];
        assert!(solve_in_place(&mut diagonal, 2, &mut rhs));
        assert_eq!(rhs, [0.5, 4.0]);
    }
}
