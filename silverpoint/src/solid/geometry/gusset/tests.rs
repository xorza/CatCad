use super::*;
use crate::number::tolerance::ALIGNED;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::marchings::Marchings;
use std::f64::consts::PI;

/// The corner of `.notes/KERNEL.md` §7.7, at the origin and square.
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
    // `.notes/KERNEL.md` §7.7.
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

/// [`square`] with the riser tipped twenty degrees.
///
/// The fillet's spine moves and nothing else does: the round still runs down
/// `y = 1, z = −1` and the floor is still `z = 0`.
fn leaning() -> Gusset {
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
    Gusset::new(filled, square.cut, from, square.turning)
}

/// [`square`] with the fillet's frame turned a fifth of a radian backwards.
///
/// The same patch in the same place: only where the fillet's angles are counted
/// from moves, which is a free choice of the axis and no part of the surface.
/// What it buys is a patch whose turn runs *past* the half turn an inversion
/// answers in — its bounds are `[−2.94, −4.51]` where `Axis::angle_of` answers
/// in `(−π, π]` — so anything reading a place back against those bounds has to
/// carry the branch rather than trust it.
fn framed() -> Gusset {
    let square = square();
    let turn: f64 = -0.2;
    let filled = Cylinder {
        axis: Axis::new(
            square.filled.axis.origin,
            square.filled.axis.direction,
            DVec3::new(turn.cos(), 0.0, -turn.sin()),
        ),
        radius: square.filled.radius,
    };
    Gusset::new(filled, square.cut, square.from, square.turning)
}

/// Three places clear of both blends, to cast from.
const CAST: [DVec3; 3] = [
    DVec3::new(5.0, -4.0, 3.0),
    DVec3::new(-6.0, 2.0, 7.0),
    DVec3::new(2.0, 9.0, -5.0),
];

