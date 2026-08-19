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
        .sew(
            combining.kept(),
            combining.loops(),
            combining.imprints(),
            &mut body,
        )
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

/// **A loop walked the other way turns its marks over and steps them round by
/// one.**
///
/// The rule a face grown the other way off its plane depends on: its loops are
/// wound clockwise in their own parameters and the sewing turns them over — and
/// a mark says what the stretch *leaving* its vertex runs along, so reversing
/// alone would hand each vertex the mark of the stretch that used to enter it.
///
/// Invisible today, because every mark a planar boolean makes is
/// [`Came::Edge`] and turning those over changes nothing. It stops being
/// invisible the moment an arc reaches a face that looks the other way, and by
/// then there is nothing on screen to tell a chord from an arc.
#[test]
fn a_loop_walked_the_other_way_steps_its_marks_round() {
    // Three vertices, the stretches leaving them along imprints 0, 1 and 2.
    // Reversed the loop is `C B A`: C to B is what B to C was, B to A is what
    // A to B was, and A round to C is what C to A was.
    // Real vertices off a real body rather than handles conjured here: an
    // arena mints them and there is no other way to hold one, which is the
    // point of an arena.
    let mut body = Body::default();
    let made: Vec<VertexId> = (0..3)
        .map(|at| {
            body.topology_mut().add_vertex(Vertex {
                at: DVec3::new(at as f64, 0.0, 0.0),
                tolerance: PLACED,
            })
        })
        .collect();
    let stood = |vertex: usize, along: u32| Stepped {
        vertex: made[vertex],
        along: Came::Arc(along),
    };
    let mut walk = [stood(0, 0), stood(1, 1), stood(2, 2)];
    turned(&mut walk);
    assert_eq!(walk, [stood(2, 1), stood(1, 0), stood(0, 2)]);

    // Twice round is where it started, which is what says it is a walk of the
    // same loop rather than a shuffle.
    turned(&mut walk);
    assert_eq!(walk, [stood(0, 0), stood(1, 1), stood(2, 2)]);
}
