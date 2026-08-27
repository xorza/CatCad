use crate::number::predicate::{ApproxEq, normalized, parallel, slack, square, touching, wraps};
use crate::number::tolerance::{EXACT, ROUNDING};
use glam::{DVec2, DVec3};
use std::f64::consts::TAU;

/// The boundary is closed on every type, and a tolerance of zero is exact
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

    let origin = DVec3::ZERO;
    assert!(origin.approx_eq(DVec3::new(1.0, 0.0, 0.0), 1.0), "closed");
    assert!(!origin.approx_eq(DVec3::new(1.0, 0.0, 0.0), 0.999));
    assert!(origin.approx_eq(origin, EXACT));
    assert!(!origin.approx_eq(DVec3::new(f64::EPSILON, 0.0, 0.0), EXACT));
}

/// The tolerance is a disc in two dimensions and a ball in three, not a box —
/// which is the whole reason this is not glam's `abs_diff_eq`.
#[test]
fn a_place_is_the_same_distance_away_every_way_round() {
    let flat = DVec2::ZERO;
    // Exactly one away along each axis, and on the boundary either way.
    assert!(flat.approx_eq(DVec2::new(1.0, 0.0), 1.0));
    assert!(flat.approx_eq(DVec2::new(0.0, 1.0), 1.0));

    // The diagonal at (0.8, 0.8) is 1.131 away, so a disc of radius 1 refuses
    // it where a box of half-width 1 would take it. Component-wise both
    // coordinates are inside; that is the bug this avoids.
    let diagonal = DVec2::splat(0.8);
    assert!(diagonal.abs_diff_eq(flat, 1.0), "glam takes it");
    assert!(!flat.approx_eq(diagonal, 1.0), "and a disc must not");
    // And it is taken once the radius reaches the diagonal's own length.
    assert!(flat.approx_eq(diagonal, 0.8 * std::f64::consts::SQRT_2));

    // (1,1,1) is √3 = 1.7320508 away, so the same argument one dimension up.
    let origin = DVec3::ZERO;
    let corner = DVec3::ONE;
    assert!(!origin.approx_eq(corner, 1.0), "a box, not a ball");
    assert!(!origin.approx_eq(corner, 1.732));
    assert!(origin.approx_eq(corner, 1.733));
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

/// A direction counts as normalized within a millionth of unit length, and
/// anything never normalized at all is nowhere near it.
///
/// Hand-computed on the squared length the test reads: `(3, 4, 0)` is five
/// long, so it misses by 24. A vector a ten-millionth long stands `2e-7` out in
/// the square and is taken, and one a thousandth long stands `2e-3` out and is
/// refused.
#[test]
fn a_direction_is_normalized_to_within_a_millionth() {
    assert!(normalized(DVec3::X));
    assert!(normalized(DVec3::new(1.0, 1.0, 1.0).normalize()));
    assert!(!normalized(DVec3::new(3.0, 4.0, 0.0)), "never normalized");
    assert!(!normalized(DVec3::ZERO));

    assert!(normalized(DVec3::X * (1.0 + 1e-7)));
    assert!(!normalized(DVec3::X * 1.001));
}

/// Two directions are parallel whichever end of each is taken, and a
/// quarter turn apart is not parallel at all.
#[test]
fn parallel_asks_about_the_line_rather_than_the_arrow() {
    assert!(parallel(DVec3::X, DVec3::X));
    assert!(parallel(DVec3::X, DVec3::NEG_X), "an axis has no near end");
    assert!(!parallel(DVec3::X, DVec3::Y));
    assert!(!parallel(DVec3::X, DVec3::Z));

    // A hair off, either side of the bound. The sine of a small angle is
    // the angle, so a tenth of the bound in radians is a tenth of it here.
    let leaning = DVec3::new(1.0, 1e-10, 0.0).normalize();
    assert!(parallel(DVec3::X, leaning));
    let further = DVec3::new(1.0, 1e-8, 0.0).normalize();
    assert!(!parallel(DVec3::X, further));
}

/// Two directions stand square whichever end of each is taken, and a pair
/// running the same way does not.
///
/// The mirror of the test above: what `parallel` takes, this refuses, and what
/// it refuses, this takes. A hair either side of the bound reads the same way,
/// because the cosine of an angle a hair off a right angle is that hair.
#[test]
fn square_asks_the_same_bound_from_the_other_end() {
    assert!(square(DVec3::X, DVec3::Y));
    assert!(square(DVec3::X, DVec3::NEG_Y), "an axis has no near end");
    assert!(!square(DVec3::X, DVec3::X));
    assert!(!square(DVec3::X, DVec3::NEG_X));

    let leaning = DVec3::new(1e-10, 1.0, 0.0).normalize();
    assert!(square(DVec3::X, leaning));
    let further = DVec3::new(1e-8, 1.0, 0.0).normalize();
    assert!(!square(DVec3::X, further));
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