#[test]
fn a_corner_that_leans_is_a_different_patch() {
    // Every place of the patch moves with the fillet's spine, which is what
    // says the surface reads its blends rather than a shape of its own.
    let square = square();
    let leaning = leaning();
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

/// **A ray aimed at a place on the patch is answered exactly there.**
///
/// Swept over the whole patch — nine turns of the fillet's angle by four runs
/// along the ruling — and cast from three places clear of both blends, so the
/// ray leans a different way at every one of them. The place is `way` from
/// where the ray starts, so the crossing it owes reads one.
///
/// **Inside the patch and not on its edges**, which are boundary places the
/// loops bounding a face decide rather than the surface — see
/// [`Gusset::met_by`], where that is stated.
///
/// Over the square corner and the leaning one both, which is what says the
/// solve reads the blends it was handed rather than a shape of its own.
#[test]
fn a_ray_aimed_at_the_patch_is_answered_where_it_lands() {
    for (what, gusset) in [("square", square()), ("leaning", leaning())] {
        for down in 0..=8 {
            for across in 1..=4 {
                let angle = TURN[0] + (TURN[1] - TURN[0]) * f64::from(down) / 9.0;
                let uv = DVec2::new(angle, f64::from(across) / 5.0);
                let at = gusset.at(uv);
                for from in CAST {
                    let met = gusset.met_by(from, at - from);
                    assert!(
                        met.all().iter().any(|along| (along - 1.0).abs() < 1e-7),
                        "{what}: {uv} was not answered from {from}, {:?} came back",
                        met.all(),
                    );
                }
            }
        }
    }
}

/// **Every crossing answered stands on the patch**, read back through its own
/// inversion — and they come back in order.
///
/// The other half of the sweep above: that one says nothing is missed, and this
/// one says nothing is invented. A ray answering a place the inversion does not
/// put back where it was would be a root of the harmonic that belongs to the
/// other tangent, or a place past the end of a ruling.
///
/// **To a ten-thousandth, which is the inversion's own conditioning and not the
/// solve's.** Reading a place back wants `acos` of a ratio that reaches one at
/// the edge on the fillet, so an error in the place comes out square-rooted in
/// the angle — a crossing good to a hundred-millionth reads back to about a
/// ten-thousandth near that edge.
#[test]
fn every_crossing_answered_stands_on_the_patch() {
    let gusset = square();
    for down in 0..=8 {
        for across in 1..=4 {
            let angle = TURN[0] + (TURN[1] - TURN[0]) * f64::from(down) / 9.0;
            let at = gusset.at(DVec2::new(angle, f64::from(across) / 5.0));
            for from in CAST {
                let way = at - from;
                let met = gusset.met_by(from, way);
                assert!(
                    met.all().windows(2).all(|pair| pair[0] <= pair[1]),
                    "{:?} is out of order",
                    met.all(),
                );
                for &along in met.all() {
                    let place = from + way * along;
                    let read = gusset.at(gusset.uv(place));
                    assert!(
                        read.abs_diff_eq(place, 1e-4),
                        "the crossing at {along} stands at {place}, which reads back {read}",
                    );
                }
            }
        }
    }
}

/// **The tip reads nought whatever the ray, and is never counted.**
///
/// The tangent plane at the tip is the face the two blends share, and that face
/// touches the round — so every line in it runs tangent to the round and the
/// equation is satisfied there by every ray. It is a doubled root and not a
/// crossing, which is why `.notes/KERNEL.md` §7.7 divides it out before the
/// harmonic is read: six roots are left where eight would be.
///
/// Held as a comparison rather than against a bound: the reading at the tip is
/// nine orders below the reading a fifth of a radian away from it. And no ray
/// aimed inside the patch is answered at the tip, where the ruling has closed
/// to nothing and there is no line left to cross.
#[test]
fn the_tip_reads_nought_whatever_the_ray_and_is_never_counted() {
    let gusset = square();
    let framing = gusset.framing();
    let tip = framing.met;
    let angle = gusset.filled.axis.angle_of(tip);
    for from in CAST {
        for down in 0..=4 {
            let at = gusset.at(DVec2::new(
                TURN[0] + (TURN[1] - TURN[0]) * f64::from(down) / 5.0,
                0.5,
            ));
            let way = at - from;
            assert!(
                gusset.aimed(angle, from, way, framing).abs()
                    < 1e-9 * gusset.aimed(angle + 0.2, from, way, framing).abs(),
                "the tip reads {} from {from}",
                gusset.aimed(angle, from, way, framing),
            );
            for &along in gusset.met_by(from, way).all() {
                assert!(
                    (from + way * along).distance(tip) > 1e-6,
                    "the tip was counted a crossing from {from}",
                );
            }
        }
    }
}

/// **A ray lying along a ruling is not counted crossing it.**
///
/// The graze policy [`Crossings`] states, in the case a ruled surface has and a
/// quadric does not: a ray *in* the surface meets it everywhere, which is no
/// crossing to count.
#[test]
fn a_ray_along_a_ruling_counts_for_none() {
    let gusset = square();
    for down in 0..=4 {
        let angle = TURN[0] + (TURN[1] - TURN[0]) * f64::from(down) / 5.0;
        let head = gusset.at(DVec2::new(angle, 0.0));
        let foot = gusset.at(DVec2::new(angle, 1.0));
        let met = gusset.met_by(head, foot - head);
        assert!(
            met.all().iter().all(|along| along.abs() > 1e-9),
            "a ray along a ruling was counted crossing it: {:?}",
            met.all(),
        );
    }
}

/// **The patch spans the fillet's angle from its first edge to the tip**, the
/// near way round.
///
/// Hand-computed and held against [`TURN`], which the fixture works out for
/// itself: the first edge starts at `(0, 0, 1)`, a half turn round from the
/// fillet's reference, and the tip stands at `(1, 1, 0)`, a quarter turn from
/// it. The leaning corner keeps the same tip and moves only its start, the
/// riser it began on having tipped.
#[test]
fn the_patch_spans_the_angle_between_its_first_edge_and_the_tip() {
    let gusset = square();
    let bounds = gusset.bounds();
    for (got, want) in bounds.iter().zip(TURN) {
        assert!(
            (got - want).abs() < 1e-12,
            "{bounds:?} rather than {TURN:?}"
        );
    }
    // Under a half turn, which is what makes it the near way round and the gap
    // rather than the fillet.
    assert!(
        (bounds[1] - bounds[0]).abs() < PI,
        "{bounds:?} is the far way"
    );

    let leaning = leaning();
    let turned = leaning.bounds();
    assert!(
        (turned[1] - TURN[1]).abs() < 1e-12,
        "{turned:?} left the tip where it was not",
    );
    assert!(
        (turned[0] - TURN[0]).abs() > 1e-3,
        "{turned:?} began where the square one did",
    );
    assert!(
        (turned[1] - turned[0]).abs() < PI,
        "{turned:?} is the far way"
    );
}

/// **The tip is the one place the parameters say nothing, and the patch wraps
/// in its first alone.**
///
/// The tip is where every ruling has closed to nothing, so `v` names no
/// direction there — the same question a cone's apex answers, one tier down.
///
/// And the wrapping is held to what it *does* rather than to what it says: a
/// whole turn added to the angle names the same place, and a whole one added
/// to the run along the ruling names another.
#[test]
fn the_tip_is_the_one_place_the_patch_says_nothing_at() {
    let gusset = square();
    assert!(gusset.singular(gusset.met()));
    assert!(!gusset.singular(gusset.from));

    let uv = DVec2::new(TURN[0], 0.5);
    assert_eq!(gusset.round(), BVec2::new(true, false));
    assert!(!gusset.singular(gusset.at(uv)));
    assert!(
        gusset.at(uv + DVec2::new(TAU, 0.0)).distance(gusset.at(uv)) < 1e-9,
        "a whole turn of the angle left the place",
    );
    assert!(
        gusset.at(uv + DVec2::Y).distance(gusset.at(uv)) > 0.1,
        "a whole run along the ruling stayed where it was",
    );
}

/// **Two patches that differ key differently, and one keyed twice keys alike.**
///
/// A filter and never a decision — see [`Key`] — so what this holds is that the
/// four things a patch is made of all reach the key.
#[test]
fn a_patch_keys_by_everything_it_is_made_of() {
    let gusset = square();
    assert_eq!(gusset.key(), square().key());
    assert_ne!(gusset.key(), leaning().key());
    let turned = Gusset::new(gusset.filled, gusset.cut, gusset.from, !gusset.turning);
    assert_ne!(gusset.key(), turned.key(), "the branch is not in the key");
    // Further round the same fillet, which is where the first edge is free to
    // start and the one thing left that a key has to tell apart.
    let along = gusset.at(DVec2::new(TURN[0] + 0.3, 0.0));
    let moved = Gusset::new(gusset.filled, gusset.cut, along, gusset.turning);
    assert_ne!(
        gusset.key(),
        moved.key(),
        "where the first edge starts is not in the key"
    );
}

/// **The second edge is chorded onto the round and reaches the tip, and asking
/// for less stray buys more chords.**
///
/// Every place of it lies on the round — that is what the edge *is*, the
/// tangency the rulings land at — and the last is the tip both edges share, so
/// a caller sewing it finds the corner where it expects one.
///
/// **The stray is measured and not predicted**, so what is held is that it
/// answers under what was asked and falls as the walk is refined. Between a
/// hundredth and a ten-thousandth the chord count has to rise, a curve that
/// came back the same either way being one nothing was measuring.
#[test]
fn the_second_edge_is_walked_onto_the_round_and_reaches_the_tip() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let mut walked = Vec::new();
        let mut counted = Vec::new();
        let mut strayed = Vec::new();
        for sagitta in [1e-2, 1e-3, 1e-4] {
            let most = gusset.chorded(sagitta, &mut walked);
            assert!(most <= sagitta, "{named}: {most} strays past {sagitta}");
            assert!(
                walked.len() >= 5,
                "{named}: {} chords is no walk",
                walked.len(),
            );
            for &at in &walked {
                assert!(
                    (gusset.cut.axis.off(at) - gusset.cut.radius).abs() < 1e-9,
                    "{named}: {at} is off the round",
                );
            }
            let last = *walked.last().expect("a walk has an end");
            assert!(
                last.distance(gusset.met()) < 1e-9,
                "{named}: {last} is not the tip",
            );
            let first = walked[0];
            let began = gusset.at(DVec2::new(gusset.bounds()[0], 1.0));
            assert!(
                first.distance(began) < 1e-9,
                "{named}: {first} is not the foot of the ruling the patch begins at",
            );
            counted.push(walked.len());
            strayed.push(most);
        }
        assert!(
            counted[0] < counted[2],
            "{named}: {counted:?} chords for a hundredth and a ten-thousandth",
        );
        assert!(
            strayed[2] < strayed[0],
            "{named}: {strayed:?} strayed the same however finely it was walked",
        );
    }
}

