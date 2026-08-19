use super::*;
use crate::Plane;
use crate::math::winding;
use crate::number::tolerance::EXACT;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use crate::solid::build::extrusion::Extrusion;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::grown::Grown;
use crate::solid::mesh::Mesher;
use crate::solid::named::{Named, Step};
use crate::solid::topology::edge::Edge;
use std::f64::consts::{PI, TAU};

/// The two steps the blocks below are grown by.
///
/// Two, and that is the point: a boolean's answer holds faces of both operands,
/// and which feature grew a face is half of what names it — see
/// [`Named`](crate::solid::named::Named). Both bodies calling their base
/// `Grown::Base` is exactly the collision the other half is for.
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
///
/// Raised a unit off the ground and only two deep, so that not one of its faces
/// is coplanar with one of the cube's: two solids meeting flush is a case of its
/// own, and this is not it.
fn corner() -> Body {
    block(
        raised(1.0),
        &[(3.0, 3.0), (5.0, 3.0), (5.0, 5.0), (3.0, 5.0)],
        2.0,
        TOOL,
    )
}

/// The same two-by-two-by-two block, sitting on the ground the cube stands on.
///
/// Which puts the two bases on one plane, both with their material above it —
/// the case [`corner`] is raised a unit to avoid.
fn flush() -> Body {
    block(
        Plane::GROUND,
        &[(3.0, 3.0), (5.0, 3.0), (5.0, 5.0), (3.0, 5.0)],
        2.0,
        TOOL,
    )
}

/// A two-by-two-by-two block standing on the cube's far end.
///
/// Which puts one of its faces on one of the cube's the other way up: material
/// below the plane they share and material above it.
fn stacked() -> Body {
    block(
        raised(4.0),
        &[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)],
        2.0,
        TOOL,
    )
}

/// How much surface a combine kept.
fn covered(combining: &Combining) -> f64 {
    combining
        .kept()
        .iter()
        .map(|kept| {
            kept.loops
                .clone()
                .map(|run| winding::swept(combining.loops().get(run)) / 2.0)
                .sum::<f64>()
        })
        .sum()
}

/// **Each of the three operations keeps exactly the surface it should**, and
/// the three answers are hand-computed from one pair of blocks.
///
/// A four-cube and a two-cube overlapping in a one-by-one-by-two notch at a
/// corner. The cube has 96 of surface and the tool 24; the notch takes 4 off
/// the cube and puts 6 of the tool's inside it.
///
/// - the cut keeps `92 + 6 = 98` — the cube less what the tool swallowed, plus
///   the walls of the notch;
/// - the join keeps `92 + 18 = 110` — the same, plus the whole of the tool that
///   stood clear;
/// - the intersection keeps `4 + 6 = 10`, which is the surface of the notch
///   itself: two ends of one, four sides of two.
///
/// Read as areas rather than as counts, because how many *pieces* each face
/// comes apart into is the splitter's business and no part of the answer — see
/// the note on cutting further than necessary.
#[test]
fn the_three_operations_keep_the_surface_each_of_them_should() {
    let (cube, corner) = (cube(), corner());
    let mut combining = Combining::default();
    for (doing, want) in [
        (Operation::Cut, 98.0),
        (Operation::Join, 110.0),
        (Operation::Intersect, 10.0),
    ] {
        assert!(combining.combine(&cube, &corner, doing), "{doing:?}");
        let covered = covered(&combining);
        assert!(
            (covered - want).abs() < 1e-9,
            "{doing:?} kept {covered} of surface rather than {want}",
        );
        // Nothing kept covers nothing: a region of no width is a region that
        // was never there.
        assert!(
            combining.kept().iter().all(|kept| !kept.loops.is_empty()),
            "{doing:?} kept a region with no outline",
        );
    }
}

