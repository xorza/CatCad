//! Comparison with room in it, for the two kinds of number a sketch measures.

use glam::DVec2;

/// How near two places have to be to count as one, in sketch units.
///
/// The tolerance the geometry works to wherever a coincidence has to be
/// admitted through a rounding rather than found exactly. A line that grazes a
/// circle meets it twice in the algebra and once on the page, and the two roots
/// come out a hair apart; a corner meant to land on another misses it by the
/// last bit of a division. Left alone, either becomes a vertex nobody drew and
/// a sliver of face between two edges that were meant to meet.
///
/// Here rather than with whichever routine reads it, because more than one
/// does: crossings are folded to it, an endpoint is allowed to reach that far
/// past a curve, a triangulation counts a corner this close to one of a
/// triangle's own as being that corner, and a cleanup counts two pieces of
/// geometry this close as the same piece.
///
/// An order of magnitude above the residual tolerance a solve converges to,
/// which is what makes the two tests in [`Sketch::remove_duplicates`] agree
/// instead of disagreeing at the boundary: a pair a solve has driven together
/// sits within the residual tolerance, so it reads as positionally equal here
/// too, and a coincidence and a measurement never answer differently about the
/// same pair.
///
/// Far below anything a pointer can distinguish — a click resolves to a
/// fraction of a sketch unit at any sane zoom — so nothing a user placed on
/// purpose is ever within it of something else.
///
/// [`Sketch::remove_duplicates`]: crate::Sketch::remove_duplicates
pub(crate) const TOUCHING: f64 = 1e-9;

/// How much area a loop has to enclose to be a region rather than a sliver, in
/// square sketch units.
///
/// The same number as [`TOUCHING`] and not the same quantity, which is the
/// whole reason it is written down separately: a tolerance on a length and a
/// tolerance on an area answer differently the moment a drawing is rescaled,
/// and one constant standing for both hides that. Nothing derives it from
/// `TOUCHING` — `TOUCHING` squared would be 1e-18, which admits every sliver
/// there is — so it is a measured bound in its own right and moves on its own.
///
/// Read against two scalings, deliberately. An arrangement compares it to a
/// true signed area; a triangulation compares it to `perp_dot`, which is twice
/// one, so the bound cleared there is half. They are different questions in
/// different algorithms — whether a loop is a face, and whether a corner can be
/// cut off — and neither is owed the other's boundary.
pub(crate) const SLIVER: f64 = 1e-9;

/// How near parallel two directions may run before the angle between them stops
/// meaning anything, as a sine.
///
/// Dimensionless, unlike the two around it, and again the same number by
/// measurement rather than by derivation.
pub(crate) const PARALLEL: f64 = 1e-9;

/// How long a difference of two points has to be before a direction can be
/// recovered from it, in sketch units.
///
/// Not a coincidence tolerance: two points this close are the same *place* by
/// [`TOUCHING`] three decades earlier, and this asks a narrower question about
/// what is left — whether normalizing the difference between them still says
/// anything. Below it the quotient is noise, so whoever asked picks a direction
/// instead: an intersection answers that the curve is a point, and a
/// constraint's derivative pushes along +x, because any direction will do when
/// the solver only needs somewhere to push.
///
/// Here rather than with either reader, because there are two and they had a
/// constant apiece — the same name, the same value and the same paragraph,
/// stated twice in modules that never mention each other.
pub(crate) const NO_DIRECTION: f64 = 1e-12;

/// Equality to within a tolerance.
///
/// Geometry that a solve has settled is equal in the sense that matters and
/// unequal in the sense `==` tests: a coincidence is driven to a residual under
/// [`TOLERANCE`](crate::sketch::solver), not to identical bits, so anything
/// asking whether two points are in the same place has to say how close counts.
///
/// An extension trait rather than glam's own [`DVec2::abs_diff_eq`], for two
/// reasons that both matter here. `f64` has no such method at all — a radius is
/// as much a measurement as a position. And glam's is component-wise, which
/// makes the tolerance a *square* around the point and so admits a pair
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

#[cfg(test)]
mod tests {
    use super::*;

    /// The boundary is closed on both types, and a tolerance of zero is exact
    /// equality rather than a comparison that admits nothing.
    #[test]
    fn the_tolerance_is_inclusive_and_zero_means_exact() {
        assert!(1.0_f64.approx_eq(1.5, 0.5), "the boundary is out");
        assert!(!1.0_f64.approx_eq(1.5, 0.499));
        assert!(1.0_f64.approx_eq(1.0, 0.0));
        assert!(!1.0_f64.approx_eq(1.0 + f64::EPSILON, 0.0));

        // Sign is the difference's, not the operands': -1 and 1 are two apart
        // whichever way round they are asked.
        assert!((-1.0_f64).approx_eq(1.0, 2.0));
        assert!(!(-1.0_f64).approx_eq(1.0, 1.999));
        assert!(1.0_f64.approx_eq(-1.0, 2.0));
    }

    /// The vector tolerance is a disc, not a square — which is the whole reason
    /// this is not glam's `abs_diff_eq`.
    #[test]
    fn a_vectors_tolerance_reaches_the_same_distance_every_way() {
        let origin = DVec2::ZERO;
        // Exactly one away along each axis, and on the boundary either way.
        assert!(origin.approx_eq(DVec2::new(1.0, 0.0), 1.0));
        assert!(origin.approx_eq(DVec2::new(0.0, 1.0), 1.0));

        // The diagonal at (0.8, 0.8) is 1.131 away, so a disc of radius 1
        // refuses it where a square of half-width 1 would take it. Component-
        // wise both coordinates are inside; that is the bug this avoids.
        let diagonal = DVec2::splat(0.8);
        assert!(diagonal.abs_diff_eq(origin, 1.0), "glam takes it");
        assert!(!origin.approx_eq(diagonal, 1.0), "and a disc must not");
        // And it is taken once the radius reaches the diagonal's own length.
        assert!(origin.approx_eq(diagonal, 0.8 * std::f64::consts::SQRT_2));
    }
}
