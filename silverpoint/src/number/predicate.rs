//! Every comparison the kernel makes, named.
//!
//! Named so that admitting a coincidence is a thing that happens in one place
//! and can be made to record itself. A routine that wrote `< 1e-9` would be
//! deciding the shape of a solid inside an expression, where the decision
//! cannot be seen, cannot be widened, and cannot be told from a rounding.

use crate::number::tolerance::{ROUNDING, WRAPPING};
use glam::DVec3;

/// Whether two places are the same place, to within `given`.
///
/// Euclidean rather than per-axis: a distance has no business depending on
/// which way it is measured. The same reasoning as
/// [`ApproxEq`](crate::math::approx::ApproxEq), which asks it of a sketch's
/// flat coordinates where this asks it of the world's.
pub(crate) fn coincident(a: DVec3, b: DVec3, given: f64) -> bool {
    debug_assert!(given >= 0.0, "a negative {given} admits nothing");
    // Squared, so the comparison costs no square root.
    a.distance_squared(b) <= given * given
}

/// Whether something `off` away from a curve or a surface counts as lying on
/// it, to within `given`.
///
/// `off` is a distance and so never negative — a caller holding a signed one
/// takes its magnitude before asking, because which side it fell on is a
/// different question from whether it is there at all.
pub(crate) fn touching(off: f64, given: f64) -> bool {
    debug_assert!(off >= 0.0, "{off} is signed, and this asks how far");
    debug_assert!(given >= 0.0, "a negative {given} admits nothing");
    off <= given
}

/// What an entity's tolerance comes to once the machine is allowed for.
///
/// Additive rather than a maximum, and the two are different questions. A
/// tolerance is what the *geometry* is known to; [`ROUNDING`] is what the
/// arithmetic reading it cannot promise away. A check that took the larger of
/// the two would be checking an exact entity against nothing but the rounding,
/// and one that took the tolerance alone would fail on arithmetic rather than
/// on geometry.
///
/// Only checks read this. Nothing is *constructed* to it, because there is no
/// decision here to record: two routes to one place answered the same answer.
pub(crate) fn slack(tolerance: f64) -> f64 {
    debug_assert!(tolerance >= 0.0, "a negative {tolerance} admits nothing");
    tolerance + ROUNDING
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
mod tests {
    use super::*;
    use crate::number::tolerance::{EXACT, ROUNDING};
    use std::f64::consts::TAU;

    /// The boundary is closed, the disc reaches equally far every way, and
    /// [`EXACT`] means exact rather than admitting nothing.
    #[test]
    fn coincidence_is_a_closed_disc_and_zero_means_exact() {
        let origin = DVec3::ZERO;
        assert!(coincident(origin, DVec3::new(1.0, 0.0, 0.0), 1.0), "closed");
        assert!(!coincident(origin, DVec3::new(1.0, 0.0, 0.0), 0.999));
        assert!(coincident(origin, origin, EXACT));
        assert!(!coincident(
            origin,
            DVec3::new(f64::EPSILON, 0.0, 0.0),
            EXACT
        ));

        // (1,1,1) is √3 = 1.7320508 away, so a disc of radius 1 refuses it
        // where a per-axis comparison of half-width 1 would take it.
        let diagonal = DVec3::ONE;
        assert!(!coincident(origin, diagonal, 1.0), "a box, not a disc");
        assert!(!coincident(origin, diagonal, 1.732));
        assert!(coincident(origin, diagonal, 1.733));
    }

    /// Slack is the tolerance plus the rounding, so an exact entity is still
    /// given the machine's own room and a loose one is not given it twice.
    #[test]
    fn slack_adds_the_rounding_rather_than_taking_the_larger() {
        assert_eq!(slack(EXACT), ROUNDING);
        assert_eq!(slack(ROUNDING), 2.0 * ROUNDING);
        assert_eq!(slack(1.0), 1.0 + ROUNDING);
        // Which is the whole difference from a maximum: that would have
        // answered `ROUNDING` for the middle case and checked nothing.
        assert!(slack(ROUNDING) > ROUNDING.max(ROUNDING));
    }

    /// A whole turn wraps whichever way it is turned; a hair under it does not.
    #[test]
    fn only_a_whole_turn_wraps() {
        assert!(wraps(TAU));
        assert!(wraps(-TAU), "the direction of a turn is not its size");
        assert!(!wraps(TAU * 0.75));
        assert!(!wraps(0.0));
        // A half turn either side of the boundary, which is what says the
        // boundary is where `WRAPPING` puts it and not at `TAU` itself.
        assert!(wraps(TAU - 1e-10));
        assert!(!wraps(TAU - 1e-8));
    }

    /// Distance to a surface is compared inclusively, and [`EXACT`] admits only
    /// what is genuinely on it.
    #[test]
    fn touching_is_inclusive_and_exact_admits_only_zero() {
        assert!(touching(1.0, 1.0));
        assert!(!touching(1.0, 0.999));
        assert!(touching(0.0, EXACT));
        assert!(!touching(f64::MIN_POSITIVE, EXACT));
    }
}