/// **Every place of the patch falls inside the box it says it fills**, and the
/// box is no wider than the corner it stands in.
///
/// Asked over a grid of the patch's own parameters rather than at its corners
/// alone: the rulings are what the convex hull argument turns on, so a box
/// holding both edges and missing the middle is the way it could be wrong.
///
/// **And it is coarse rather than wrong.** Read over the fillet's whole turn,
/// so it reaches a reach past the arc the patch covers — which is why what is
/// held is that it stays inside two reaches of the tip on the square corner,
/// where the patch itself covers a quarter turn of a unit fillet.
#[test]
fn the_box_holds_every_place_of_the_patch() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let fills = gusset.fills();
        let bounds = gusset.bounds();
        for round in 0..=8 {
            let u = bounds[0] + (bounds[1] - bounds[0]) * f64::from(round) / 8.0;
            for along in 0..=8 {
                let uv = DVec2::new(u, f64::from(along) / 8.0);
                let at = gusset.at(uv);
                assert!(fills.holds(at), "{named}: {fills:?} misses {at}");
            }
        }
        assert!(fills.holds(gusset.met()), "{named}: the tip is outside");
        assert!(
            fills.holds(gusset.from),
            "{named}: the first edge is outside"
        );
        // The whole turn of a unit fillet reaches two across, and the walk adds
        // the round's own reach to it.
        assert!(
            fills.half().max_element() < 3.0,
            "{named}: {fills:?} is wider than the corner",
        );
    }
}

