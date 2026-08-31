use super::*;
use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::Boolean;
use crate::solid::boolean::operation::Operation;
use crate::solid::build::builder::Extrusion;
use crate::solid::build::revolving::{Revolution, Sector};
use crate::solid::mesh::Mesher;
use crate::solid::named::Step;
use std::f64::consts::TAU;

const CUBE: Step = Step(1);
const TOOL: Step = Step(2);

/// The ground, moved `by` along its own normal.
fn raised(by: f64) -> Plane {
    Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal() * by,
        ..Plane::GROUND
    }
}

/// A block from `corners`, carried `deep` off `plane`, grown by `by`.
fn block(plane: Plane, corners: &[(f64, f64)], deep: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(corners);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, &[0], plane, deep, by).body()
}

/// A rod of `radius` about `at`, carried `deep` off `plane`.
fn rod(plane: Plane, at: DVec2, radius: f64, deep: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(at);
    sketch.add_circle(middle, radius);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, &[0], plane, deep, by).body()
}

/// A prism of `sides` about `at`, `radius` across the corners.
fn prism(plane: Plane, at: DVec2, radius: f64, sides: usize, deep: f64, by: Step) -> Body {
    let corners: Vec<(f64, f64)> = (0..sides)
        .map(|step| {
            let turn = TAU * step as f64 / sides as f64;
            (at.x + radius * turn.cos(), at.y + radius * turn.sin())
        })
        .collect();
    block(plane, &corners, deep, by)
}

/// **A pocket milled into a block leaves its top in slivers, and the merge
/// takes them back to one face.**
///
/// A cut is taken by whole *surfaces* — see `.notes/KERNEL.md` §7.4 — so each
/// of the eight walls of the tool divides the block's top from edge to edge,
/// and every piece but the pocket itself is kept. They lie on one surface,
/// carry one name and face one way, so §5 already calls the set of them one
/// face and the merge is what makes the body say so.
///
/// **And the body it leaves is the same solid.** `4·4·4` less the pocket, which
/// is an eight-sided prism of `½·8·r²·sin(τ/8) = 2√2` two deep — and every
/// figure of it exact, a block and a prism being planes throughout.
#[test]
fn a_pocket_milled_into_a_block_merges_its_top_back_to_one_face() {
    let cube = block(
        raised(0.0),
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
        CUBE,
    );
    let mill = prism(raised(2.0), DVec2::new(2.0, 2.0), 1.0, 8, 4.0, TOOL);
    let mut boolean = Boolean::default();
    let mut milled = Body::default();
    assert!(
        boolean.combine(&cube, &mill, Operation::Cut, &mut milled),
        "a block was refused its pocket",
    );

    let mut merging = Merging::default();
    let mut merged = Body::default();
    merging.merge(&milled, &mut merged);

    assert!(
        milled.topology().faces().count() > merged.topology().faces().count(),
        "the merge took nothing away",
    );
    assert_eq!(
        merged.topology().faces().count(),
        merged.names().count(),
        "a name is one face of the answer",
    );
    assert_eq!(
        merged.names().count(),
        milled.names().count(),
        "a name was lost or gained",
    );
    assert_eq!(
        merged.reckoning().genus,
        milled.reckoning().genus,
        "the merge moved what the body shuts in",
    );
    assert_eq!(
        merged.topology().lumps().count(),
        1,
        "a milled block is one piece",
    );

    // Planes throughout, so the mesh is the solid rather than a reading of it.
    let want = 64.0 - 2.0 * 2.0_f64.sqrt() * 2.0;
    let mut mesher = Mesher::default();
    let shut = mesher.volume(&merged, 1e-3);
    assert!(
        (shut - want).abs() < 1e-9,
        "the merged block shut in {shut}"
    );
    let had = mesher.volume(&milled, 1e-3);
    assert!(
        (shut - had).abs() < 1e-9,
        "the merge moved {} of it",
        shut - had
    );
}

/// **A hole through the merged face survives it**, which is what the
/// cancellation gives rather than asks: a stretch whose other side was dropped
/// has no twin and stays.
///
/// The same prism taken clean through the block: its eight walls divide the top
/// from edge to edge and every piece outside the hole is kept, so the merged
/// top is one face with the hole punched out of it — and the body is a ring.
///
/// **And the rim of the hole is still an edge**, which is the boundary the
/// prism's own walls need: it divides the top from a wall rather than one piece
/// of the top from another, so nothing cancels along it.
#[test]
fn a_hole_through_the_merged_face_survives_the_merge() {
    let cube = block(
        raised(0.0),
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
        CUBE,
    );
    let through = prism(raised(-1.0), DVec2::new(2.0, 2.0), 1.0, 8, 6.0, TOOL);
    let mut boolean = Boolean::default();
    let mut bored = Body::default();
    assert!(
        boolean.combine(&cube, &through, Operation::Cut, &mut bored),
        "a block was refused its hole",
    );

    let mut merging = Merging::default();
    let mut merged = Body::default();
    merging.merge(&bored, &mut merged);

    assert_eq!(merged.reckoning().genus, 1, "a block with a hole is a ring");
    assert_eq!(
        merged.topology().faces().count(),
        merged.names().count(),
        "a name is one face of the answer",
    );
    // The two flats hold one loop each round the block and one round the hole;
    // every wall holds one.
    let holed = merged
        .topology()
        .faces()
        .filter(|(_, face)| face.holes() == 1)
        .count();
    assert_eq!(holed, 2, "the hole was lost or doubled");

    let want = 64.0 - 2.0 * 2.0_f64.sqrt() * 4.0;
    let mut mesher = Mesher::default();
    let shut = mesher.volume(&merged, 1e-3);
    assert!((shut - want).abs() < 1e-9, "the ring shut in {shut}");
}