/// **A cut turns the tool's faces over and leaves the cube's alone**, which is
/// what makes the wall of a pocket face into it.
#[test]
fn a_cut_turns_the_tools_faces_over_and_leaves_the_cubes_alone() {
    let (cube, corner) = (cube(), corner());
    let mut combining = Combining::default();
    assert!(combining.combine(&cube, &corner, Operation::Cut));

    // The tool's own top cap faces up; the floor of the notch it leaves is the
    // same surface facing down. Found by the one place at the top of the notch
    // — a unit square three deep over the cube's far corner.
    let roof = combining
        .kept()
        .iter()
        .find(|kept| {
            let Surface::Plane(plane) = kept.surface else {
                return false;
            };
            (plane.origin - (Plane::GROUND.origin + Plane::GROUND.normal() * 3.0)).length() < 1e-9
        })
        .expect("the notch has a roof");
    assert!(!roof.outward, "the roof of a notch faces down into it");

    // And the cube's own base came through whole and unturned: the tool stands
    // clear of it, so every one of the pieces it was cut into is kept, they
    // still add up to the sixteen it always covered, and not one of them faces
    // into the material.
    let base: Vec<&Kept> = combining
        .kept()
        .iter()
        .filter(|kept| kept.name == CUBE.grew(Grown::Base) && !kept.outward)
        .collect();
    let covered: f64 = base
        .iter()
        .flat_map(|kept| kept.loops.clone())
        .map(|run| winding::swept(combining.loops().get(run)) / 2.0)
        .sum();
    assert!(
        (covered - 16.0).abs() < 1e-9,
        "the cube's base came through as {covered} of the sixteen it covers",
    );
}

/// **A body with nothing in it takes nothing away and adds nothing**, which is
/// what an extrusion of no depth comes to and what a document will hand this
/// on every frame of a depth being typed.
#[test]
fn combining_with_a_body_that_holds_nothing_leaves_the_other_alone() {
    let cube = cube();
    let nothing = block(
        Plane::GROUND,
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        0.0,
        TOOL,
    );
    assert!(nothing.is_empty());

    let mut combining = Combining::default();
    for (doing, want) in [
        (Operation::Cut, 96.0),
        (Operation::Join, 96.0),
        (Operation::Intersect, 0.0),
    ] {
        assert!(combining.combine(&cube, &nothing, doing), "{doing:?}");
        let covered = covered(&combining);
        assert!((covered - want).abs() < 1e-9, "{doing:?} kept {covered}");
    }
    // And the other way round, where the empty one is the first named: a join
    // is the same either way, and a cut takes nothing out of nothing.
    assert!(combining.combine(&nothing, &cube, Operation::Join));
    assert!((covered(&combining) - 96.0).abs() < 1e-9);
    assert!(combining.combine(&nothing, &cube, Operation::Cut));
    assert!(covered(&combining).abs() < 1e-9);
}

/// **Held flush against each other, exactly one of the two faces that meet is
/// kept** — and whether it is kept at all turns on which side each body holds
/// its material.
///
/// The same corner overlap as above in two placements. Dropped onto the cube's
/// own base plane, the two bases are one plane with material above both of
/// them; stood on the cube's far end, the square the two share has material
/// below it and material above. Surface totals rather than volumes, because
/// they are what count the shared square: a join keeping both copies of it
/// reads a unit high and keeping neither reads a unit low, where a sewn body
/// would only refuse.
///
/// Flush, sharing a base — the square covers 1:
///
/// - the join keeps `92 + 18 = 110`, the same as the raised block: the square
///   is on the union's skin and exactly one of the two copies of it is;
/// - the cut keeps `91 + 5 = 96` — the square is buried on neither side, so the
///   cube's base gives it up along with the two wall pieces the notch
///   swallowed, and the notch is left open at the base with two walls and a
///   roof where the raised block's had two walls, a roof and a floor. That this
///   comes to the cube's own 96 is arithmetic rather than the cube coming
///   through untouched: what it gave up and what the tool put back are the
///   same 5;
/// - the intersection keeps the notch's own 10, floored now by one body's base
///   rather than by two.
///
/// Face to face, one standing on the other — the square covers 4:
///
/// - the join keeps `96 − 4 + 24 − 4 = 112`: neither copy survives, the union
///   having material on both sides of the square;
/// - the cut keeps the cube's own 96 whole, the tool standing entirely clear of
///   its material — which is the placement where dropping both would be wrong;
/// - the intersection keeps nothing, two solids touching sharing no volume.
#[test]
fn two_solids_held_flush_keep_one_of_the_two_faces_that_meet() {
    let cube = cube();
    let mut combining = Combining::default();
    for (placing, tool, wants) in [
        ("sharing a base", flush(), [96.0, 110.0, 10.0]),
        ("face to face", stacked(), [96.0, 112.0, 0.0]),
    ] {
        let doings = [Operation::Cut, Operation::Join, Operation::Intersect];
        for (doing, want) in doings.into_iter().zip(wants) {
            assert!(
                combining.combine(&cube, &tool, doing),
                "{placing}, {doing:?}"
            );
            let covered = covered(&combining);
            assert!(
                (covered - want).abs() < 1e-9,
                "{placing}, {doing:?} kept {covered} of surface rather than {want}",
            );
        }
    }
}

