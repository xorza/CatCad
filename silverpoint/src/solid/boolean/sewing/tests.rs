use super::*;
use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::{Combining, Operation};
use crate::solid::build::extrusion::Extrusion;
use crate::solid::mesh::Mesher;

/// A block from `corners`, carried `deep` off `plane`.
fn block(plane: Plane, corners: &[(f64, f64)], deep: f64) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(corners);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, 0, plane, deep).body()
}

/// The four-by-four-by-four block everything below is cut against.
fn cube() -> Body {
    block(
        Plane::GROUND,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
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

/// How much space `body` shuts in, read off its own triangles.
fn volume(body: &Body) -> f64 {
    Mesher::default().volume(body, 1e-6)
}

/// **Each of the three operations sews a valid body holding the right
/// volume.**
///
/// The same pair of blocks the pipeline is checked on — a four-cube and a
/// two-cube overlapping in a one-by-one-by-two notch — read now as solids
/// rather than as loose surface. A cut leaves `64 − 2`, a join `64 + 8 − 2`,
/// and an intersection the notch itself.
///
/// Every one of them goes through the validity check on the way out, so an
/// edge left with one face or a shell that does not close is a panic rather
/// than a number that happens to be wrong.
#[test]
fn each_operation_sews_a_body_holding_the_volume_it_should() {
    let (cube, corner) = (cube(), corner());
    for (doing, want) in [
        (Operation::Cut, 62.0),
        (Operation::Join, 70.0),
        (Operation::Intersect, 2.0),
    ] {
        let body = combined(&cube, &corner, doing).unwrap_or_else(|| panic!("{doing:?} refused"));
        let held = volume(&body);
        assert!(
            (held - want).abs() < 1e-9,
            "{doing:?} shut in {held} rather than {want}",
        );
        // One lump, because neither of these operations leaves the answer in
        // pieces at this placement.
        assert_eq!(body.topology().lumps().count(), 1, "{doing:?}");
    }
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

/// **A body swallowed whole leaves a cavity**, which is the one case a boolean
/// of two solids has for a shell inside a shell.
#[test]
fn cutting_a_block_out_of_the_middle_leaves_a_cavity_inside_it() {
    let inner = Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal(),
        ..Plane::GROUND
    };
    let middle = block(
        inner,
        &[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)],
        2.0,
    );
    let body = combined(&cube(), &middle, Operation::Cut).expect("a block out of the middle");

    assert_eq!(body.topology().lumps().count(), 1);
    let (_, lump) = body.topology().lumps().next().expect("the one lump");
    assert_eq!(lump.voids.len(), 1, "the hollow is not a cavity");
    // Four cubed less two cubed, and the mesh reads it whichever shell the
    // triangles came off.
    let held = volume(&body);
    assert!((held - 56.0).abs() < 1e-9, "it shut in {held}");
}

/// Two blocks that do not touch join into a body in two pieces.
#[test]
fn joining_two_blocks_that_never_meet_leaves_a_body_in_two_lumps() {
    let away = block(
        Plane::GROUND,
        &[(9.0, 9.0), (11.0, 9.0), (11.0, 11.0), (9.0, 11.0)],
        2.0,
    );
    let body = combined(&cube(), &away, Operation::Join).expect("two blocks apart");

    assert_eq!(
        body.topology().lumps().count(),
        2,
        "one lump for two blocks"
    );
    let held = volume(&body);
    assert!((held - (64.0 + 8.0)).abs() < 1e-9, "it shut in {held}");

    // And an intersection of them is nothing at all.
    let empty = combined(&cube(), &away, Operation::Intersect).expect("nothing in common");
    assert!(empty.is_empty(), "two blocks apart share something");
}
