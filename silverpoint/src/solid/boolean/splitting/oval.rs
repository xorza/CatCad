//! The closed cut a conic makes in a face's own parameters.

use crate::math::arc;
use crate::solid::boolean::splitting::cut::ROUNDED;
use glam::DVec2;

/// A closed cut round an ellipse, the inside kept where `inward`.
///
/// **Closed, which is the whole of what makes it a different case.** A straight
/// cut always meets a region's boundary if it meets the region at all; a closed
/// one can lie wholly within a region and take a disc out of its middle without
/// touching an edge of it, and it can be crossed twice by one straight run of
/// boundary. Both are states the straight arm never reaches — see `Cut::closed`
/// and `Cut::grazes`.
///
/// **An ellipse and not a circle, which costs one divide and buys a
/// milestone.** A circle is the pair of halves being equal, and everything here
/// reads them as a pair either way — where a second shape for the round case
/// would be eleven more arms saying nearly the same thing. A plane meeting a
/// cylinder squarely gives the circle, and obliquely the ellipse; two equal
/// cylinders on crossing axes give two of the second.
///
/// **A type rather than a variant's fields**, so that what only an ellipse can
/// answer is asked only of an ellipse. Written on the enum, each of the three
/// below had to answer a straight cut and a wave with a made-up number.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Oval {
    pub(crate) middle: DVec2,
    /// Unit, the way the longer half runs.
    pub(crate) along: DVec2,
    /// The two halves: `x` along [`Oval::along`] and `y` across it. Equal for a
    /// circle.
    pub(crate) half: DVec2,
    /// Whether the inside is kept rather than everything but it.
    pub(crate) inward: bool,
    /// Which of the caller's runs this is — see `Came::Arc`.
    ///
    /// **Bookkeeping on a piece of geometry, and it earns its place.** What a
    /// round cut puts down has to be marked with the curve it came from, and
    /// carried beside the cut instead it would be a second value threaded
    /// through six calls in step with the first — which is six chances to split
    /// by one cut and stamp another's number.
    pub(crate) run: u32,
}

impl Oval {
    /// `off` turned into the cut's own frame — along its longer half, and
    /// across it.
    ///
    /// A rotation and nothing else, so a length here is a length there. Takes a
    /// direction rather than a place, the middle being what tells the two apart
    /// and only some callers wanting it taken off.
    pub(crate) fn frame(self, off: DVec2) -> DVec2 {
        DVec2::new(off.dot(self.along), self.along.perp_dot(off))
    }

    /// How far `point` stands inside the ellipse, along the ray from its
    /// middle.
    ///
    /// Positive within. Reduces to `radius − |p − middle|` where the two halves
    /// are equal, which is what makes an ellipse the one shape here rather than
    /// a second one beside the circle.
    pub(crate) fn reach(self, point: DVec2) -> f64 {
        let off = self.frame(point - self.middle);
        let out = (off / self.half).length();
        if out < f64::MIN_POSITIVE {
            // The middle itself, where every ray is as good and the nearest
            // place on the ellipse is a shorter half away.
            return self.half.min_element();
        }
        off.length() * (1.0 / out - 1.0)
    }

    /// How many chords a stretch of `sweep` of the ellipse is worth.
    ///
    /// The longer half is what [`arc::chords`] reads as a radius, and it is
    /// exactly the bound wanted: `(a·cos t, b·sin t)` moves with a second
    /// derivative of `a` at most, and a circle's is its radius everywhere.
    /// Held to [`ROUNDED`] of that half, which is the classification tolerance
    /// the rest of the cut is chorded at.
    pub(crate) fn steps(self, sweep: f64) -> usize {
        arc::chords(self.half.x, sweep, self.half.x * ROUNDED)
    }

    /// The place `down` along the cut, which is `Cut::down` read backwards —
    /// see there for why both are written.
    pub(crate) fn at(self, down: f64) -> DVec2 {
        // `down` runs the way the cut does; counterclockwise keeps the inside
        // on the left, so the cut runs the other way round where it is not the
        // inside being kept.
        let (across, ahead) = if self.inward { down } else { -down }.sin_cos();
        self.middle
            + self.along * (self.half.x * ahead)
            + self.along.perp() * (self.half.y * across)
    }
}
