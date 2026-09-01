//! The two places a straight run dips across a cut and back.

use crate::number::tolerance::PLACED;
use glam::DVec2;

/// Up to two places a straight run crosses a cut at, gathered a chord at a
/// time.
///
/// **What the two cuts with no closed form to solve against share.** A marched
/// cut and a flare each answer where a run dips across them by walking their
/// own chords and holding the run against every one — see
/// [`Traced::grazes`](super::traced::Traced) and
/// [`Flare::grazes`](super::flare::Flare), which differ in the chords they lay
/// and in what each fences out, and in nothing else. What is here is the rule
/// left over, which has nothing to do with either shape.
///
/// **Deduplicated within [`PLACED`] against the last one taken**, and against
/// that one alone: a run through a corner of the chords is met by both of the
/// chords meeting there, ends counting for a crossing — see
/// [`intersect::spans`](crate::math::intersect::spans) — and those two arrive
/// one after the other, the chords being walked in order.
///
/// **Refused past two rather than truncated.** A run meeting the chords three
/// times crossed the cut rather than dipping over it and back, and the first
/// two of those would answer a question nobody asked.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct Dipped {
    at: [DVec2; 2],
    /// How many crossings arrived, which runs past the two there is room for so
    /// that a third is a refusal rather than a place written over.
    held: usize,
}

impl Dipped {
    /// Take the crossing at `at` in.
    pub(super) fn hold(&mut self, at: DVec2) {
        let kept = self.held.min(self.at.len());
        if kept > 0 && at.distance(self.at[kept - 1]) <= PLACED {
            return;
        }
        if self.held < self.at.len() {
            self.at[self.held] = at;
        }
        self.held += 1;
    }

    /// The two, or `None` where the run met the cut any other number of times.
    pub(super) fn both(self) -> Option<[DVec2; 2]> {
        (self.held == self.at.len()).then_some(self.at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **Two and only two, and a repeat is not a second.**
    ///
    /// The three rules, each shown on its own. Two places a hair over
    /// [`PLACED`] apart are two; the second of them arriving again is the one
    /// corner of the chords met twice and folds away; a third place is a run
    /// that crossed rather than dipped and is refused outright.
    ///
    /// **Against the last taken and not against both**, which is the rule that
    /// would be silently loosened by a walk over the pair: the first place
    /// arriving again after the second is a third crossing, and it is refused
    /// exactly as any other third would be.
    #[test]
    fn two_are_a_dip_and_a_third_is_a_crossing() {
        let (first, second) = (DVec2::new(1.0, 0.0), DVec2::new(2.0, 0.0));
        let held = |places: &[DVec2]| {
            let mut dipped = Dipped::default();
            for &at in places {
                dipped.hold(at);
            }
            dipped.both()
        };

        assert_eq!(held(&[first, second]), Some([first, second]));
        assert_eq!(held(&[]), None, "nothing is no dip");
        assert_eq!(held(&[first]), None, "one crossing is an arrival");

        // The corner of two chords, met by each of them: within `PLACED` of the
        // one before it, so the second reading is the first place again.
        let again = second + DVec2::new(PLACED / 2.0, 0.0);
        assert_eq!(held(&[first, second, again]), Some([first, second]));
        assert_eq!(held(&[first, again, second]), Some([first, again]));

        assert_eq!(held(&[first, second, first]), None, "a third crossing");
        assert_eq!(
            held(&[first, second, first, second]),
            None,
            "a refusal is not walked back",
        );
    }
}