/// **A merge that would wrap its own surface is left alone**, which
/// `.notes/KERNEL.md` §4.4 forbids and a build's own split is there to keep
/// from happening.
///
/// A bore's wall is two faces of one cylinder, sharing a surface, a name and a
/// way to face — everything the merge groups by. Put back together they would
/// be one face covering a whole turn of the cylinder's own angle, which no face
/// may do. So the group is read for the wrap and left as it was.
#[test]
fn a_merge_that_would_wrap_its_own_surface_is_left_alone() {
    let cube = block(
        raised(0.0),
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
        CUBE,
    );
    let drill = rod(raised(-1.0), DVec2::new(2.0, 2.0), 1.0, 6.0, TOOL);
    let mut boolean = Boolean::default();
    let mut bored = Body::default();
    assert!(
        boolean.combine(&cube, &drill, Operation::Cut, &mut bored),
        "a block was refused its bore",
    );

    let mut merging = Merging::default();
    let mut merged = Body::default();
    merging.merge(&bored, &mut merged);

    // The wall of the bore, which is the one name of the answer with more than
    // one face to it.
    let wall = merged
        .names()
        .map(|named| merged.patches(named).count())
        .max()
        .expect("a bored block has faces");
    assert_eq!(wall, 2, "the bore's wall was merged into one face");
    assert_eq!(
        merged.reckoning(),
        bored.reckoning(),
        "the merge moved what the body shuts in",
    );
}

/// **A cut by a finer tool costs the answer nothing**, which is the whole of
/// what the merge is for: a boolean divides a wall by every surface that
/// reaches it, so a tool of *n* sides leaves a block's top in *n* or more
/// pieces — and the shape a reader means has the same faces however finely the
/// tool was drawn.
///
/// The counts are the shapes' own: a block bored by an eight-sided prism is 6
/// walls, 2 flats less the hole and 8 walls of the hole, which is 16. Each
/// count below is `6 + sides` the same way, and the pieces the boolean handed
/// back are in the thousands by the last of them.
#[test]
fn a_finer_tool_costs_the_merged_body_no_faces() {
    let mut merging = Merging::default();
    let mut mesher = Mesher::default();
    for sides in [4usize, 16, 64] {
        let cube = block(
            raised(0.0),
            &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
            4.0,
            CUBE,
        );
        let tool = prism(raised(-1.0), DVec2::new(2.0, 2.0), 1.0, sides, 6.0, TOOL);
        let mut boolean = Boolean::default();
        let mut split = Body::default();
        assert!(
            boolean.combine(&cube, &tool, Operation::Cut, &mut split),
            "a block was refused a {sides}-sided hole",
        );
        let mut merged = Body::default();
        merging.merge(&split, &mut merged);
        assert_eq!(
            merged.topology().faces().count(),
            6 + sides,
            "a {sides}-sided hole left the block more faces than it has",
        );
        let (whole, tidy) = (mesher.volume(&split, 1e-3), mesher.volume(&merged, 1e-3));
        assert!(
            (whole - tidy).abs() < 1e-9,
            "the merge moved {} of volume",
            whole - tidy,
        );
    }
}

/// **A group left with no boundary at all is left alone**, which is the wrap of
/// `.notes/KERNEL.md` §4.4 twice over and the one case no reading of a loop can
/// catch.
///
/// A circle spun a whole turn is a torus, and a build lays its wall in parts
/// with a seam between each — one surface, one name, one way to face, so the
/// merge groups every part of it. Every coedge of the group cancels, and what
/// the merge would put back together bounds nothing: a single face covering the
/// whole of a closed surface. There is no outline to measure, so the count of
/// merged loops is what says so.
#[test]
fn a_group_the_merge_would_leave_boundless_is_left_alone() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(3.0, 0.0));
    sketch.add_circle(middle, 1.0);
    let found = Arrangement::of(&sketch);
    let ring = Revolution::new(
        &found,
        &[0],
        Plane::GROUND,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        TOOL,
    )
    .body();
    let faces = ring.topology().faces().count();
    assert_eq!(faces, 6, "a whole turn lays its wall in six parts");

    let mut merging = Merging::default();
    let mut merged = Body::default();
    merging.merge(&ring, &mut merged);

    assert_eq!(
        merged.topology().faces().count(),
        faces,
        "the torus was merged into one face covering the whole of it",
    );
    assert_eq!(merged.reckoning(), ring.reckoning(), "the ring moved");
}