/// **A box the patch reaches is answered, and one it comes nowhere near is
/// refused.**
///
/// Settled off the patch's own box rather than by halving the caller's — see
/// [`Gusset::spans`] — so what is held is that the answer turns on where the
/// patch actually is. The square corner spans the unit cube about the origin
/// and the tip at `(1, 1, 0)`, so a box a hundred out is refused and one round
/// the tip is not, whatever slack either is asked with.
#[test]
fn a_box_is_spanned_where_the_patch_reaches_it() {
    let gusset = square();
    assert!(
        gusset.spans(Bounds::about(gusset.met(), 0.1), 0.0),
        "the tip"
    );
    assert!(
        gusset.spans(Bounds::about(gusset.from, 0.1), 0.0),
        "the first edge",
    );
    assert!(
        !gusset.spans(Bounds::about(DVec3::new(100.0, 0.0, 0.0), 1.0), 0.0),
        "a box a hundred out",
    );
    // And slack is what makes a near miss a hit, which is the whole of what a
    // caller hands one in for.
    let near = Bounds::about(DVec3::new(4.0, 1.0, 0.0), 0.1);
    assert!(!gusset.spans(near, 0.0), "clear of the patch");
    assert!(
        gusset.spans(near, 3.0),
        "clear of it by less than the slack"
    );
}

/// **A place on the patch reads no distance from it, and one moved along the
/// normal reads how far it was moved.**
///
/// Sought rather than solved — see [`Gusset::nearest`] — so what is held is
/// that the search finds the ruling the place actually stands off. Asked at
/// nine places across the patch, because a search right at one corner and
/// wrong between them is exactly how it could fail.
///
/// **And what comes back is always on the patch**, wherever it was asked from
/// — which is what a march leans on when it corrects a place onto a surface.
#[test]
fn a_place_off_the_patch_reads_the_distance_it_was_moved() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let bounds = gusset.bounds();
        for round in 1..4 {
            let u = bounds[0] + (bounds[1] - bounds[0]) * f64::from(round) / 4.0;
            for along in 1..4 {
                let uv = DVec2::new(u, f64::from(along) / 4.0);
                let at = gusset.at(uv);
                assert!(gusset.off(at) < 1e-6, "{named}: {uv} is on the patch");
                // A fifth of a reach out along the normal, which is the one
                // direction the answer is the whole of the move.
                let off = at + gusset.normal(uv) * 0.2;
                assert!(
                    (gusset.off(off) - 0.2).abs() < 1e-6,
                    "{named}: {uv} moved a fifth reads {}",
                    gusset.off(off),
                );
            }
        }
        for from in CAST {
            let onto = gusset.nearest(from);
            assert!(
                gusset.off(onto) < 1e-6,
                "{named}: {from} was answered {onto}, which is off the patch",
            );
        }
    }
}

