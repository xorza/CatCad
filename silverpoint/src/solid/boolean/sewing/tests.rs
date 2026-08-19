use super::*;
use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::{Combining, Operation};
use crate::solid::build::extrusion::Extrusion;
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
        .sew(combining.kept(), combining.loops(), &mut body)
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
