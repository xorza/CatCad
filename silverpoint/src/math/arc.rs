//! What a round curve costs to draw straight.

use std::f64::consts::{FRAC_PI_2, TAU};

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
    (sweep.abs() / widest(radius, sagitta).max(f64::MIN_POSITIVE))
        .ceil()
        .max(1.0) as usize
}

/// The widest turn a single chord on `radius` may cover and still keep within
/// `sagitta` of the arc.
///
/// A chord subtending `φ` sits `r(1 − cos(φ/2))` from the arc at its furthest,
/// so this is that read the other way round. A whole turn where the sagitta is
/// as large as the radius, there being nothing left to divide.
///
/// **What a mesher measures its own parameters in.** Cutting a face into
/// triangles is a question about a *region* of parameters, not about one curve,
/// and the answer wanted is which way round the region is long — see
/// [`Mesher`](crate::Mesher). This is that unit, and it is the same one
/// [`chords`] cuts the boundary at, which is what keeps the two from drifting.
pub(crate) fn widest(radius: f64, sagitta: f64) -> f64 {
    if sagitta >= radius {
        TAU
    } else {
        2.0 * (1.0 - sagitta / radius).acos()
    }
}

/// The share of a radius a chord covering `spread` radians falls short by,
/// `1 − cos(spread/2)`.
///
/// [`widest`] read the other way round, and here beside it for the reason the
/// note on [`chords`] gives: `radius · bulge(widest(radius, sagitta))` is the
/// sagitta again, and two spellings of one relation in two modules is exactly
/// how they come to disagree.
///
/// Held at the whole radius past a half turn, where a chord passes the middle
/// and there is no falling further — the formula goes on climbing there, and a
/// bound that overstated itself without limit would ask for a mesh finer than
/// any sagitta wanted.
pub(crate) fn bulge(spread: f64) -> f64 {
    1.0 - (spread.abs() * 0.5).min(FRAC_PI_2).cos()
}