/// **The net holds the patch to what it says it strays**, which is the one
/// promise a format with no entity for a ruled surface is given.
///
/// Read between the rulings rather than at them: the places of the net are on
/// the patch by construction, so a net that missed the patch would miss it in
/// the middle of a cell. The shares of the turn are the three the walk probes
/// at, where what the net leaves out is a blend of the two edges' own — the
/// first edge's bend, which is written down, and the second's, which was
/// measured. The blend is never worse than the worse of the two, so the answer
/// holds at every share along the ruling.
///
/// The rounding is the tip alone: the last row is written as the touch point
/// where the ellipse would be read there, and the two agree to the last bits
/// rather than exactly.
#[test]
fn the_net_holds_the_patch_to_what_it_says_it_strays() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let mut net = Vec::new();
        let mut counted = Vec::new();
        let mut strayed = Vec::new();
        for sagitta in [1e-2, 1e-3, 1e-4] {
            let most = gusset.netted(sagitta, &mut net);
            assert!(most <= sagitta, "{named}: {most} strays past {sagitta}");
            assert_eq!(net.len() % 2, 0, "{named}: a ruling lost an end");
            let rulings = net.len() / 2;
            assert!(rulings > 4, "{named}: {rulings} rulings is no net");

            let [from, to] = gusset.bounds();
            for ruling in 0..rulings {
                let [head, foot] = [net[ruling * 2], net[ruling * 2 + 1]];
                assert!(
                    (gusset.filled.axis.off(head) - gusset.filled.radius).abs() < 1e-9,
                    "{named}: {head} is off the fillet",
                );
                assert!(
                    (gusset.cut.axis.off(foot) - gusset.cut.radius).abs() < 1e-9,
                    "{named}: {foot} is off the round",
                );
            }
            assert!(
                net[0].distance(gusset.from) < 1e-9,
                "{named}: {} is not where the first edge starts",
                net[0],
            );
            let last = [net[net.len() - 2], net[net.len() - 1]];
            for at in last {
                assert!(
                    at.distance(gusset.met()) < 1e-12,
                    "{named}: {at} is not the tip",
                );
            }
            // The ellipse read at the far end of the turn is that same tip,
            // which is what says the row written there was not invented.
            let ends = gusset.at(DVec2::new(to, 0.0));
            assert!(
                ends.distance(gusset.met()) < 1e-9,
                "{named}: the first edge ends at {ends} rather than the tip",
            );

            // One cell to a pair of rulings, so a net of `rulings` rows has one
            // fewer cell than it has rows.
            let step = (to - from) / (rulings - 1) as f64;
            for cell in 0..rulings - 1 {
                for share in [0.25, 0.5, 0.75] {
                    let heads = net[cell * 2].lerp(net[cell * 2 + 2], share);
                    let feet = net[cell * 2 + 1].lerp(net[cell * 2 + 3], share);
                    for along in [0.0, 0.25, 0.5, 0.75, 1.0] {
                        let uv = DVec2::new(from + step * (cell as f64 + share), along);
                        let apart = gusset.at(uv).distance(heads.lerp(feet, along));
                        assert!(
                            apart <= most + 1e-12,
                            "{named}: the net misses {uv} by {apart}, over {most}",
                        );
                    }
                }
            }
            counted.push(rulings);
            strayed.push(most);
        }
        assert!(
            counted[0] < counted[2],
            "{named}: {counted:?} rulings for a hundredth and a ten-thousandth",
        );
        assert!(
            strayed[2] < strayed[0],
            "{named}: {strayed:?} strayed the same however finely it was laid",
        );
    }
}

