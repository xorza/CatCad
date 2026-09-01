use super::*;
use crate::solid::geometry::axis::Axis;
use std::f64::consts::PI;

/// The corner of `.notes/KERNEL.md` §9.6, at the origin and square.
///
/// The floor is `z = 0` with material below, the riser `x = 0` with material to
/// the left of it, and the wall `y = 0` with material behind. The concave edge
/// runs along `y` and the convex one along `x`, and the reach is one.
///
/// So the fillet's centres run down `x = 1, z = 1` and the round's down
/// `y = 1, z = −1`, the two standing two reaches apart across the floor.
///
/// **The branch is worked out here rather than read off the patch**, so that
/// the test asserting where the first ruling lands can fail. That ruling lies
/// in the fillet's tangent plane at `(0, 0, 1)`, which is the riser's own plane.
/// From there the round's axis stands `(0, 1, −2)` away, already square to it,
/// so the tangent reaches `√(5 − 1) = 2` and the two directions come to
/// `[−(0, 1, −2) ± 2·(0, 2, 1)]/5` — which is `(0, 0.6, 0.8)` one way and
/// `(0, −1, 0)` the other. Only the second lands on the round's rail at
/// `(0, 0, −1)`, and it is the one [`Gusset::turning`] calls false.
fn square() -> Gusset {
    Gusset::new(
        Cylinder {
            axis: Axis::new(DVec3::new(1.0, 0.0, 1.0), DVec3::Y, DVec3::X),
            radius: 1.0,
        },
        Cylinder {
            axis: Axis::new(DVec3::new(0.0, 1.0, -1.0), DVec3::X, DVec3::Y),
            radius: 1.0,
        },
        DVec3::new(0.0, 0.0, 1.0),
        false,
    )
}

/// Where the fillet's angle stands at each end of the patch's own turn.
///
/// The first edge starts at `(0, 0, 1)`, a whole half turn round from the
/// fillet's reference, and the tip stands at `(1, 1, 0)`, a quarter turn from
/// it — so the patch covers the quarter between them.
const TURN: [f64; 2] = [PI, FRAC_PI_2];

/// Which way `of` faces where it carries `at`.
fn facing(of: Cylinder, at: DVec3) -> DVec3 {
    of.normal(of.uv(at))
}

#[test]
fn the_two_blends_touch_where_their_axes_stand_nearest() {
    let gusset = square();
    // The two axes are skew along `z`: the fillet's runs at `z = 1` and the
    // round's at `z = −1`, so their common perpendicular is the segment from
    // (1, 1, 1) to (1, 1, −1) and its middle is (1, 1, 0).
    assert!(
        gusset.met().abs_diff_eq(DVec3::new(1.0, 1.0, 0.0), 1e-12),
        "{} is not where the two axes stand nearest",
        gusset.met(),
    );
    // And that middle stands one reach off each of them, which is what says the
    // two cylinders touch there rather than cross.
    assert!(
        (gusset.filled.axis.off(gusset.met()) - 1.0).abs() < 1e-12,
        "the touch point is off the fillet",
    );
    assert!(
        (gusset.cut.axis.off(gusset.met()) - 1.0).abs() < 1e-12,
        "the touch point is off the round",
    );
}

#[test]
fn every_ruling_leaves_one_blend_and_lands_on_the_other() {
    let gusset = square();
    for step in 0..40 {
        let angle = TURN[0] + (TURN[1] - TURN[0]) * f64::from(step) / 40.0;
        let head = gusset.at(DVec2::new(angle, 0.0));
        let foot = gusset.at(DVec2::new(angle, 1.0));
        assert!(
            (gusset.filled.axis.off(head) - 1.0).abs() < 1e-12,
            "the ruling at {angle} leaves {head}, which is not on the fillet",
        );
        assert!(
            (gusset.cut.axis.off(foot) - 1.0).abs() < 1e-12,
            "the ruling at {angle} lands at {foot}, which is not on the round",
        );
        // And it lies in both tangent planes, which is the whole of the
        // construction: the join to each blend is exact rather than fitted.
        let along = foot - head;
        assert!(
            along.dot(facing(gusset.filled, head)).abs() < 1e-12
                && along.dot(facing(gusset.cut, foot)).abs() < 1e-12,
            "the ruling at {angle} leans out of a tangent plane",
        );
    }
}