/// A round tool, and the name the wall it sweeps carries.
///
/// The name as well as the body, because half of what a boolean has to get
/// right about a pocket is whose face its wall is — and a circle's `Side` is
/// named by the entity of the *drawing*, which only whoever drew it knows.
#[derive(Debug)]
struct Rod {
    body: Body,
    wall: Named,
}

/// A cylinder of `radius` about `at`, carried `deep` off `plane`.
fn rod(plane: Plane, at: DVec2, radius: f64, deep: f64) -> Rod {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(at);
    let ring = sketch.add_circle(middle, radius);
    let found = Arrangement::of(&sketch);
    Rod {
        body: Extrusion::new(&found, 0, plane, deep, TOOL).body(),
        wall: TOOL.grew(Grown::Side(Bound {
            of: Entity::Circle(ring),
            along: true,
        })),
    }
}

/// A unit rod up the cube's middle, standing clear of both its ends.
fn through() -> Rod {
    rod(raised(-1.0), DVec2::new(2.0, 2.0), 1.0, 6.0)
}

/// The circle a bore of [`through`] leaves in the cube's face `by` along.
fn rim(by: f64) -> Circle {
    Circle {
        axis: Axis::new(
            raised(by).origin + DVec3::new(2.0, 0.0, -2.0),
            Plane::GROUND.normal(),
            Plane::GROUND.x,
        ),
        radius: 1.0,
    }
}

