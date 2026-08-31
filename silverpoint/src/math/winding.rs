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
/// it is on one asks [`within`], which answers both for one walk.
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

/// Where `at` stands in relation to the closed run `walk`.
///
/// A satellite of [`within`], which is the only thing that makes one.
#[derive(Debug)]
pub(crate) struct Within {
    /// How far `at` stands from the run itself.
    ///
    /// What tells a place safely within a region from one sitting on its edge,
    /// which is the question a ray cast has to ask before it trusts its own
    /// count: a ray through a corner is counted twice or not at all, and either
    /// way the answer is not the one wanted.
    ///
    /// Zero for a run with nothing in it, because there is nothing to be far
    /// from.
    pub(crate) off: f64,
    /// Whether the run holds `at`, on [`holds`]' terms.
    pub(crate) holds: bool,
}

/// Read `walk` for both of [`Within`]'s answers, over one walk of its corners.
///
/// **For the caller that wants the pair**, which is any ray cast that has to
/// know it did not graze what it counted: asking the two separately steps the
/// same corners twice. A sounding asks this per loop, per crossing, per ray,
/// per region of a rebuild. A caller that wants only the parity asks [`holds`],
/// which is spared the distance it would not read.
///
/// The run is closed whether or not its last corner repeats its first — the
/// walk wraps — so a caller need not decide which convention it is using.
pub(crate) fn within(walk: &[impl Place], at: DVec2) -> Within {
    let mut nearest = f64::INFINITY;
    let mut crossings = 0;
    for (step, &from) in walk.iter().enumerate() {
        let from = from.place();
        let to = walk[(step + 1) % walk.len()].place();
        crossings += usize::from(intersect::blocks(Span { from, to }, at));
        let along = to - from;
        let reach = along.length_squared();
        // A run may double back on a corner; there is no direction in that and
        // the corner itself is as near as anything on it.
        let nearby = if reach < NO_DIRECTION * NO_DIRECTION {
            from
        } else {
            from.lerp(to, ((at - from).dot(along) / reach).clamp(0.0, 1.0))
        };
        // The nearest squared is nearest, and rooting here would be a root per
        // corner of every loop this is asked about.
        nearest = nearest.min(nearby.distance_squared(at));
    }
    Within {
        off: if nearest.is_finite() {
            nearest.sqrt()
        } else {
            0.0
        },
        holds: crossings % 2 == 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::SQRT_2;

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

    /// **Both readings of one walk, against distances worked out by hand.**
    ///
    /// The run is the doubled unit square `(0,0) (2,0) (2,2) (0,2)`, whose
    /// edges are four whole-number lines. So the middle stands `1` from each of
    /// them, a place one out to the right stands `1` from the near edge, and a
    /// place diagonally off a corner stands `√2` from that corner — which is
    /// the arm the squared minimum has to root exactly rather than
    /// approximately.
    ///
    /// **The parity does not care which way the walk runs**, where the sweep
    /// beside it is the sign that does, so the same square reversed holds the
    /// same places.
    ///
    /// **And the parity here is [`holds`]' parity**, which is what lets that
    /// one stay the spelling for a caller that never reads a distance: the two
    /// count the same crossings, so a grid stepped across the square and out
    /// past it has to answer alike at every place.
    #[test]
    fn a_square_says_how_far_off_it_is_and_what_it_holds() {
        const SQUARE: [DVec2; 4] = [
            DVec2::new(0.0, 0.0),
            DVec2::new(2.0, 0.0),
            DVec2::new(2.0, 2.0),
            DVec2::new(0.0, 2.0),
        ];
        let mut backwards = SQUARE;
        backwards.reverse();

        for walk in [SQUARE, backwards] {
            let middle = within(&walk, DVec2::new(1.0, 1.0));
            assert_eq!(middle.off, 1.0);
            assert!(middle.holds);

            let beside = within(&walk, DVec2::new(3.0, 1.0));
            assert_eq!(beside.off, 1.0);
            assert!(!beside.holds);

            // Off a corner rather than an edge, where the nearest place on the
            // run is the corner itself and the arm is the diagonal.
            let cornered = within(&walk, DVec2::new(-1.0, -1.0));
            assert_eq!(cornered.off, SQRT_2);
            assert!(!cornered.holds);

            // On the run, which is the reading a ray cast refuses to count
            // from — an edge and a corner alike.
            assert_eq!(within(&walk, DVec2::new(1.0, 0.0)).off, 0.0);
            assert_eq!(within(&walk, DVec2::new(2.0, 2.0)).off, 0.0);

            // Halfway between the whole numbers, so no place of the grid lands
            // on the run and every one of them has an answer.
            for x in -2..8 {
                for y in -2..8 {
                    let at = DVec2::new(x as f64, y as f64) / 2.0 + DVec2::splat(0.25);
                    assert_eq!(
                        within(&walk, at).holds,
                        holds(&walk, at),
                        "{at:?} is held by one reading and not the other"
                    );
                }
            }
        }

        // Nothing to be far from and nothing to be held by.
        let nowhere: [DVec2; 0] = [];
        let empty = within(&nowhere, DVec2::ZERO);
        assert_eq!(empty.off, 0.0);
        assert!(!empty.holds);
    }
}
