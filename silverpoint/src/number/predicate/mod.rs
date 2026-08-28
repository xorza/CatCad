//! Every comparison the crate makes, named.
//!
//! Named so that admitting a coincidence is a thing that happens in one place
//! and can be made to record itself. A routine that wrote `< 1e-9` would be
//! deciding the shape of a solid inside an expression, where the decision
//! cannot be seen, cannot be widened, and cannot be told from a rounding.

use crate::number::tolerance::{ALIGNED, DRIFTING, ROUNDING, WRAPPING};
use glam::{DVec2, DVec3};

/// Equality to within a tolerance, for every kind of number geometry measures.
///
/// Geometry that a solve has settled is equal in the sense that matters and
/// unequal in the sense `==` tests: a coincidence is driven to a residual under
/// [`TOLERANCE`](crate::sketch::solver), not to identical bits, so anything
/// asking whether two points are in the same place has to say how close counts.
///
/// One trait over `f64`, [`DVec2`] and [`DVec3`], because it is one question
/// asked of a radius, of a place in a drawing and of a place in the world. An
/// extension trait rather than glam's own [`DVec2::abs_diff_eq`], for two
/// reasons that both matter here. `f64` has no such method at all — a radius is
/// as much a measurement as a position. And glam's is component-wise, which
/// makes the tolerance a *box* around the point and so admits a pair
/// √2 · epsilon apart on the diagonal while refusing one epsilon away along an
/// axis; a distance has no business depending on which way it is measured.
pub(crate) trait ApproxEq {
    /// Whether the two are the same to within `epsilon`.
    fn approx_eq(self, other: Self, epsilon: f64) -> bool;
}

impl ApproxEq for f64 {
    fn approx_eq(self, other: Self, epsilon: f64) -> bool {
        debug_assert!(epsilon >= 0.0, "a negative {epsilon} admits nothing");
        (self - other).abs() <= epsilon
    }
}

impl ApproxEq for DVec2 {
    fn approx_eq(self, other: Self, epsilon: f64) -> bool {
        debug_assert!(epsilon >= 0.0, "a negative {epsilon} admits nothing");
        // Squared, so the comparison costs no square root. Euclidean rather
        // than per-axis — see the note on the trait.
        self.distance_squared(other) <= epsilon * epsilon
    }
}

impl ApproxEq for DVec3 {
    fn approx_eq(self, other: Self, epsilon: f64) -> bool {
        debug_assert!(epsilon >= 0.0, "a negative {epsilon} admits nothing");
        self.distance_squared(other) <= epsilon * epsilon
    }
}

/// Whether something `off` away from a curve or a surface counts as lying on
/// it, to within `given`.
///
/// `off` is a distance and so never negative — a caller holding a signed one
/// takes its magnitude before asking, because which side it fell on is a
/// different question from whether it is there at all. A caller holding the two
/// numbers themselves rather than the distance between them asks
/// [`ApproxEq::approx_eq`] instead, which is the same question without the
/// subtraction written out at the call.
pub(crate) fn touching(off: f64, given: f64) -> bool {
    debug_assert!(off >= 0.0, "{off} is signed, and this asks how far");
    debug_assert!(given >= 0.0, "a negative {given} admits nothing");
    off <= given
}

/// What an entity's tolerance comes to once the machine is allowed for, over
/// values as large as `size`.
///
/// Additive rather than a maximum, and the two are different questions. A
/// tolerance is what the *geometry* is known to; the rest is what the
/// arithmetic reading it cannot promise away. A check that took the larger of
/// the two would be checking an exact entity against nothing but the rounding,
/// and one that took the tolerance alone would fail on arithmetic rather than
/// on geometry.
///
/// **The machine's share is two terms, because a rounding has two.**
/// [`ROUNDING`] is the floor, for a comparison whose values are small or
/// nought; [`DRIFTING`] is what a value of `size` carries of its own. With only
/// the first, a body drawn a hundred million units from the origin fails its
/// own validity check — one place in the last out there is already the wider of
/// the two, and it is the machine rather than the geometry.
///
/// `size` is how large the values being compared are, and the caller names it
/// because only the caller knows: a distance handed in has already lost the
/// magnitudes it came from.
///
/// **A check reads it, and so does a construction that has to beat one.**
/// Nothing is *built* to this width, there being no decision here to record:
/// two routes to one place answered the same answer. What a construction reads
/// it for is the other way round — `intersect::span_ring` asks how well it
/// could place a round crossing and pays the exact tier where the answer is
/// worse than this, a place no check can tell from the truth being a place
/// worth no more work.
pub(crate) fn slack(tolerance: f64, size: f64) -> f64 {
    debug_assert!(tolerance >= 0.0, "a negative {tolerance} admits nothing");
    debug_assert!(size >= 0.0, "a size is a magnitude, not {size}");
    tolerance + ROUNDING + size * DRIFTING
}

/// Whether `direction` has been normalized.
///
/// **Only an assert reads this**, here and where an axis checks its own frame,
/// and that is what sets the room it gives: slack enough for a direction
/// normalized once or twice, tight enough that anything never normalized at all
/// is nowhere near it. Nothing is constructed to it and nothing decides a shape
/// by it, which is why it is written here rather than among the tolerances the
/// geometry works to.
pub(crate) fn normalized(direction: DVec3) -> bool {
    const UNIT: f64 = 1e-6;
    direction.length_squared().approx_eq(1.0, UNIT)
}

/// Whether two unit directions run the same way, or exactly opposite ways.
///
/// Which is one question and not two: an axis has no preferred end, so a
/// surface asking whether a plane lies square across it means the same thing by
/// either answer. A caller that needs the *sense* as well takes the dot product
/// itself, and this is not what it wants.
pub(crate) fn parallel(one: DVec3, two: DVec3) -> bool {
    debug_assert!(
        normalized(one) && normalized(two),
        "{one:?} and {two:?} are compared as directions and are not unit",
    );
    // The sine of the angle between them, which is what [`ALIGNED`] bounds.
    one.cross(two).length() <= ALIGNED
}

/// Whether two unit directions stand square to each other.
///
/// The mirror of [`parallel`], bounded by the same [`ALIGNED`]: the dot product
/// of two unit directions is the cosine of the angle between them where the
/// cross product is the sine, so one number bounds both questions from their
/// own ends.
pub(crate) fn square(one: DVec3, two: DVec3) -> bool {
    debug_assert!(
        normalized(one) && normalized(two),
        "{one:?} and {two:?} are compared as directions and are not unit",
    );
    one.dot(two).abs() <= ALIGNED
}

/// Whether a turn of `sweep` radians carries a surface the whole way round.
///
/// The one question that decides whether a face has to be split — see
/// [`WRAPPING`]. A face that wrapped would walk one edge twice and read its
/// parameters two ways, which is the special case every loop walk in every
/// kernel pays for forever.
pub(crate) fn wraps(sweep: f64) -> bool {
    sweep.abs() >= WRAPPING
}

#[cfg(test)]
mod tests;