/// How far the flat triangle on `corners` stands from the patch, measured over
/// the triangle itself at `steps` to a side.
///
/// **At matching parameters**, which is what the answer is about: a place of
/// the patch against the place the triangle's own plane puts those same
/// parameters at.
fn measured(gusset: &Gusset, corners: [DVec2; 3], steps: u32) -> f64 {
    let places = corners.map(|uv| gusset.at(uv));
    let mut most = 0.0_f64;
    for one in 0..=steps {
        for two in 0..=(steps - one) {
            let held = [
                1.0 - f64::from(one + two) / f64::from(steps),
                f64::from(one) / f64::from(steps),
                f64::from(two) / f64::from(steps),
            ];
            let uv = (0..3).fold(DVec2::ZERO, |sum, at| sum + corners[at] * held[at]);
            let flat = (0..3).fold(DVec3::ZERO, |sum, at| sum + places[at] * held[at]);
            most = most.max(gusset.at(uv).distance(flat));
        }
    }
    most
}

/// The two triangles the cell between the angles `u` and the runs `v` is cut
/// into, which is what a mesher lays over one.
fn halves(u: [f64; 2], v: [f64; 2]) -> [[DVec2; 3]; 2] {
    let corner = |at: usize, along: usize| DVec2::new(u[at], v[along]);
    [
        [corner(0, 0), corner(1, 0), corner(1, 1)],
        [corner(0, 0), corner(1, 1), corner(0, 1)],
    ]
}

/// **A triangle never strays further than the answer says**, measured over the
/// triangle rather than at its corners.
///
/// The corners are on the patch by construction, so a bound that was under the
/// truth would be under it in the middle. Read over whole cells and over a
/// triangle none of whose corners share an angle or a run, which is the general
/// case the reduction has to hold for.
///
/// **And it falls as the cell narrows.** A triangle spanning the whole ruling
/// carries the ruling's own turn, which goes as the width rather than as its
/// square — so five cells is worth about five times one, not twenty-five.
#[test]
fn a_triangle_never_strays_further_than_it_says() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let [from, to] = gusset.bounds();
        let mut widest = Vec::new();
        for cells in [1u32, 2, 5] {
            let step = (to - from) / f64::from(cells);
            let mut most = 0.0_f64;
            for cell in 0..cells {
                let u = [
                    from + step * f64::from(cell),
                    from + step * f64::from(cell + 1),
                ];
                for held in halves(u, [0.0, 1.0]) {
                    let says = gusset.straying(held);
                    let truth = measured(&gusset, held, 12);
                    assert!(
                        truth <= says + 1e-12,
                        "{named}: {held:?} strays {truth}, over the {says} promised",
                    );
                    most = most.max(says);
                }
            }
            widest.push(most);
        }
        assert!(
            widest[2] < widest[0] / 3.0,
            "{named}: {widest:?} hardly fell as the cells narrowed",
        );

        // A triangle sharing neither an angle nor a run between any two of its
        // corners, which no cell of a grid is and the reduction still holds for.
        let slant = [
            DVec2::new(from, 0.0),
            DVec2::new(from + (to - from) * 0.4, 1.0),
            DVec2::new(from + (to - from) * 0.7, 0.3),
        ];
        let says = gusset.straying(slant);
        let truth = measured(&gusset, slant, 16);
        assert!(
            truth <= says + 1e-12,
            "{named}: a slanting triangle strays {truth}, over the {says} promised",
        );
    }
}

/// **A triangle strays by the worst of its three sides**, which is the whole of
/// the reduction: a ruled surface is straight along its ruling, so what a
/// triangle leaves out stands on its own boundary.
///
/// A side is read by passing one corner twice, which is what
/// [`Surface::straying`](crate::solid::geometry::surface::Surface) already
/// names that shape for.
///
/// **And a side at one angle strays nothing at all.** Both its corners sit on
/// one ruling, the patch is straight along that ruling, and the side is the
/// ruling — so there is nothing between them to leave out.
#[test]
fn a_triangle_strays_by_the_worst_of_its_three_sides() {
    let gusset = square();
    let [from, to] = gusset.bounds();
    let corners = [
        DVec2::new(from, 0.0),
        DVec2::new(from + (to - from) * 0.4, 1.0),
        DVec2::new(from + (to - from) * 0.7, 0.3),
    ];
    let sides = [[0, 1], [1, 2], [2, 0]]
        .map(|[a, b]| gusset.straying([corners[a], corners[b], corners[b]]));
    let worst = sides.iter().fold(0.0_f64, |far, &at| far.max(at));
    assert!(
        (gusset.straying(corners) - worst).abs() < 1e-15,
        "{} is not the worst of {sides:?}",
        gusset.straying(corners),
    );
    assert!(worst > 1e-3, "{sides:?} left nothing to be the worst of");

    let angle = from + (to - from) * 0.4;
    let ruling = DVec2::new(angle, 1.0);
    assert_eq!(
        gusset.straying([DVec2::new(angle, 0.0), ruling, ruling]),
        0.0,
        "the patch left its own ruling",
    );
}

