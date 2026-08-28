use super::*;
use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::{Combining, Operation};
use crate::solid::build::builder::Extrusion;
use crate::solid::grown::Grown;
use crate::solid::named::{Named, Step};

/// The two steps the blocks below are grown by.
///
/// Two, and that is the point: a boolean's answer holds faces of both operands,
/// and which feature grew a face is half of what names it — see
/// [`Named`](crate::solid::named::Named). Both bodies calling their base
/// `Grown::Base` is exactly the collision the other half is for.
const CUBE: Step = Step(1);
const TOOL: Step = Step(2);

/// A block from `corners`, carried `deep` off `plane`, grown by `by`.
fn block(plane: Plane, corners: &[(f64, f64)], deep: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(corners);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, 0, plane, deep, by).body()
}

/// The four-by-four-by-four block everything below is cut against.
fn cube() -> Body {
    block(
        Plane::GROUND,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
        CUBE,
    )
}

/// A two-by-two-by-two block over one corner of it, clear of its ends.
fn corner() -> Body {
    let raised = Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal(),
        ..Plane::GROUND
    };
    block(
        raised,
        &[(3.0, 3.0), (5.0, 3.0), (5.0, 5.0), (3.0, 5.0)],
        2.0,
        TOOL,
    )
}

/// Combine two bodies and sew the answer, or say it would not.
fn combined(one: &Body, two: &Body, doing: Operation) -> Option<Body> {
    let mut combining = Combining::default();
    if !combining.combine(one, two, doing) {
        return None;
    }
    let mut body = Body::default();
    Sewing::default()
        .sew(combining.sewn(), &mut body)
        .then_some(body)
}

/// **Regions cut apart in two parameter planes are sewn onto one edge**, which
/// is the whole of what the registry is for.
///
/// The wall of the notch and the face of the cube it cuts into are worked out
/// by different arithmetic in different planes, and meet along a line that
/// neither of them knows the other has. That they come back as *one* edge with
/// two faces on it is what the validity check asserts on the way out — and what
/// the count below reads back.
#[test]
fn the_two_bodies_meet_along_edges_they_share_rather_than_edges_they_both_made() {
    let body = combined(&cube(), &corner(), Operation::Cut).expect("a cut of two blocks");
    let topology = body.topology();

    // **No two vertices stand in one place.** Which is the registry's whole
    // claim: the notch's walls and the cube's faces were cut in different
    // parameter planes and meet along corners neither knew the other had, so
    // without a lookup by position each of those corners would come back twice
    // — and the edges hanging off them would not be shared either.
    let placed: Vec<DVec3> = topology.vertices().map(|(_, it)| it.at).collect();
    for (at, &here) in placed.iter().enumerate() {
        for &there in &placed[at + 1..] {
            assert!(
                here.distance(there) > PLACED,
                "two vertices stand at {here:?} and {there:?}",
            );
        }
    }
    // And the body knows its own shape: one shell, no cavities, genus nought.
    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
}

/// **The answer's faces say which feature grew each of them**, which is the
/// half of a name a body made of two bodies cannot do without.
///
/// A cut of the cube by the corner block. Both operands call their own ends
/// `Base` and `Far`, so without the step half the answer would hold four faces
/// answering to two names — and clicking the floor of the notch would light the
/// base of the block it was cut out of.
///
/// Ten names over the two: the cube's six, whole; and four of the tool's — its
/// two ends, and the two of its four walls that reach inside the cube at all.
/// The other two stood clear and are no part of the notch. Which is
/// `.notes/KERNEL.md` §5's rule that a cut's new surfaces are named by the
/// tool, read off the body.
#[test]
fn every_face_of_the_answer_says_which_of_the_two_bodies_grew_it() {
    let body = combined(&cube(), &corner(), Operation::Cut).expect("a cut of two blocks");
    let names: Vec<Named> = body.names().collect();

    let mine = |by| move |named: &&Named| named.by == by;
    assert_eq!(names.iter().filter(mine(CUBE)).count(), 6, "{names:?}");
    assert_eq!(names.iter().filter(mine(TOOL)).count(), 4, "{names:?}");

    // And the walls of the notch are the tool's own two, not the cube's: a cut
    // leaves the tool's surface behind facing the other way, and the name says
    // which surface rather than which side of it.
    let walls = names
        .iter()
        .filter(mine(TOOL))
        .filter(|named| matches!(named.grown, Grown::Side(_)))
        .count();
    assert_eq!(walls, 2, "{names:?}");
}

