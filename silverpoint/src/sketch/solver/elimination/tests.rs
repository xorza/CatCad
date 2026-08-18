//! What the reduction promises, against a walk that takes none of its
//! shortcuts.

use crate::sketch::solver::elimination::{Elimination, RANK_TOLERANCE};
use crate::sketch::solver::system::System;

/// The reduction, with none of the skipping: every row walked whole, every
/// column carried through the echelon pass.
///
/// Written out rather than shared with the one above, because "the same
/// answer without the shortcuts" is the whole of what it is for. What comes
/// back is what a caller reads: which columns pivoted, which were left free,
/// which equation each row ended as, and the null space built off them.
fn reference(
    jacobian: &[f64],
    movable: &[bool],
    n: usize,
) -> (Vec<usize>, Vec<usize>, Vec<usize>, Vec<f64>) {
    let mut a = jacobian.to_vec();
    let m = a.len() / n;
    let mut origin: Vec<usize> = (0..m).collect();
    let scale = a.iter().fold(0.0f64, |acc, v| acc.max(v.abs())).max(1.0);
    let tolerance = RANK_TOLERANCE * scale;
    let (mut pivots, mut free) = (Vec::new(), Vec::new());
    let mut rank = 0;
    for col in 0..n {
        if !movable[col] {
            continue;
        }
        if rank == m {
            free.push(col);
            continue;
        }
        let mut pivot = rank;
        for row in rank..m {
            if a[row * n + col].abs() > a[pivot * n + col].abs() {
                pivot = row;
            }
        }
        if a[pivot * n + col].abs() <= tolerance {
            free.push(col);
            continue;
        }
        if pivot != rank {
            for c in 0..n {
                a.swap(pivot * n + c, rank * n + c);
            }
            origin.swap(pivot, rank);
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
        pivots.push(col);
        rank += 1;
    }
    for row in (0..rank).rev() {
        let pivot = pivots[row];
        let diagonal = a[row * n + pivot];
        for c in 0..n {
            a[row * n + c] /= diagonal;
        }
        for above in 0..row {
            let factor = a[above * n + pivot];
            if factor == 0.0 {
                continue;
            }
            for c in 0..n {
                a[above * n + c] -= factor * a[row * n + c];
            }
        }
    }
    let axes = free.len();
    let mut null = vec![0.0; n * axes];
    for (axis, &col) in free.iter().enumerate() {
        null[col * axes + axis] = 1.0;
    }
    for (row, &pivot) in pivots.iter().enumerate() {
        for (axis, &col) in free.iter().enumerate() {
            null[pivot * axes + axis] = -a[row * n + col];
        }
    }
    (pivots, free, origin, null)
}

/// How far the two walks may part on a null-space entry.
///
/// Not zero: the reduction sets an eliminated cell to the zero it is defined
/// to hold where the reference subtracts its way to a rounding of one, so the
/// two carry different last bits from there on. Absolute rather than
/// relative for the small entries, since what a passed-over column holds is
/// near zero and a relative bound on it would be a bound on noise.
///
/// Two decades under the difference that dropping the passed-over columns
/// from the update makes — measured at 6e-13 — so the sweep still fails if
/// they are dropped, and two decades over anything the two walks' rounding
/// has been seen to produce.
const CLOSE: f64 = 1e-13;

/// Neither shortcut in the reduction changes what it answers.
///
/// Two claims are being held, and both are about cells being *exactly* zero
/// rather than nearly so — which is why a reference rather than a tolerance.
/// The elimination skips everything outside the stretch a row holds
/// anything in; the echelon pass after it carries only the columns nothing
/// pivoted on, every other one being zero in the row it would be subtracted
/// from.
///
/// Swept over a spread of shapes rather than asserted on one, because which
/// columns come out free is decided by the numbers and a single fixture pins
/// whichever partition it happened to produce. The generator is a cheap
/// deterministic hash, so a failure is reproducible from its seed.
#[test]
fn the_reduction_answers_what_the_unshortened_walk_answers() {
    let (mut seen_free, mut seen_redundant, mut seen_faint) = (false, false, false);
    for seed in 0..300u64 {
        let n = 6 + (seed % 6) as usize;
        let m = 5 + (seed % 8) as usize;
        let mut state = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
        let mut next = || {
            state = state
                .wrapping_mul(6364136223846793005)
                .wrapping_add(1442695040888963407);
            ((state >> 33) as f64 / (1u64 << 30) as f64) - 1.0
        };
        // Local and sparse, the way a sketch's equations are: each row holds
        // a short run of neighbouring columns.
        let mut jacobian = vec![0.0; m * n];
        for row in 0..m {
            let start = (next().abs() * (n - 2) as f64) as usize % (n - 1);
            for col in start..(start + 3).min(n) {
                jacobian[row * n + col] = next();
            }
        }
        // A repeated row, so the rank is short of the count and something is
        // always left over to be named redundant.
        for c in 0..n {
            jacobian[c] = jacobian[n + c];
        }
        // A column nothing meaningfully reaches, but which is not *empty*
        // either: below the tolerance and so passed over, and carrying
        // something all the same. That is the only way a passed-over column
        // holds anything at all, and it is what says the reduction carries
        // them rather than assuming they are zero — a sweep of columns that
        // are exactly empty cannot tell the two apart.
        let faint = (seed % 5) as usize;
        for row in 0..m {
            jacobian[row * n + faint] = 1e-12 * (1.0 + row as f64);
        }
        let movable: Vec<bool> = (0..n).map(|col| col % 7 != 3).collect();
        for row in 0..m {
            for (col, &may) in movable.iter().enumerate() {
                if !may {
                    jacobian[row * n + col] = 0.0;
                }
            }
        }

        let mut system = System::default();
        system.jacobian = jacobian.clone();
        system.movable = movable.clone();

        let mut elimination = Elimination::default();
        elimination.null_space(&system);
        let (pivots, free, origin, null) = reference(&jacobian, &movable, n);

        assert_eq!(elimination.pivots, pivots, "seed {seed}: pivots part");
        assert_eq!(elimination.free, free, "seed {seed}: freedoms part");
        assert_eq!(
            elimination.origin[..pivots.len()],
            origin[..pivots.len()],
            "seed {seed}: the rows that pivoted came from elsewhere"
        );
        assert_eq!(elimination.null.len(), null.len(), "seed {seed}");
        for (at, (&had, &want)) in elimination.null.iter().zip(&null).enumerate() {
            assert!(
                (had - want).abs() <= CLOSE * want.abs().max(1.0),
                "seed {seed}: null space parts at {at}: {had} against {want}"
            );
        }
        seen_free |= !free.is_empty();
        seen_redundant |= pivots.len() < m;
        // The faint column is the one the sweep is really for, and it only
        // reaches the reduction where the mask has not zeroed it out from
        // under it — so whether any seed actually produced one is a thing to
        // ask rather than assume.
        seen_faint |= movable[faint] && free.contains(&faint);
    }
    assert!(
        seen_free,
        "no sweep left a column free, so the echelon pass \
        never carried anything and the claim about it went untested"
    );
    assert!(seen_redundant, "no sweep left a row over");
    assert!(
        seen_faint,
        "no sweep passed over a column that was carrying something, so the \
         one case the reduction has to carry them for went untested"
    );
}
