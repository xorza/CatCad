//! Which way round a closed run of corners goes, how much it shuts in, and
//! what falls within it.
//!
//! Three readings of one closed polyline, kept together because every caller
//! that wants one wants another: a cut asks what its pieces enclose and which
//! of them hold which hole, and a body asks whether a ray came through a face
//! or grazed its edge.

use crate::math::intersect::{self, Span};
use crate::number::tolerance::NO_DIRECTION;
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
/// **About the run's own first corner, and that is not a nicety.** The shoelace
/// is the same sum wherever it is taken from, so moving it costs nothing and is
/// exact rather than approximate — but taken about the origin every term is a
/// product of two whole coordinates, and what is wanted is the difference
/// between terms the size of the loop. A unit square at a hundred million has
/// terms of `10¹⁶`, whose places in the last are worth two apiece, so twice its
/// area comes back as nought and the arrangement throws the face away for a
/// sliver. About the first corner the terms are the size of the loop itself and
/// the answer is the answer.
///
/// The run is closed whether or not its last corner repeats its first — the
/// walk wraps — so a caller need not decide which convention it is using.
pub(crate) fn swept(walk: &[impl Place]) -> f64 {
    swept_over(walk.iter().map(|it| it.place()))
}

/// The same, over a run read once rather than held in a slice.
///
/// **One rule and two ways in, not two rules.** A caller whose corners are
/// worked out as they are walked has no slice to hand over, and a buffer for
/// one would be a heap block on the path a body is rebuilt down — see
/// `Traced::holds`. The body is here and [`swept`] is a call to it.
///
/// The closing term is nought, the last corner sweeping nothing against the
/// first, so the run needs no wrap of its own.
pub(crate) fn swept_over(places: impl Iterator<Item = DVec2>) -> f64 {
    let mut places = places;
    let Some(first) = places.next() else {
        return 0.0;
    };
    let (mut total, mut here) = (0.0, first);
    for next in places {
        total += (here - first).perp_dot(next - first);
        here = next;
    }
    total
}

/// Whether `at` falls within the closed run `walk`, by counting how many times
/// a ray cast to the right of it crosses.
///
/// Odd is within, which is the Jordan curve theorem and nothing more. Through
/// [`intersect::blocks`], so that the one place a ray is held against a
/// straight run is the one the drawing's own containment already goes through
/// — see [`Arrangement`](crate::Arrangement) — and so that the parity turns on
/// a determinant rather than on a quotient.
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
            intersect::blocks(span, at)
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

#[cfg(test)]
mod tests {
    use super::*;

    /// **A unit square a hundred million out shuts in what it shuts in.**
    ///
    /// Twice the area of a unit square is 2 wherever it is drawn, and that is
    /// the whole of the hand computation. Taken about the origin the shoelace's
    /// terms are `10¹⁶`, whose places in the last are worth two apiece — so
    /// every term rounds to a multiple of two, the difference of one they were
    /// meant to show is lost, and the sum comes back as nought. An arrangement
    /// holds that against `ENCLOSED` and throws the face away for a sliver.
    ///
    /// **And the sign is the face-or-hole decision**, so the same square walked
    /// the other way has to come back at minus two rather than at nought
    /// either.
    #[test]
    fn a_square_far_from_the_origin_shuts_in_what_it_shuts_in() {
        const K: f64 = 1e8;
        let square = |at: DVec2| [at, at + DVec2::X, at + DVec2::ONE, at + DVec2::Y];
        let far = square(DVec2::splat(K));

        // Taken about the origin, which is what this used to be.
        let naive: f64 = far
            .iter()
            .enumerate()
            .map(|(at, here)| here.perp_dot(far[(at + 1) % far.len()]))
            .sum();
        assert_eq!(
            naive, 0.0,
            "the terms no longer round, so nothing is tested"
        );

        assert_eq!(swept(&far), 2.0, "a square out at {K} shut in nothing");
        assert_eq!(swept(&square(DVec2::ZERO)), 2.0);

        let mut backwards = far;
        backwards.reverse();
        assert_eq!(swept(&backwards), -2.0, "the hole did not read as one");

        // Nothing at all encloses nothing, which is the empty walk this used to
        // reach by not looping.
        let nowhere: [DVec2; 0] = [];
        assert_eq!(swept(&nowhere), 0.0);
    }
}
