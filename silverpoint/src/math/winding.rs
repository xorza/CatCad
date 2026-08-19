//! Which way round a closed run of corners goes, how much it shuts in, and
//! what falls within it.
//!
//! Three readings of one closed polyline, kept together because every caller
//! that wants one wants another: a cut asks what its pieces enclose and which
//! of them hold which hole, and a body asks whether a ray came through a face
//! or grazed its edge.

use crate::math::approx::NO_DIRECTION;
use crate::math::intersect::{self, Span};
use glam::DVec2;

/// Something one of the rules below can read a place off.
///
/// **So that a caller carrying more than a place per corner is still one
/// polyline to these.** A boolean's regions remember where each stretch of
/// their boundary came from — a `Corner` rather than a place — and stripping
/// that back to bare places before asking what a loop encloses would be a copy
/// per loop per cut, on the path a document is rebuilt down sixty times a
/// second.
///
/// One trait rather than a second spelling of each rule, which is the whole
/// point: two of them would drift, and what they answer about is the same
/// polyline either way.
pub(crate) trait Place: Copy {
    fn place(self) -> DVec2;
}

impl Place for DVec2 {
    fn place(self) -> DVec2 {
        self
    }
}

/// Twice the area a closed run of corners shuts in, positive counterclockwise.
///
/// **Twice**, because that is where the shoelace naturally stops and both
/// readers want something different from it: one asks only the sign, and the
/// other holds it against a bound it can halve as easily as this can double.
/// Doubling it here and halving it there would be two roundings for no gain.
///
/// The run is closed whether or not its last corner repeats its first — the
/// walk wraps — so a caller need not decide which convention it is using.
pub(crate) fn swept(walk: &[impl Place]) -> f64 {
    let mut total = 0.0;
    for (at, &here) in walk.iter().enumerate() {
        total += here.place().perp_dot(walk[(at + 1) % walk.len()].place());
    }
    total
}

/// Whether `at` falls within the closed run `walk`, by counting how many times
/// a ray cast to the right of it crosses.
///
/// Odd is within, which is the Jordan curve theorem and nothing more. Through
/// [`intersect::rightward`], so that the one place a ray is held against a
/// straight run is the one the drawing's own containment already goes through
/// — see [`Arrangement`](crate::Arrangement).
///
/// Says nothing useful about a place *on* the run. A caller that cares whether
/// it is on one asks [`off`] first.
pub(crate) fn holds(walk: &[impl Place], at: DVec2) -> bool {
    let crossings = walk
        .iter()
        .enumerate()
        .filter(|&(step, &from)| {
            let span = Span {
                from: from.place(),
                to: walk[(step + 1) % walk.len()].place(),
            };
            intersect::rightward(span, at).is_some_and(|x| x > at.x)
        })
        .count();
    crossings % 2 == 1
}

/// How far `at` stands from the closed run `walk` itself.
///
/// What tells a place safely within a region from one sitting on its edge,
/// which is the question a ray cast has to ask before it trusts its own count:
/// a ray through a corner is counted twice or not at all, and either way the
/// answer is not the one wanted.
///
/// Zero for a run with nothing in it, because there is nothing to be far from.
pub(crate) fn off(walk: &[impl Place], at: DVec2) -> f64 {
    let mut nearest = f64::INFINITY;
    for (step, &from) in walk.iter().enumerate() {
        let from = from.place();
        let to = walk[(step + 1) % walk.len()].place();
        let along = to - from;
        let reach = along.length_squared();
        // A run may double back on a corner; there is no direction in that and
        // the corner itself is as near as anything on it.
        let nearby = if reach < NO_DIRECTION * NO_DIRECTION {
            from
        } else {
            from.lerp(to, ((at - from).dot(along) / reach).clamp(0.0, 1.0))
        };
        nearest = nearest.min(nearby.distance(at));
    }
    if nearest.is_finite() { nearest } else { 0.0 }
}
