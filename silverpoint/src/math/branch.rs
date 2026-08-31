//! Which turn of a wrapping angle a reading means.

use std::f64::consts::TAU;

/// `angle` moved to the turn of it nearest `to`.
///
/// **What keeps a face on a round surface in one piece.** An inversion answers
/// in a half turn either side of the reference direction, so a loop traced
/// across the far side of a cylinder comes back as two stretches of parameter a
/// whole turn apart — and a place asked about afterwards is asked in whichever
/// turn the inversion chose rather than the one the face was laid out in.
///
/// No face may wrap — `.notes/KERNEL.md` §4.4 — so there is exactly one turn a
/// reading could have meant, and the nearest is it.
pub(crate) fn nearest(angle: f64, to: f64) -> f64 {
    angle + TAU * ((to - angle) / TAU).round()
}

/// The middle of the turn from `from` round to `to`, the way the angle grows.
///
/// **Round the circle and not along the line**, which is the whole of it: a
/// stretch running off the end of a turn and back to the start of it reads as
/// the *rest* of the circle when its two ends are merely subtracted, and its
/// midpoint then lands on the far side. Stated here because both readers of it
/// have a stretch that wraps by construction, and because getting it wrong is
/// a place a face is ordered from and a walk seeded at.
pub(crate) fn halfway(from: f64, to: f64) -> f64 {
    from + (to - from).rem_euclid(TAU) / 2.0
}
