//! The axis-aligned box a run of places fills.

use glam::DVec3;

/// The smallest box holding everything put into it.
///
/// **What a boolean asks before it cuts.** A face is divided by a *surface* of
/// the other body, and a surface reaches well past the faces standing on it —
/// so a cut is taken along the whole of a crossing whether or not the other
/// body is anywhere near, which costs faces where the crossing can be carried
/// and costs a refusal where it cannot. A box apiece is what tells the two
/// apart cheaply enough to ask every time.
///
/// Held rather than measured wherever the places were walked for something else
/// anyway: a face's boundary is traced to be flattened, and folding six floats
/// out of that walk costs nothing beside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bounds {
    pub(crate) low: DVec3,
    pub(crate) high: DVec3,
}

impl Default for Bounds {
    /// Nothing, and inverted rather than a point at the origin — which would be
    /// a claim to be there. The first place put in replaces both ends.
    fn default() -> Self {
        Self {
            low: DVec3::INFINITY,
            high: DVec3::NEG_INFINITY,
        }
    }
}

impl Bounds {
    /// The box reaching `radius` from `middle` on every axis.
    pub(crate) fn about(middle: DVec3, radius: f64) -> Self {
        Self {
            low: middle - radius,
            high: middle + radius,
        }
    }

    /// Take `at` in.
    pub(crate) fn hold(&mut self, at: DVec3) {
        self.low = self.low.min(at);
        self.high = self.high.max(at);
    }

    /// Take the whole of `other` in.
    pub(crate) fn swallow(&mut self, other: Self) {
        self.low = self.low.min(other.low);
        self.high = self.high.max(other.high);
    }

    /// Whether the two come within `slack` of overlapping.
    ///
    /// Generous on purpose, and in both of the ways it has to be. Two faces
    /// pressed flush against each other have boxes that touch exactly, and
    /// nothing may cull *that* pair — it is the one the operator's flush rule
    /// exists for. And a curved face's box is read off a boundary walked as
    /// chords, which fall inside the true edge by up to the sagitta they were
    /// walked at, so the box is that much too small before anything else is
    /// said.
    ///
    /// A box that holds nothing meets nothing: the inverted ends make every
    /// comparison false, which is the answer wanted rather than an accident.
    pub(crate) fn meets(self, other: Self, slack: f64) -> bool {
        (self.low - slack).cmple(other.high).all() && (other.low - slack).cmple(self.high).all()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// **A box holds what it was given, and meets what it overlaps** — with
    /// the three edge cases the boolean's cull rests on.
    ///
    /// Every figure hand-written: a unit box about the origin and one about
    /// `(3, 0, 0)` stand two apart on x, so a slack of one leaves them apart
    /// and a slack of two brings them together. Which is the whole of what the
    /// slack is for: a curved face's box comes off chords that fall inside the
    /// true edge, and two faces pressed flush have boxes that touch exactly.
    #[test]
    fn a_box_holds_what_it_was_given_and_meets_what_it_overlaps() {
        let mut grown = Bounds::default();
        for at in [
            DVec3::new(1.0, -2.0, 3.0),
            DVec3::new(-1.0, 4.0, 0.0),
            DVec3::new(0.0, 0.0, 5.0),
        ] {
            grown.hold(at);
        }
        assert_eq!(grown.low, DVec3::new(-1.0, -2.0, 0.0));
        assert_eq!(grown.high, DVec3::new(1.0, 4.0, 5.0));

        // **Nothing meets nothing**, which the inverted ends give for free and
        // which the cull reads as "this face reaches nowhere".
        let nothing = Bounds::default();
        assert!(!nothing.meets(grown, 1.0));
        assert!(!grown.meets(nothing, 1.0));

        // Two apart on x, and nowhere else.
        let here = Bounds::about(DVec3::ZERO, 1.0);
        let there = Bounds::about(DVec3::new(3.0, 0.0, 0.0), 1.0);
        assert!(!here.meets(there, 0.0));
        assert!(!here.meets(there, 0.9));
        assert!(here.meets(there, 1.1));
        // And whichever way round it is asked.
        assert!(there.meets(here, 1.1));

        // Touching exactly is meeting, with no slack at all — the pair a flush
        // cut must never be culled on.
        let against = Bounds::about(DVec3::new(2.0, 0.0, 0.0), 1.0);
        assert!(here.meets(against, 0.0));

        // Swallowed, the two reach as far as both.
        let mut both = here;
        both.swallow(there);
        assert_eq!(both.low, DVec3::new(-1.0, -1.0, -1.0));
        assert_eq!(both.high, DVec3::new(4.0, 1.0, 1.0));
        assert!(both.meets(Bounds::about(DVec3::new(3.5, 0.0, 0.0), 0.1), 0.0));
    }
}
