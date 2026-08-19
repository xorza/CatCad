//! Where a quadratic is nought.

/// The two places `a·t² + b·t + c` is nought, in order, or `None` where it is
/// nowhere.
///
/// **Two or none, never one.** A double root is a line grazing what it is held
/// against, and an answer that turns on which side of nought a discriminant
/// landed is an answer that flickers: the same ray a hair either way would
/// report two crossings or none, and a count of crossings is what decides
/// whether a place is inside a solid. A caller that wants the tangency itself
/// asks the tolerance ladder for it rather than the arithmetic.
///
/// `None` for an `a` of nought as well, which is not a quadratic: a ray along a
/// cylinder's axis, or one that never leans towards a cone.
///
/// **Nought exactly, and near-nought is the caller's.** How near counts as
/// degenerate depends on what the three came from — they are not normalized and
/// nothing here knows their scale — so a caller whose `a` can approach nought
/// smoothly owes itself a bound of its own. What a tiny `a` gives back is one
/// root near where a linear would put it and one enormous, and the enormous one
/// is a place with no significant digits left in it.
///
/// **The stable form**, which is worth the two lines. Taking `(−b ± √Δ)/2a`
/// both ways subtracts two near-equal numbers for whichever root is small,
/// which is exactly the root a ray grazing a surface has — so the naive form is
/// least accurate where the geometry is hardest.
pub(crate) fn roots(a: f64, b: f64, c: f64) -> Option<[f64; 2]> {
    let under = b * b - 4.0 * a * c;
    if under <= 0.0 || a == 0.0 {
        return None;
    }
    // `b.signum()` is 1 at nought, which is the branch that keeps the sum away
    // from cancelling — and at nought there is nothing to cancel either way.
    let split = -0.5 * (b + b.signum() * under.sqrt());
    let (one, two) = (split / a, c / split);
    Some(if one <= two { [one, two] } else { [two, one] })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **The roots are the roots, in order, and a graze is a miss.**
    ///
    /// Hand-computed: `t² − 3t + 2` is `(t−1)(t−2)`, and negating it moves both
    /// roots nowhere while turning `a` over — which is the case an unsorted
    /// answer gets backwards.
    #[test]
    fn a_quadratic_answers_both_its_roots_in_order() {
        let near = |got: Option<[f64; 2]>, want: [f64; 2]| {
            let got = got.expect("two roots");
            assert!(
                (got[0] - want[0]).abs() < 1e-12 && (got[1] - want[1]).abs() < 1e-12,
                "{got:?} rather than {want:?}",
            );
        };
        near(roots(1.0, -3.0, 2.0), [1.0, 2.0]);
        near(roots(-1.0, 3.0, -2.0), [1.0, 2.0]);
        // Straddling nought, and the smaller root is the one the stable form is
        // for: `t² + 1e8·t − 1` has roots near `−1e8` and `1e-8`, and the naive
        // form loses the second one entirely.
        let got = roots(1.0, 1e8, -1.0).expect("two roots");
        assert!(
            (got[1] - 1e-8).abs() < 1e-20,
            "the small root came back {}",
            got[1]
        );

        // A double root is a graze, and a graze is a miss.
        assert_eq!(roots(1.0, -2.0, 1.0), None, "a tangent line reported a hit");
        assert_eq!(roots(1.0, 0.0, 1.0), None, "a miss reported a hit");
        // And nothing quadratic about it at all.
        assert_eq!(roots(0.0, 2.0, -1.0), None);
    }
}
