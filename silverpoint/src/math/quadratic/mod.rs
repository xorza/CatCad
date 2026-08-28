//! Where a quadratic is nought.
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
/// whether a place is inside a solid.
///
/// `None` for an `a` of nought as well, which is not a quadratic: a ray along a
/// cylinder's axis, or one that never leans towards a cone.
///
/// **A caller that has to get the graze itself right does not come here at
/// all.** `b² − 4ac` over coefficients that have already rounded is not the
/// discriminant of the curves that made them, and a tangency is exactly where
/// the two part company — see `intersect::Aimed`, which takes the branch off
/// the places and the radius and then places the root through the exact tier.
/// What is left here is the ray casting, where a graze is a miss by policy and
/// the coefficients are only ever read for a crossing that is not close.
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
mod tests;
