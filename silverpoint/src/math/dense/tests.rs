//! What the dense solve promises, against matrices worked by hand.

use super::*;

/// A banded matrix solved through its envelope answers exactly what the
/// dense walk answers.
///
/// **Exactly**, to the bit, and that is the whole claim rather than a
/// tolerance: the cells the envelope skips are zero, so the products it
/// skips are zero, so the sums it forms are the same sums with the same
/// terms in the same order. A skyline factorisation that came back *nearly*
/// right would mean the envelope was cutting into something.
///
/// Built as `LLᵀ` from a lower factor that is itself banded, so the matrix
/// is positive definite by construction and its envelope is known rather
/// than assumed. Nine wide, band two — enough that the envelope skips a
/// third of the triangle, which a three-by-three could not show.
#[test]
fn an_envelope_solves_exactly_what_the_dense_walk_solves() {
    const N: usize = 9;
    const BAND: usize = 2;
    // A banded lower factor with a strong diagonal, so `L Lᵀ` is SPD.
    let mut lower = vec![0.0; N * N];
    for i in 0..N {
        for j in i.saturating_sub(BAND)..=i {
            lower[i * N + j] = if i == j {
                3.0 + i as f64
            } else {
                1.0 + (i + j) as f64 * 0.25
            };
        }
    }
    let mut matrix = vec![0.0; N * N];
    for i in 0..N {
        for j in 0..N {
            let cell: f64 = (0..N).map(|k| lower[i * N + k] * lower[j * N + k]).sum();
            matrix[i * N + j] = cell;
        }
    }
    // The envelope the caller would have accumulated, and a check that it
    // really does describe the matrix.
    let first: Vec<usize> = (0..N).map(|i| i.saturating_sub(BAND)).collect();
    for i in 0..N {
        for j in 0..first[i] {
            assert_eq!(matrix[i * N + j], 0.0, "the fixture reaches past {i},{j}");
        }
    }
    assert!(
        first.iter().enumerate().any(|(i, &from)| i - from == BAND),
        "the fixture has no band for the envelope to skip"
    );

    let rhs: Vec<f64> = (0..N).map(|i| 1.0 + i as f64).collect();
    let (mut dense, mut banded) = (matrix.clone(), matrix);
    let (mut through_dense, mut through_envelope) = (rhs.clone(), rhs);
    assert!(solve_in_place(&mut dense, N, &[0; N], &mut through_dense));
    assert!(solve_in_place(
        &mut banded,
        N,
        &first,
        &mut through_envelope
    ));
    assert_eq!(
        through_envelope, through_dense,
        "the envelope changed the answer"
    );
    // And the factor itself, inside the envelope: the two walks wrote the
    // same numbers, not merely numbers that solve the same way.
    for i in 0..N {
        for j in first[i]..=i {
            assert_eq!(
                banded[i * N + j],
                dense[i * N + j],
                "the factors part at {i},{j}"
            );
        }
    }
}

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
    assert!(solve_in_place(&mut a, 3, &[0; 3], &mut b));
    assert_eq!(b, [1.0, 2.0, 3.0]);

    // The lower triangle is the factor itself, which pins the arithmetic
    // rather than only the answer it happened to produce.
    let lower = [a[0], a[3], a[4], a[6], a[7], a[8]];
    assert_eq!(lower, [2.0, 1.0, 2.0, 3.0, 1.0, 2.0]);

    // Indefinite: `L₁₁² = 1 − 2² = −3`, so there is no real factor and the
    // caller is told to damp harder rather than handed a wrong step.
    let mut indefinite = [1.0, 2.0, 2.0, 1.0];
    assert!(!solve_in_place(
        &mut indefinite,
        2,
        &[0; 2],
        &mut [1.0, 1.0]
    ));

    // Singular sits on the same boundary: `L₁₁² = 1 − 1 = 0`.
    let mut singular = [1.0, 1.0, 1.0, 1.0];
    assert!(!solve_in_place(&mut singular, 2, &[0; 2], &mut [1.0, 1.0]));

    // A diagonal system is the case with no off-diagonal work to do at all.
    let mut diagonal = [4.0, 0.0, 0.0, 0.25];
    let mut rhs = [2.0, 1.0];
    assert!(solve_in_place(&mut diagonal, 2, &[0; 2], &mut rhs));
    assert_eq!(rhs, [0.5, 4.0]);
}
