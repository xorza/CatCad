use super::*;
use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::extrusion::Extrusion;

/// A block from `corners`, carried `deep` off the ground.
fn block(corners: &[(f64, f64)], deep: f64) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(corners);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, 0, Plane::GROUND, deep).body()
}

/// Where `at` stands in `body`, in the plane's own coordinates lifted by `up`.
fn standing(body: &Body, at: (f64, f64), up: f64) -> Standing {
    let world = Plane::GROUND.point(DVec2::new(at.0, at.1)) + DVec3::Y * up;
    Sounding::default().standing(world, body)
}

/// Assert that `at`, lifted by `up`, is on `body` and that it faces `facing`
/// there.
fn faces(body: &Body, at: (f64, f64), up: f64, facing: DVec3) {
    let found = standing(body, at, up);
    let Standing::On(found) = found else {
        panic!("{at:?} lifted by {up} came back {found:?} rather than on the body");
    };
    assert!(
        found.distance(facing) < 1e-9,
        "{at:?} lifted by {up} faces {found:?} rather than {facing:?}",
    );
}

/// **A block holds what is within it and nothing else**, and says so about the
/// places on its own faces — and which way it faces there — rather than
/// guessing.
///
/// Read at the middle, well outside, and then at points chosen to make the ray
/// cast work: a place level with a face, one lined up with an edge, one lined
/// up with a corner. Every one of those puts some ray straight along something
/// it must not be counted against, which is the whole reason more than one
/// direction is tried.
#[test]
fn a_block_holds_what_is_within_it_and_owns_its_own_faces() {
    // Two by two on the ground, three deep.
    let body = block(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], 3.0);

    assert_eq!(standing(&body, (1.0, 1.0), 1.5), Standing::Inside);
    assert_eq!(
        standing(&body, (9.0, 9.0), 9.0),
        Standing::Outside,
        "far off"
    );
    assert_eq!(
        standing(&body, (1.0, 1.0), -0.5),
        Standing::Outside,
        "below"
    );
    assert_eq!(
        standing(&body, (3.0, 1.0), 1.5),
        Standing::Outside,
        "beside"
    );

    // On the base, on the far end, and on a wall — each of them facing out of
    // the material rather than the way its own surface happens to point: the
    // base lies on the ground plane, whose normal is up, and holds the block
    // above it.
    faces(&body, (1.0, 1.0), 0.0, DVec3::NEG_Y);
    faces(&body, (1.0, 1.0), 3.0, DVec3::Y);
    faces(&body, (0.0, 1.0), 1.5, DVec3::NEG_X);

    // Lined up with an edge and with a corner, inside and out: places whose
    // rays would run along the block's own geometry if the direction were not
    // chosen to avoid it. The first stands on the corner two walls meet at,
    // where either of the two is a true answer to which way it faces.
    assert!(matches!(standing(&body, (0.0, 0.0), 1.5), Standing::On(_)));
    assert_eq!(standing(&body, (-1.0, 0.0), 1.5), Standing::Outside);
    assert_eq!(standing(&body, (-1.0, 0.0), -1.0), Standing::Outside);
    assert_eq!(
        standing(&body, (1.9, 1.9), 2.9),
        Standing::Inside,
        "in a corner"
    );
}

/// **A ray that grazes an edge is thrown away and another cast**, and the
/// answer is the one the geometry says rather than the one a miscount would.
///
/// Both places here are chosen so that the *first* direction tried runs exactly
/// through an upright edge of the block, where a crossing is counted twice or
/// not at all. Without the retry the first would come back inside out and the
/// second would be a coin toss — and every other test in this file would still
/// pass, because none of them puts a ray anywhere near an edge.
///
/// Read in world terms: the block stands over `x ∈ [0, 2]`, `z ∈ [-2, 0]`,
/// `y ∈ [0, 3]`, the first cast runs along `(1, 2, 3)`, and both places are
/// half a step back along it from a corner of the block.
#[test]
fn a_ray_that_grazes_an_edge_is_thrown_away_and_another_cast() {
    let body = block(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], 3.0);

    // Inside, aimed at the upright edge over the profile's `(2, 0)` corner.
    assert_eq!(standing(&body, (1.5, 1.5), 0.5), Standing::Inside);
    // Outside, aimed at the upright edge over `(0, 0)`.
    assert_eq!(standing(&body, (-1.0, 3.0), -0.5), Standing::Outside);
}

/// **A bore is outside the body it is bored through**, which is what says the
/// count reads a face's holes as well as its outline.
///
/// A six-by-six block with a two-by-two bore through it. A place down the bore
/// is surrounded by material and is still outside it: every ray out of it
/// crosses the walls an even number of times.
#[test]
fn a_place_down_a_bore_is_outside_the_block_it_is_bored_through() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)]);
    sketch.outline(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]);
    let found = Arrangement::of(&sketch);
    let ring = found
        .faces()
        .iter()
        .position(|face| face.holes() == 1)
        .expect("the bore is a hole of the block");
    let body = Extrusion::new(&found, ring, Plane::GROUND, 5.0).body();

    assert_eq!(
        standing(&body, (3.0, 3.0), 2.5),
        Standing::Outside,
        "the bore"
    );
    assert_eq!(
        standing(&body, (1.0, 1.0), 2.5),
        Standing::Inside,
        "the wall"
    );
    assert_eq!(standing(&body, (3.0, 1.0), 2.5), Standing::Inside);
    // On the wall of the bore, which is a face like any other — and faces into
    // the bore, because that is the way out of the material.
    faces(&body, (2.0, 3.0), 2.5, DVec3::X);
    // Above the bore, level with the far end: clear of the solid either way.
    assert_eq!(standing(&body, (3.0, 3.0), 7.0), Standing::Outside);
}

/// A block grown the other way off its plane is sounded the same, and faces
/// the other way where it sits on the plane it was drawn on.
///
/// Which is worth asking separately: a body grown against its plane's normal
/// has every one of its faces wound the other way round, and a ray count that
/// leaned on winding rather than on crossings would answer backwards.
#[test]
fn a_block_grown_the_other_way_is_sounded_the_same() {
    let body = block(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)], -3.0);
    assert_eq!(standing(&body, (1.0, 1.0), -1.5), Standing::Inside);
    assert_eq!(standing(&body, (1.0, 1.0), 1.5), Standing::Outside);
    faces(&body, (1.0, 1.0), -3.0, DVec3::NEG_Y);
    faces(&body, (1.0, 1.0), 0.0, DVec3::Y);
}
