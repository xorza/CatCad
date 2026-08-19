//! What a round curve costs to draw straight.

use std::f64::consts::TAU;

/// How many chords a turn of `sweep` on `radius` is worth, none of them further
/// than `sagitta` from the true arc.
///
/// **The one rule, and it is one function because two would drift.** Everything
/// that turns a curve into corners reads this — a sketch face put through the
/// filler, a solid's face put through the mesher — and two of them cutting the
/// same arc at different places is a hairline in a picture that nobody can find
/// by reading either.
///
/// At least one, whatever is asked: a chord is the coarsest an arc can be, and
/// answering with none would be answering that the arc is not there.
pub(crate) fn chords(radius: f64, sweep: f64, sagitta: f64) -> usize {
    // A chord subtending `φ` sits `r(1 − cos(φ/2))` from the arc at its
    // furthest, so the sagitta asks for chords no wider than this angle.
    let widest = if sagitta >= radius {
        TAU
    } else {
        2.0 * (1.0 - sagitta / radius).acos()
    };
    (sweep.abs() / widest.max(f64::MIN_POSITIVE))
        .ceil()
        .max(1.0) as usize
}