/// **The lookup rests on one claim**: a vertex standing within [`PLACED`] of a
/// place is filed in one of the eight cells [`celled`] asks about. Broken, the
/// sewing would put down two vertices where two regions meet and close
/// nothing — so it is held here against every corner of the box of one
/// tolerance about the place, which reaches every direction a coincidence can
/// lie in and further than the ball itself does.
///
/// The places asked from are a cell corner, a cell middle, a whole number of
/// cells out, and a coordinate large enough that a cell index runs to eleven
/// figures — which is where a grid over a tolerance this fine would fall apart
/// if it were going to.
#[test]
fn every_place_within_a_tolerance_falls_in_a_cell_the_lookup_asks() {
    let asked = [
        DVec3::ZERO,
        DVec3::new(CELL, 2.0 * CELL, -3.0 * CELL),
        DVec3::new(0.5 * CELL, 1.5 * CELL, -2.5 * CELL),
        DVec3::new(1e3, -1e3, 1e3),
    ];
    for at in asked {
        let cells = celled(at);
        for x in [-1.0, 0.0, 1.0] {
            for y in [-1.0, 0.0, 1.0] {
                for z in [-1.0, 0.0, 1.0] {
                    // A corner of the box of side two tolerances, which holds
                    // every place the ball does and then some — the claim the
                    // cells rest on is one about each axis on its own.
                    let near = at + DVec3::new(x, y, z) * PLACED;
                    let home = celled(near)[0];
                    assert!(
                        cells.contains(&home),
                        "{near} is filed in {home:?}, which {at} never asks",
                    );
                }
            }
        }
    }
}

/// An edge is filed by the pair of vertices it runs between and by nothing
/// else, so the two faces that share it — which walk it in opposite
/// directions — reach the same chain. A different pair is a different key, or
/// the chain would be every edge of the body.
#[test]
fn an_edge_keys_the_same_whichever_way_round_it_is_walked() {
    let mut body = Body::default();
    let place = |at| Vertex {
        at,
        tolerance: PLACED,
    };
    let one = body.topology_mut().add_vertex(place(DVec3::ZERO));
    let two = body.topology_mut().add_vertex(place(DVec3::X));
    let three = body.topology_mut().add_vertex(place(DVec3::Y));

    assert_eq!(tied([one, two]), tied([two, one]));
    assert_ne!(tied([one, two]), tied([one, three]));
    assert_ne!(tied([one, two]), tied([two, three]));
}

/// The places pinned come out in curve order, each curve's in the order that
/// curve runs, with the ones that are a place already found dropped.
///
/// All three halves matter. The order by curve is what lets a reader halve the
/// list rather than walk it; the order along the curve is what saves both
/// readers a sort; and the dropping is what keeps one place from splitting a
/// rim twice.
#[test]
fn the_pinned_places_come_out_by_curve_and_along_it_with_the_repeats_dropped() {
    let mut sewing = Sewing::default();
    let place = |curve, along, at| Pinned { curve, at, along };
    // Out of curve order, out of parameter order, and with one place on each
    // curve written down twice — a rounding apart the second time, which is
    // how the two faces meeting on a rim arrive at it.
    sewing.scratch.pinned.extend([
        place(1, 2.0, DVec3::new(2.0, 0.0, 0.0)),
        place(0, 1.0, DVec3::new(0.0, 1.0, 0.0)),
        place(1, 0.0, DVec3::ZERO),
        place(0, 1.0, DVec3::new(0.0, 1.0, PLACED / 4.0)),
        place(1, 0.0, DVec3::new(PLACED / 4.0, 0.0, 0.0)),
    ]);
    sewing.fold();

    let along = |on| -> Vec<f64> {
        placed_on(&sewing.scratch.pinned, on)
            .iter()
            .map(|it| it.along)
            .collect()
    };
    assert_eq!(along(0), [1.0]);
    assert_eq!(along(1), [0.0, 2.0]);
    assert_eq!(along(2), [] as [f64; 0], "a curve nothing pinned");
    assert_eq!(sewing.scratch.pinned.len(), 3, "a repeat survived");
}
