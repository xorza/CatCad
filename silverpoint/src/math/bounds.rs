//! The axis-aligned box a run of places fills.

use glam::{DVec2, DVec3};
use std::ops::{Add, Div, Sub};

/// A place a box can be taken over, read one axis at a time.
///
/// A satellite of [`Bounds`], and the whole of what a box asks of the place it
/// is drawn round: two ends to start inverted from, the two ways of taking a
/// place in, and the one comparison every question below is made of. Which is
/// what lets the two-dimensional box and the three-dimensional one be one box
/// rather than two spelt alike — the boolean draws one round a face in the
/// world and another round the same face in its own parameters, and the
/// arithmetic is the arithmetic either way.
pub(crate) trait Axial:
    Copy
    + Add<Self, Output = Self>
    + Add<f64, Output = Self>
    + Sub<f64, Output = Self>
    + Div<f64, Output = Self>
{
    /// Further out than any place on every axis, which an empty box's low end
    /// starts at.
    const HIGHEST: Self;
    /// Further back than any place on every axis, which its high end starts at.
    const LOWEST: Self;

    /// The nearer of the two on every axis.
    fn least(self, other: Self) -> Self;
    /// The further of the two on every axis.
    fn most(self, other: Self) -> Self;
    /// Whether this is at most `other` on every axis.
    fn under(self, other: Self) -> bool;
}

impl Axial for DVec2 {
    const HIGHEST: Self = Self::INFINITY;
    const LOWEST: Self = Self::NEG_INFINITY;

    fn least(self, other: Self) -> Self {
        self.min(other)
    }

    fn most(self, other: Self) -> Self {
        self.max(other)
    }

    fn under(self, other: Self) -> bool {
        self.cmple(other).all()
    }
}

impl Axial for DVec3 {
    const HIGHEST: Self = Self::INFINITY;
    const LOWEST: Self = Self::NEG_INFINITY;

    fn least(self, other: Self) -> Self {
        self.min(other)
    }

    fn most(self, other: Self) -> Self {
        self.max(other)
    }

    fn under(self, other: Self) -> bool {
        self.cmple(other).all()
    }
}

/// The smallest box holding everything put into it.
///
/// **What a boolean asks before it cuts.** A face is divided by a *surface* of
/// the other body, and a surface reaches well past the faces standing on it —
/// so a cut is taken along the whole of a crossing whether or not the other
/// body is anywhere near, which costs faces where the crossing can be carried
/// and costs a refusal where it cannot. A box apiece is what tells the two
/// apart cheaply enough to ask every time.
///
/// **Over places of either dimension**, because the boolean wants both of the
/// same body: a box round a face in the world says which surfaces reach it, and
/// a box round that face in its *own* parameters says which turn of a wrapping
/// angle it stands in and which runs come nowhere near it.
///
/// Held rather than measured wherever the places were walked for something else
/// anyway: a face's boundary is traced to be flattened, and folding four or six
/// floats out of that walk costs nothing beside it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Bounds<At: Axial> {
    pub(crate) low: At,
    pub(crate) high: At,
}

impl<At: Axial> Default for Bounds<At> {
    /// Nothing, and inverted rather than a point at the origin — which would be
    /// a claim to be there. The first place put in replaces both ends.
    fn default() -> Self {
        Self {
            low: At::HIGHEST,
            high: At::LOWEST,
        }
    }
}

impl<At: Axial> Bounds<At> {
    /// The box reaching `radius` from `middle` on every axis.
    pub(crate) fn about(middle: At, radius: f64) -> Self {
        Self {
            low: middle - radius,
            high: middle + radius,
        }
    }

    /// Take `at` in.
    pub(crate) fn hold(&mut self, at: At) {
        self.low = self.low.least(at);
        self.high = self.high.most(at);
    }

    /// Take the whole of `other` in.
    pub(crate) fn swallow(&mut self, other: Self) {
        self.low = self.low.least(other.low);
        self.high = self.high.most(other.high);
    }

    /// Whether it holds `at`.
    ///
    /// A box that holds nothing holds nowhere: the inverted ends make every
    /// comparison false, which is the answer wanted rather than an accident.
    pub(crate) fn holds(self, at: At) -> bool {
        self.low.under(at) && at.under(self.high)
    }

    /// The place half way between its two ends.
    pub(crate) fn middle(self) -> At {
        (self.low + self.high) / 2.0
    }

    /// The same box moved by `shift`.
    pub(crate) fn moved(self, shift: At) -> Self {
        Self {
            low: self.low + shift,
            high: self.high + shift,
        }
    }

    /// Whether the two come within `slack` of overlapping.
    ///
    /// Generous on purpose where a caller asks for slack, and in both of the
    /// ways it has to be. Two faces pressed flush against each other have boxes
    /// that touch exactly, and nothing may cull *that* pair — it is the one the
    /// operator's flush rule exists for. And a curved face's box is read off a
    /// boundary walked as chords, which fall inside the true edge by up to the
    /// sagitta they were walked at, so the box is that much too small before
    /// anything else is said. A caller comparing two boxes read off the same
    /// parameters has no chording between them to allow for and asks for none.
    ///
    /// A box that holds nothing meets nothing, for the reason [`Bounds::holds`]
    /// gives.
    pub(crate) fn meets(self, other: Self, slack: f64) -> bool {
        (self.low - slack).under(other.high) && (other.low - slack).under(self.high)
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

        // Held, which is meeting a box of no size at all — the edge counting
        // as inside, so a place on a face's own edge is on the face.
        assert!(here.holds(DVec3::ZERO));
        assert!(here.holds(DVec3::ONE));
        assert!(!here.holds(DVec3::new(1.5, 0.0, 0.0)));
        assert!(!Bounds::default().holds(DVec3::ZERO));
    }

    /// **The same box one dimension down**, which is the whole point of the
    /// trait beneath it: a face's own parameters are two numbers where the
    /// world is three, and the arithmetic must not be written twice.
    ///
    /// Hand-computed throughout. The three places below reach from `(-1, -2)`
    /// to `(1, 4)`, whose middle is `(0, 1)`. Moved by `(τ, 0)` the box holds
    /// `(τ, 1)` and no longer holds `(0, 1)`, which is the wrapping-parameter
    /// shift a marched run is carried by.
    #[test]
    fn a_flat_box_is_the_same_box_with_one_axis_fewer() {
        let mut grown = Bounds::default();
        for at in [
            DVec2::new(1.0, -2.0),
            DVec2::new(-1.0, 4.0),
            DVec2::new(0.0, 0.0),
        ] {
            grown.hold(at);
        }
        assert_eq!(grown.low, DVec2::new(-1.0, -2.0));
        assert_eq!(grown.high, DVec2::new(1.0, 4.0));
        assert_eq!(grown.middle(), DVec2::new(0.0, 1.0));
        assert!(grown.holds(DVec2::new(0.0, 1.0)));

        let shift = DVec2::new(std::f64::consts::TAU, 0.0);
        let moved = grown.moved(shift);
        assert_eq!(moved.low, DVec2::new(-1.0 + shift.x, -2.0));
        assert_eq!(moved.high, DVec2::new(1.0 + shift.x, 4.0));
        assert!(moved.holds(DVec2::new(shift.x, 1.0)));
        assert!(!moved.holds(DVec2::new(0.0, 1.0)));

        // A turn apart, so they meet on no slack and meet on enough of it.
        assert!(!grown.meets(moved, 0.0));
        assert!(grown.meets(moved, shift.x));

        let nothing = Bounds::default();
        assert!(!nothing.meets(grown, 1.0));
        assert!(!nothing.holds(DVec2::ZERO));
    }
}
