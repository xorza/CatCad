//! A run across a wave of an angle that wraps.

use crate::inline::Inline;
use glam::DVec2;
use std::f64::consts::{PI, TAU};

/// Where along the run from `from` to `to` the sine of `x − phase` comes to
/// `sine`, in no order.
///
/// **Two answers at most, and the turn is the whole of the difficulty.** A sine
/// takes a value twice a turn, at `asin` of it and at half a turn less that —
/// and each of those stands at every turn of the angle, where the run covers
/// one stretch of it. What comes back is the turn of each that the run's own
/// span holds, and nothing for the one it does not.
///
/// **Where a cut in a cylinder's parameters is fenced.** Both curved cuts a
/// cylinder can carry — the boolean's `Ripple` and `Bow` — turn where the sine
/// of their own angle reaches a value they solve for, and a fence laid at one
/// turn of it and not another is a root walked past.
///
/// Nothing for a run that stands at one angle, which has no span to hold a turn
/// in, and nothing for a value no sine reaches. Half open in how far along, so
/// a run's own far end is left to the run that starts there.
pub(crate) fn met(sine: f64, phase: f64, from: DVec2, to: DVec2) -> Inline<f64, 2> {
    let mut found = Inline::none();
    let run = to.x - from.x;
    if run == 0.0 || sine.abs() > 1.0 {
        return found;
    }
    let (lo, hi) = (from.x.min(to.x), from.x.max(to.x));
    let first = sine.asin();
    for turn in [first, PI - first] {
        let over = ((lo - phase - turn) / TAU).ceil();
        let angle = phase + turn + TAU * over;
        let along = (angle - from.x) / run;
        if angle < hi && (0.0..1.0).contains(&along) {
            found.push(along);
        }
    }
    found
}
