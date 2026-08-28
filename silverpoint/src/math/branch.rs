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
