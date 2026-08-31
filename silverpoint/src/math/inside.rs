//! One place well within a region, found by laying a line across it.

use crate::math::intersect::{self, Span};
use crate::math::winding::Place;
use glam::DVec2;

/// Finds a place well within a region, keeping the room it works in.
///
/// Held across calls rather than stood up for each, like the cutter next door:
/// a boolean asks this of every region of every face it cut, and a document is
/// rebuilt on every frame of a drag through the drawing under it.
#[derive(Debug, Default)]
pub(crate) struct Inside {
    /// The levels of every corner of one region, sorted.
    levels: Vec<f64>,
    /// Where the line crosses its boundary, sorted.
    crossings: Vec<f64>,
}

impl Inside {
    /// A place well within the region `loops` bound — the outline first, then
    /// one loop per hole — or `None` where it covers nothing to be within.
    ///
    /// **The middle of the widest stretch one horizontal line cuts out of it**,
    /// which is inside it however the region bends and whatever holes are
    /// punched through it. Where the average of a region's corners is only
    /// inside one that happens to be convex, and a boolean makes plenty that
    /// are not.
    ///
    /// **The line is laid where no corner stands**, and that is what makes this
    /// exact rather than careful: the levels of the corners are sorted and the
    /// line goes half way across the widest gap between two of them, so every
    /// edge either straddles it cleanly or lies wholly to one side. There is no
    /// crossing through a corner to be counted twice or not at all, and no
    /// tolerance deciding which happened. The crossings then alternate in and
    /// out along the line, which is the same parity a ray cast reads — and
    /// each of them is [`intersect::rightward`], which is where a straight run
    /// is held against a level line everywhere in the crate.
    ///
    /// **Widest rather than any of them**, because a caller asking this asks it
    /// of a region that stands for itself: one place is sounded and its answer
    /// keeps or drops the whole region, so a place close to the boundary is a
    /// place the sounding may read as standing on it. The widest stretch is
    /// what puts it as far off as one line can.
    ///
    /// Linear in the corners but for the two sorts, where cutting the region
    /// into triangles to read the middle of the widest is quadratic in them.
    pub(crate) fn of<'a, At: Place + 'a>(
        &mut self,
        loops: impl Iterator<Item = &'a [At]> + Clone,
    ) -> Option<DVec2> {
        self.levels.clear();
        for walk in loops.clone() {
            self.levels
                .extend(walk.iter().map(|corner| corner.place().y));
        }
        self.levels.sort_by(f64::total_cmp);
        let level = self
            .levels
            .windows(2)
            .max_by(|one, two| (one[1] - one[0]).total_cmp(&(two[1] - two[0])))
            // A gap whose own middle rounds to an end of it is a gap no line
            // fits inside, and a corner *on* the line is a crossing counted
            // twice or not at all. Which is the whole of what is asked of the
            // levels, so it is asked of the answer rather than of the gap.
            .and_then(|gap| {
                let level = 0.5 * (gap[0] + gap[1]);
                (level > gap[0] && level < gap[1]).then_some(level)
            })?;

        self.crossings.clear();
        for walk in loops {
            for (step, corner) in walk.iter().enumerate() {
                let span = Span {
                    from: corner.place(),
                    to: walk[(step + 1) % walk.len()].place(),
                };
                if let Some(across) = intersect::rightward(span, level) {
                    self.crossings.push(across);
                }
            }
        }
        self.crossings.sort_by(f64::total_cmp);
        let widest = self
            .crossings
            .as_chunks::<2>()
            .0
            .iter()
            .max_by(|one, two| (one[1] - one[0]).total_cmp(&(two[1] - two[0])))
            .filter(|span| span[1] > span[0])?;
        Some(DVec2::new(0.5 * (widest[0] + widest[1]), level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A square from `(0, 0)` to `(4, 4)`, walked counterclockwise.
    fn square(low: f64, high: f64) -> [DVec2; 4] {
        [
            DVec2::new(low, low),
            DVec2::new(high, low),
            DVec2::new(high, high),
            DVec2::new(low, high),
        ]
    }

    /// **The middle of a square is its middle**, which is the whole of the
    /// hand computation.
    ///
    /// The corners stand at levels `0` and `4`, so the one gap between them is
    /// the widest and the line goes at `2`. It crosses the two upright edges at
    /// `0` and `4`, so the widest stretch is the whole width and its middle is
    /// `2`. Which is `(2, 2)`.
    ///
    /// **And a hole splits the line into two stretches**, of which the wider
    /// wins. The hole runs from `(1, 1)` to `(2, 3)`, so the corner levels are
    /// `0, 1, 3, 4` and the widest gap is `1..3`, putting the line at `2`.
    /// There the square gives crossings at `0` and `4` and the hole gives `1`
    /// and `2`, so the stretches are `0..1` and `2..4` and the second is twice
    /// the first. Its middle is `(3, 2)`.
    #[test]
    fn a_square_is_answered_at_its_middle_and_a_holed_one_beside_its_hole() {
        let mut inside = Inside::default();

        let whole = [square(0.0, 4.0)];
        assert_eq!(
            inside.of(whole.iter().map(|walk| walk.as_slice())),
            Some(DVec2::new(2.0, 2.0))
        );

        // Wound the same way as the outline, which nothing here reads: the
        // crossings alternate in and out whichever way each loop runs.
        let hole = [
            DVec2::new(1.0, 1.0),
            DVec2::new(2.0, 1.0),
            DVec2::new(2.0, 3.0),
            DVec2::new(1.0, 3.0),
        ];
        let holed: [&[DVec2]; 2] = [&square(0.0, 4.0), &hole];
        assert_eq!(
            inside.of(holed.into_iter()),
            Some(DVec2::new(3.0, 2.0)),
            "the narrower stretch beside the hole was taken"
        );
    }

    /// **A place the average of the corners would put outside**, which is the
    /// case the widest stretch exists for.
    ///
    /// A `U` `4` wide and `4` tall, with a notch cut down from the top between
    /// `x = 2` and `x = 3` as far as level `1`. Its corners stand at levels
    /// `0`, `1` and `4`, so the widest gap is `1..4` and the line goes at
    /// `2.5`. There the `U` is two arms, `0..2` and `3..4`, and the wider is
    /// the first — so the answer is `(1, 2.5)`.
    ///
    /// The average of the eight corners is `(2.25, 2.25)`, which is in the
    /// notch and outside the region altogether.
    #[test]
    fn a_notched_region_is_answered_inside_an_arm_and_not_in_the_notch() {
        let walk = [
            DVec2::new(0.0, 0.0),
            DVec2::new(4.0, 0.0),
            DVec2::new(4.0, 4.0),
            DVec2::new(3.0, 4.0),
            DVec2::new(3.0, 1.0),
            DVec2::new(2.0, 1.0),
            DVec2::new(2.0, 4.0),
            DVec2::new(0.0, 4.0),
        ];
        let mut inside = Inside::default();
        assert_eq!(
            inside.of([walk.as_slice()].into_iter()),
            Some(DVec2::new(1.0, 2.5))
        );
    }

    /// **Nothing to be within.** A region of no corners, one of two, and one
    /// whose corners all stand at one level all cover no area, and there is no
    /// place inside any of them to hand back.
    ///
    /// **And one two levels apart that no line fits between**, which is the
    /// case a refusal is the only honest answer to. The two levels below are
    /// neighbouring `f64`s, so the middle of the gap between them rounds back
    /// to the lower of the two — putting two corners *on* the line, where a
    /// crossing is counted twice or not at all and the parity is nonsense.
    #[test]
    fn a_region_covering_nothing_has_no_place_within_it() {
        let mut inside = Inside::default();

        let nowhere: [DVec2; 0] = [];
        assert_eq!(inside.of([nowhere.as_slice()].into_iter()), None);

        let edge = [DVec2::ZERO, DVec2::X];
        assert_eq!(inside.of([edge.as_slice()].into_iter()), None);

        let flat = [DVec2::ZERO, DVec2::X, DVec2::new(2.0, 0.0)];
        assert_eq!(inside.of([flat.as_slice()].into_iter()), None);

        let over = f64::from_bits(1.0f64.to_bits() + 1);
        assert_eq!(
            0.5 * (1.0 + over),
            1.0,
            "the two levels are no longer neighbours"
        );
        let thin = [
            DVec2::new(0.0, 1.0),
            DVec2::new(4.0, 1.0),
            DVec2::new(2.0, over),
        ];
        assert_eq!(inside.of([thin.as_slice()].into_iter()), None);
    }
}
