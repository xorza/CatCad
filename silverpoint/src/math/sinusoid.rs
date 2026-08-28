//! A run across a wave of an angle that wraps.

use crate::inline::Inline;
use glam::DVec2;
use std::f64::consts::TAU;

/// The two angles where `round·cos ψ + up·sin ψ` comes to `to`, in no order.
///
/// **One solve for every sinusoid**, which is what a cosine and a sine of one
/// angle added come to: a single wave of size `hypot(round, up)` shifted to
/// `atan2(up, round)`, so the angles it takes a value at are that shift and one
/// `acos` either side of it.
///
/// Nothing where the value stands further off than the wave ever reaches, and
/// nothing for a wave of no size at all — which takes its one value everywhere
/// or nowhere, and neither is an angle to hand back.
pub(crate) fn angles(round: f64, up: f64, to: f64) -> Inline<f64, 2> {
    let mut found = Inline::none();
    let size = round.hypot(up);
    if size == 0.0 || to.abs() > size {
        return found;
    }
    let (turn, share) = (up.atan2(round), (to / size).acos());
    found.push(turn + share);
    found.push(turn - share);
    found
}

/// Where along the run from `from` to `to` the sine of `x − phase` comes to
/// `sine`, in no order.
///
/// **Two answers at most, and the turn is the whole of the difficulty.** A sine
/// takes a value twice a turn — see [`angles`], which is the solve — and each of
/// those stands at every turn of the angle, where the run covers one stretch of
/// it. What comes back is the turn of each that the run's own span holds, and
/// nothing for the one it does not.
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
    if run == 0.0 {
        return found;
    }
    let (lo, hi) = (from.x.min(to.x), from.x.max(to.x));
    for turn in angles(0.0, 1.0, sine) {
        let over = ((lo - phase - turn) / TAU).ceil();
        let angle = phase + turn + TAU * over;
        let along = (angle - from.x) / run;
        if angle < hi && (0.0..1.0).contains(&along) {
            found.push(along);
        }
    }
    found
}