/// **A block bored through comes out one solid with a hole through it**, and
/// the hole is a circle rather than the seventy-one flats it was classified as.
///
/// The case the whole of M5 was arranged around: a closed imprint has no corner
/// of its own to begin at, and has to be split exactly where the wall meeting it
/// is already split, or the rim of the hole and the rim of the wall are two
/// circles with four vertices between them and no edge in common — see
/// [`Sewing::encircle`](super::sewing::Sewing::encircle).
///
/// Counted rather than merely valid. Eight faces: the cube's six, two of them
/// now with a rim inside them, and the two the bore's wall is split into. Twelve
/// vertices: the cube's eight and two on each rim. Eighteen edges: the cube's
/// twelve, two arcs round each rim, and the two upright seams the wall is split
/// at. Ten loops, the two bored faces having an outline and a hole each. Which
/// is `12 − 18 + 2(8) − 10 − 2 + 2G = 0`, so `G = 1` — a solid with a hole
/// through it, and the one reading that tells it from a block with two dents.
#[test]
fn a_block_bored_through_comes_out_a_solid_with_a_hole_through_it() {
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(boolean.combine(&cube(), &through().body, Operation::Cut, &mut into));

    let topology = into.topology();
    assert_eq!(topology.faces().count(), 8);
    assert_eq!(topology.edges().count(), 18);
    assert_eq!(topology.vertices().count(), 12);
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 1, "{reckoning:?}");

    // **Exact, and that is the whole bargain.** Nothing the classification
    // chorded reached the body: both halves of the wall lie on the tool's own
    // cylinder, radius and axis and all.
    let walls: Vec<&Face> = topology
        .faces()
        .map(|(_, face)| face)
        .filter(|face| matches!(face.surface, Surface::Cylinder(_)))
        .collect();
    assert_eq!(
        walls.len(),
        2,
        "the bore's wall came out as {} faces",
        walls.len()
    );
    for wall in &walls {
        let Surface::Cylinder(tube) = wall.surface else {
            unreachable!("filtered above")
        };
        assert_eq!(tube.radius, 1.0);
        assert_eq!(tube.axis.origin, DVec3::new(2.0, -1.0, -2.0));
        assert_eq!(tube.axis.direction, Plane::GROUND.normal());
        assert!(!wall.outward, "the wall of a bore faces into the material");
    }
    assert!(topology.faces().all(|(_, face)| face.tolerance == EXACT));

    // Four arcs, two on each rim, each a half turn of the tool's own circle.
    // The *sweep* and not the ends: two places on a circle say nothing about
    // which of the two ways round between them an edge goes, and a rim read as
    // a pair of chords would have the same two ends.
    let arcs: Vec<&Edge> = topology
        .edges()
        .map(|(_, edge)| edge)
        .filter(|edge| matches!(edge.curve, Curve::Circle(_)))
        .collect();
    assert_eq!(arcs.len(), 4, "the rims came out as {} arcs", arcs.len());
    for edge in &arcs {
        let Curve::Circle(circle) = edge.curve else {
            unreachable!("filtered above")
        };
        assert!(
            circle == rim(0.0) || circle == rim(4.0),
            "an arc landed on {circle:?}",
        );
        let swept = edge.bounds[1] - edge.bounds[0];
        assert!(
            (swept.abs() - PI).abs() < 1e-12,
            "an arc swept {swept} rather than half a turn",
        );
    }
    // Two each way round: a rim is walked one way by the face it is a hole in
    // and the other by the wall, and four arcs all running the same way would
    // be a rim covered twice and a rim covered not at all.
    assert_eq!(
        arcs.iter()
            .filter(|edge| edge.bounds[1] > edge.bounds[0])
            .count(),
        2,
        "the four arcs do not run two each way round",
    );
}

/// **What a round tool leaves, held against the arithmetic**, over the four
/// placements one can take against a block.
///
/// A cube of 64 and a unit rod. Bored through, a cut takes `π·1²·4`; stopped
/// half way, `π·1²·2`; stood on the far end, a join adds `π·1²·2`; and the
/// intersection of the through rod with the cube is the piece of rod inside it,
/// `π·1²·4` and nothing besides.
///
/// **Read at three sagittas**, which is what says the wall is a cylinder rather
/// than a very fine prism. A mesh inscribes its polygon in the circle, so it
/// under-covers a disc by about `⅔·s·2πr` — a hole reads *large* by that much
/// over its depth and a boss reads *small*. Ten times finer has to be ten times
/// closer; a single reading at a single sagitta would pass just as well against
/// a body whose wall really was the polygon.
#[test]
fn a_round_tool_leaves_the_volume_the_arithmetic_says() {
    let cube = cube();
    let cases = [
        ("bored through", through(), Operation::Cut, 64.0 - PI * 4.0),
        (
            "stopped half way",
            rod(raised(2.0), DVec2::new(2.0, 2.0), 1.0, 4.0),
            Operation::Cut,
            64.0 - PI * 2.0,
        ),
        (
            "stood on the end",
            rod(raised(4.0), DVec2::new(2.0, 2.0), 1.0, 2.0),
            Operation::Join,
            64.0 + PI * 2.0,
        ),
        ("kept as a rod", through(), Operation::Intersect, PI * 4.0),
    ];
    let mut boolean = Boolean::default();
    let mut mesher = Mesher::default();
    let mut into = Body::default();
    for (placing, tool, doing, want) in cases {
        assert!(
            boolean.combine(&cube, &tool.body, doing, &mut into),
            "{placing}",
        );
        let mut last = f64::INFINITY;
        for sagitta in [1e-4, 1e-5, 1e-6] {
            // Two rims' worth of chording over four of depth, doubled — a
            // bound on what the mesh gives up, not a prediction of it.
            let slack = (2.0 / 3.0) * sagitta * TAU * 4.0 * 2.0;
            let off = mesher.volume(&into, sagitta) - want;
            assert!(
                off.abs() < slack,
                "{placing} shut in {off} off {want} at a sagitta of {sagitta}",
            );
            assert!(
                off.abs() < last,
                "{placing} came no closer at a sagitta of {sagitta}: {off}",
            );
            last = off.abs();
        }
    }
}