/// **A grid at a sagitta holds every triangle inside one of its cells**, which
/// is the whole of what a stride is for.
///
/// Asked of the two triangles of every cell of the grid, and measured outright
/// over the triangle for the first cell, the middle one and the last — the last
/// being where the ruling has closed to a point and the reading is at its
/// hardest.
///
/// **And a finer sagitta rules a finer grid**, in both parameters: the angle
/// takes half the sagitta and the run along the ruling the other half, so
/// neither can stand still while the other moves.
#[test]
fn a_grid_at_a_sagitta_holds_every_triangle_inside_a_cell() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let [from, to] = gusset.bounds();
        let span = to - from;
        let mut cells = Vec::new();
        for sagitta in [1e-2, 1e-3] {
            let cell = gusset.strides(sagitta);
            let across = cell.y.min(1.0);
            let steps = (span.abs() / cell.x).round() as u32;
            let runs = (1.0 / across).ceil() as u32;
            assert!(steps > 1, "{named}: {steps} steps round is no grid");
            for step in 0..steps {
                let u = [
                    from + span * f64::from(step) / f64::from(steps),
                    from + span * f64::from(step + 1) / f64::from(steps),
                ];
                for run in 0..runs {
                    let v = [
                        f64::from(run) / f64::from(runs),
                        f64::from(run + 1) / f64::from(runs),
                    ];
                    for held in halves(u, v) {
                        let says = gusset.straying(held);
                        assert!(
                            says <= sagitta,
                            "{named}: a cell of the grid promises {says}, over {sagitta}",
                        );
                        if step == 0 || step == steps / 2 || step + 1 == steps {
                            let truth = measured(&gusset, held, 8);
                            assert!(
                                truth <= sagitta,
                                "{named}: a cell of the grid strays {truth}, over {sagitta}",
                            );
                        }
                    }
                }
            }
            cells.push(cell);
        }
        assert!(
            cells[1].x < cells[0].x && cells[1].y < cells[0].y,
            "{cells:?} did not narrow for {named} as the sagitta fell",
        );
    }
}

/// **The second edge files as a run with two ends**, which is what tells it
/// from every marched curve the kernel has filed until now.
///
/// A march round a meeting comes back to where it began; this one runs from the
/// ruling the patch starts at round to the tip and stops. Filed, the run says
/// so, and its whole turn walks that stretch and no further.
#[test]
fn the_second_edge_files_as_a_run_with_two_ends() {
    for (named, gusset) in [("square", square()), ("leaning", leaning())] {
        let mut walked = Vec::new();
        let most = gusset.chorded(1e-4, &mut walked);
        let chords = walked.len() - 1;
        let mut marchings = Marchings::default();
        let run = marchings.add(&walked, most);

        let strayed = marchings.strayed(run);
        assert!(
            !strayed.shut,
            "{named}: the second edge was filed as closing"
        );
        assert_eq!(strayed.most, most, "{named}: the run lost its bound");
        assert_eq!(
            marchings.steps(run, TAU),
            chords,
            "{named}: the run answers with other than the chords it has",
        );

        let began = gusset.at(DVec2::new(gusset.bounds()[0], 1.0));
        for (t, want) in [(0.0, began), (TAU, gusset.met())] {
            let at = marchings.at(run, t);
            assert!(
                at.distance(want) < 1e-9,
                "{named}: {at} at {t} is not {want}",
            );
        }

        // Every place it was laid down at is on the round, which is the join
        // the edge keeps — the readings between them stand on chords and are
        // held to the bound above instead.
        for (_, at) in marchings.sampled(run) {
            assert!(
                (gusset.cut.axis.off(at) - gusset.cut.radius).abs() < 1e-9,
                "{named}: {at} is off the round",
            );
        }
    }
}

