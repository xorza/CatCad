//! Where a quadratic is nought.
//!
//! Two entries over one piece of arithmetic, differing in nothing but what
//! they make of a double root. A ray cast at a surface counts crossings and a
//! graze must not change that count; an arrangement splits a curve where
//! another touches it and the graze is the whole answer. So the policy is named
//! at the call and the algebra is written once.
//!
//! **Nought exactly, and near-nought is the caller's.** How near counts as
//! degenerate depends on what `a`, `b` and `c` came from — they are not
//! normalized and nothing here knows their scale — so a caller whose `a` can
//! approach nought smoothly owes itself a bound of its own. What a tiny `a`
//! gives back is one root near where a linear would put it and one enormous,
//! and the enormous one is a place with no significant digits left in it.

/// The two places `a·t² + b·t + c` is nought, in order, or `None` where it is
/// nowhere.
///
/// **Two or none, never one.** A double root is a line grazing what it is held
/// against, and an answer that turns on which side of nought a discriminant
/// landed is an answer that flickers: the same ray a hair either way would
/// report two crossings or none, and a count of crossings is what decides
/// whether a place is inside a solid. A caller that wants the tangency itself
/// asks [`grazing_roots`] for it.
///
/// `None` for an `a` of nought as well, which is not a quadratic: a ray along a
/// cylinder's axis, or one that never leans towards a cone.
pub(crate) fn roots(a: f64, b: f64, c: f64) -> Option<[f64; 2]> {
    let under = b * b - 4.0 * a * c;
    if under <= 0.0 {
        return None;
    }
    stable(a, b, c, under)
}

/// The same, with a graze answered as the double root it stands for.
///
/// For a caller after the *place* one curve touches another rather than a count
/// of how many times it goes through: an arrangement splits at a tangency, and
/// dropping it there leaves a corner nobody can find again. The pair comes back
/// with both ends at that one place, so a caller folding near-equal roots into
/// one needs no case for it.
pub(crate) fn grazing_roots(a: f64, b: f64, c: f64) -> Option<[f64; 2]> {
    let under = b * b - 4.0 * a * c;
    if under < 0.0 {
        return None;
    }
    stable(a, b, c, under)
}

/// Both roots, given a discriminant already known not to be negative.
///
/// **The stable form**, which is worth the two lines. Taking `(−b ± √Δ)/2a`
/// both ways subtracts two near-equal numbers for whichever root is small,
/// which is exactly the root a ray grazing a surface has — so the naive form is
/// least accurate where the geometry is hardest.
fn stable(a: f64, b: f64, c: f64, under: f64) -> Option<[f64; 2]> {
    debug_assert!(under >= 0.0, "{under} has no square root to take");
    if a == 0.0 {
        return None;
    }
    if under == 0.0 {
        // The two ends of a chord that has closed to nothing, which the form
        // below cannot reach: at a double root `c / split` is `0 / 0` whenever
        // `b` is nought too.
        let doubled = -0.5 * b / a;
        return Some([doubled, doubled]);
    }
    // `b.signum()` is 1 at nought, which is the branch that keeps the sum away
    // from cancelling — and at nought there is nothing to cancel either way.
    let split = -0.5 * (b + b.signum() * under.sqrt());
    let (one, two) = (split / a, c / split);
    Some(if one <= two { [one, two] } else { [two, one] })
}

#[cfg(test)]
mod tests;