/// **The wall of a pocket is the tool's own face, turned over**, which is the
/// whole of what a cut does with the body it takes away with.
///
/// One name over both halves of it, because a wall is named by the curve it was
/// swept off and not by how §4.4 had to cut it — see
/// [`Grown::Side`](crate::solid::grown::Grown). The cube's own faces keep their
/// names through all of it: a bore takes a bite out of one of them, and a face
/// with a hole in it is the same face.
#[test]
fn a_pocket_takes_its_wall_from_the_tool_and_turns_it_over() {
    let tool = rod(raised(2.0), DVec2::new(2.0, 2.0), 1.0, 4.0);
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(boolean.combine(&cube(), &tool.body, Operation::Cut, &mut into));

    // Turned over, said as a comparison rather than as a sign: which way
    // `outward` reads is a convention, and what a cut does to a tool's face is
    // the other one, whichever that is. The floor is the tool's own *near* end
    // — this rod is driven up through the cube's top, so what stops the pocket
    // is where the rod began.
    let facing = |body: &Body, named| -> Vec<bool> {
        body.patches(named).map(|(_, face)| face.outward).collect()
    };
    for named in [tool.wall, TOOL.grew(Grown::Base)] {
        assert!(into.holds(named), "the pocket has no {named:?}");
        let was = facing(&tool.body, named);
        assert!(!was.is_empty(), "the tool has no {named:?} to turn over");
        assert_eq!(
            facing(&into, named),
            was.iter().map(|it| !it).collect::<Vec<bool>>(),
            "{named:?} came through the cut facing the way it went in",
        );
    }
    assert_eq!(
        into.patches(tool.wall).count(),
        2,
        "the pocket's wall is one face or three",
    );

    // The cube's far end is the face the pocket was sunk into, and it is one
    // face with a hole in it rather than pieces.
    let far = CUBE.grew(Grown::Far);
    assert!(into.holds(far), "{far:?} was lost");
    assert_eq!(into.patches(far).count(), 1, "the bored face came apart");
    assert_eq!(
        into.topology()
            .loops_of(into.patches(far).next().expect("it is held").1)
            .count(),
        2,
        "the bored face has no hole in it",
    );
}

/// A crossing no face's own parameters can write down is refused rather than
/// guessed at — and it is that crossing that is refused, not roundness.
///
/// Two of them, and both are shapes a modeller will ask for. A plane along a
/// cylinder's axis meets it in two ruling lines, which are cuts at a constant
/// angle in a parameter that *wraps* — see [`imprinted`]. Two cylinders across
/// each other meet in a quartic, which arrives as
/// [`Meeting::Algebraic`](crate::solid::meeting::Meeting::Algebraic). Neither
/// has anything to do with the bore above, which is round from end to end and
/// comes out exact.
#[test]
fn a_crossing_no_face_can_carry_is_refused() {
    let upright = through();
    let mut boolean = Boolean::default();
    let mut into = Body::default();

    // A slab whose side runs up the rod, half way across it.
    let slab = block(
        Plane::GROUND,
        &[(1.5, -1.0), (5.0, -1.0), (5.0, 5.0), (1.5, 5.0)],
        4.0,
        TOOL,
    );
    assert!(!boolean.combine(&upright.body, &slab, Operation::Cut, &mut into));
    assert!(into.is_empty(), "a refusal left half a body behind");

    // The same rod laid on its side, driven through the standing one.
    let across = rod(
        Plane {
            origin: Plane::GROUND.origin,
            x: Plane::GROUND.normal(),
            y: Plane::GROUND.x,
        },
        DVec2::ZERO,
        1.0,
        6.0,
    );
    assert!(!boolean.combine(&upright.body, &across.body, Operation::Cut, &mut into));
    assert!(into.is_empty());
}

// Already inside a `cfg(test)` module, so it needs no gate of its own.
mod matrix;