#[test]
fn the_first_ruling_lands_where_the_round_runs_out() {
    // The far ruling lies in the fillet's tangent plane at (0, 0, 1), which is
    // the riser's own plane `x = 0`; the only line in it tangent to the round
    // touches at (0, 0, −1), the round's ruling on the wall. So the gap's third
    // corner falls out of the construction rather than being a second choice —
    // `.notes/KERNEL.md` §9.6.
    let gusset = square();
    let landed = gusset.at(DVec2::new(TURN[0], 1.0));
    assert!(
        landed.abs_diff_eq(DVec3::new(0.0, 0.0, -1.0), 1e-12),
        "the first ruling lands at {landed} rather than on the round's rail",
    );
}

#[test]
fn the_patch_closes_to_a_point_at_the_touch_point() {
    // **Linearly, which is the claim rather than a bound**: halving the angle
    // left to the tip halves the ruling, so the patch closes to a point at a
    // proper corner rather than running out to a cusp or a side. The first two
    // steps are not yet on that line and are asserted only to shrink.
    let gusset = square();
    let met = gusset.met();
    let mut spans = Vec::new();
    for step in 1..=8 {
        let angle = TURN[1] + (TURN[0] - TURN[1]) / f64::from(1 << step);
        let head = gusset.at(DVec2::new(angle, 0.0));
        let foot = gusset.at(DVec2::new(angle, 1.0));
        assert!(
            head.distance(met) < 2.0f64.powi(1 - step)
                && foot.distance(met) < 2.0f64.powi(1 - step),
            "the ruling at {angle} does not close on the touch point",
        );
        spans.push(head.distance(foot));
    }
    for pair in spans.windows(2) {
        assert!(pair[1] < pair[0], "the ruling widens toward the tip");
    }
    for pair in spans[2..].windows(2) {
        let rate = pair[0] / pair[1];
        assert!(
            (rate - 2.0).abs() < 0.05,
            "the ruling shrinks by {rate} where halving the angle should halve it",
        );
    }
}

#[test]
fn a_place_on_the_patch_reads_back_the_parameters_it_was_made_from() {
    let gusset = square();
    for down in 0..7 {
        for across in 0..=4 {
            let angle = TURN[0] + (TURN[1] - TURN[0]) * f64::from(down) / 8.0;
            let uv = DVec2::new(angle, f64::from(across) / 4.0);
            let read = gusset.uv(gusset.at(uv));
            assert!(
                (read.x - uv.x).abs() < 1e-9 && (read.y - uv.y).abs() < 1e-9,
                "{uv} was read back as {read}",
            );
        }
    }
}

#[test]
fn each_edge_faces_the_way_the_blend_it_lies_on_does() {
    // Which is the tangency itself, and the one thing the patch is for: at
    // `v = 0` the surface faces the way the fillet does and at `v = 1` the way
    // the round does, so neither join is a crease.
    let gusset = square();
    for step in 1..8 {
        let angle = TURN[0] + (TURN[1] - TURN[0]) * f64::from(step) / 8.0;
        let one = gusset.normal(DVec2::new(angle, 0.0));
        let two = gusset.normal(DVec2::new(angle, 1.0));
        let fillet = facing(gusset.filled, gusset.at(DVec2::new(angle, 0.0)));
        let round = facing(gusset.cut, gusset.at(DVec2::new(angle, 1.0)));
        assert!(
            one.cross(fillet).length() < 1e-9,
            "at {angle} the patch faces {one} where the fillet faces {fillet}",
        );
        assert!(
            two.cross(round).length() < 1e-9,
            "at {angle} the patch faces {two} where the round faces {round}",
        );
    }
}

#[test]
fn a_corner_that_leans_is_a_different_patch() {
    // The riser tipped twenty degrees moves the fillet's spine and nothing
    // else, and every place of the patch moves with it — which is what says the
    // surface reads its blends rather than a shape of its own.
    let square = square();
    let lean: f64 = 20f64.to_radians();
    // Tangent to the floor at `z = 1` still, and to the tipped riser at
    // `(1 − sin α)/cos α` out — which is one where the riser stands up.
    let axis = DVec3::new((1.0 - lean.sin()) / lean.cos(), 0.0, 1.0);
    let filled = Cylinder {
        axis: Axis::new(axis, DVec3::Y, DVec3::X),
        radius: 1.0,
    };
    // And its rail on the riser moves with it, a reach along the riser's own
    // normal from the axis.
    let from = axis - DVec3::new(lean.cos(), 0.0, lean.sin());
    let leaning = Gusset::new(filled, square.cut, from, square.turning);
    assert!(
        leaning.met().distance(square.met()) > 0.1,
        "leaning the riser left the touch point where it was",
    );
    let uv = DVec2::new(TURN[0] + (TURN[1] - TURN[0]) / 2.0, 0.5);
    assert!(
        leaning.at(uv).distance(square.at(uv)) > 0.05,
        "leaning the riser left the middle of the patch where it was",
    );
}