/// **A normal read at a place off the patch turns by a square root of how far
/// off it stands**, where every surface of the exact tier turns by a
/// proportion — and that is what `Face::smooth` had to be told before it would
/// read either of the patch's joins as the tangency it is.
///
/// **Exactly `√(2·off/r)` along the first edge**, and hand-computed. The angle
/// comes off an `acos` of the radius over the distance from the fillet's axis,
/// which is square-root singular where a place lies on the fillet itself; and
/// the patch's normal at `v = 0` *is* the fillet's own, so it turns one for one
/// with the angle. A reach of one and a walk of a hundred-millionth is
/// `1.4142e-4` — four orders above the walk, where a cylinder of the same reach
/// would read the walk itself.
///
/// **Four times the walk reads twice the room**, which is the square root
/// asserted as a law rather than as a number: a proportion would read four
/// times.
///
/// **And it is a bound rather than a likeness.** A place actually moved by
/// `off` along the patch and inverted back reads a normal no further from the
/// true one than this said it could be — over the whole turn, and in twelve
/// directions at each place.
///
/// **Over three corners, and the third is why one of them is here at all.**
/// [`framed`] is [`square`] with the fillet's angles counted from somewhere
/// else, which moves no place of the patch and moves every angle read off it:
/// its turn runs past the half turn an inversion answers in. A reading that
/// trusted the inverted angle against the patch's own bounds put every place of
/// that corner at the far end, and read `0.39` where a walk of nothing owes
/// nought.
#[test]
fn a_normal_read_off_the_patch_turns_by_the_root_of_the_walk() {
    let walk: f64 = 1e-8;
    // Both corners are blended at a reach of one, so the root is `√(2·off)` and
    // the bearing's own share beside it is the walk itself, four orders under.
    let want = (2.0 * walk).sqrt() + walk;
    let doubled = 2.0 * (2.0 * walk).sqrt() + 4.0 * walk;
    for (what, gusset) in [
        ("square", square()),
        ("leaning", leaning()),
        ("framed", framed()),
    ] {
        let [from, to] = gusset.bounds();
        for share in [0.0, 0.25, 0.5, 0.75] {
            let u = from + (to - from) * share;
            let head = gusset.at(DVec2::new(u, 0.0));
            // A walk of nothing turns the normal by nothing. Held to
            // [`ALIGNED`] rather than to nought: a frame the world axes do not
            // line up with reads its own corners a rounding apart.
            let still = gusset.wavering(head, 0.0);
            assert!(still <= ALIGNED, "{what} moved {still} by nothing");
            let room = gusset.wavering(head, walk);
            // Held as a proportion of itself, and to a thousandth: the run
            // along the ruling adds a share of its own, which grows as the
            // ruling shortens towards the tip. Reading the walk as a
            // proportion rather than as a root would be out by four orders,
            // so the window pins the law and not the arithmetic.
            assert!(
                (room / want - 1.0).abs() < 1e-3,
                "{what} at {u} reads {room} where the root says {want}",
            );
            let quadrupled = gusset.wavering(head, 4.0 * walk);
            assert!(
                (quadrupled / doubled - 1.0).abs() < 1e-3,
                "{what} at {u} reads {quadrupled} where the root says {doubled}",
            );
        }

        for share in [0.2, 0.5, 0.8] {
            let u = from + (to - from) * share;
            for v in [0.1, 0.5, 0.9] {
                let at = gusset.at(DVec2::new(u, v));
                let room = gusset.wavering(at, walk);
                let here = gusset.normal(gusset.uv(at));
                let (one, two) = here.any_orthonormal_pair();
                for step in 0..12 {
                    let (up, out) = (TAU * f64::from(step) / 12.0).sin_cos();
                    let moved = at + (one * out + two * up) * walk;
                    let turned = here.distance(gusset.normal(gusset.uv(moved)));
                    assert!(
                        turned <= room,
                        "{what} at {u},{v} read {turned} off where {room} was allowed",
                    );
                }
            }
        }
    }
}
