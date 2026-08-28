use super::*;
use crate::Plane;
use crate::math::winding;
use crate::number::tolerance::EXACT;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use crate::solid::boolean::combining::Kept;
use crate::solid::build::builder::Extrusion;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::circle::Circle;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::ellipse::Ellipse;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::mesh::Mesher;
use crate::solid::named::{Named, Step};
use crate::solid::topology::edge::Edge;
use crate::solid::topology::face::Face;
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, SQRT_2, TAU};

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
    off(Plane::GROUND, by)
}

/// `plane`, moved `by` along its own normal.
fn off(plane: Plane, by: f64) -> Plane {
    Plane {
        origin: plane.origin + plane.normal() * by,
        ..plane
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
            let Surface::Natural(Natural::Plane(plane)) = kept.surface else {
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

/// A rod, and the name the wall it sweeps carries.
///
/// The name as well as the body, because half of what a boolean has to get
/// right about a pocket is whose face its wall is — and a circle's `Side` is
/// named by the entity of the *drawing*, which only whoever drew it knows.
///
/// Either operand: the tool that bores a block, and the body a second rod is
/// joined onto.
#[derive(Debug)]
struct Rod {
    body: Body,
    wall: Named,
}

/// A cylinder of `radius` about `at`, carried `deep` off `plane`, grown by `by`.
fn rod(plane: Plane, at: DVec2, radius: f64, deep: f64, by: Step) -> Rod {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(at);
    let ring = sketch.add_circle(middle, radius);
    let found = Arrangement::of(&sketch);
    Rod {
        body: Extrusion::new(&found, 0, plane, deep, by).body(),
        wall: by.grew(Grown::Side(Bound {
            of: Entity::Circle(ring),
            along: true,
        })),
    }
}

/// A unit rod up the cube's middle, standing clear of both its ends.
fn through() -> Rod {
    rod(raised(-1.0), DVec2::new(2.0, 2.0), 1.0, 6.0, TOOL)
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
        .filter(|face| matches!(face.surface, Surface::Natural(Natural::Cylinder(_))))
        .collect();
    assert_eq!(
        walls.len(),
        2,
        "the bore's wall came out as {} faces",
        walls.len()
    );
    for wall in &walls {
        let Surface::Natural(Natural::Cylinder(tube)) = wall.surface else {
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
            rod(raised(2.0), DVec2::new(2.0, 2.0), 1.0, 4.0, TOOL),
            Operation::Cut,
            64.0 - PI * 2.0,
        ),
        (
            "stood on the end",
            rod(raised(4.0), DVec2::new(2.0, 2.0), 1.0, 2.0, TOOL),
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
    let tool = rod(raised(2.0), DVec2::new(2.0, 2.0), 1.0, 4.0, TOOL);
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

/// **A flat milled down a shaft**, which is a plane parallel to a cylinder's
/// axis and was the last crossing refused for being one.
///
/// A ruling line is a cut at a constant angle in a parameter that *wraps*, so
/// which turn of it the face was laid out in decides whether the face is
/// divided at all — and a face may not wrap, so at most one turn falls inside
/// its own range. That is the whole of what `imprinted` needed and could not
/// ask; it asks now.
///
/// The slab reaches from `x = 1.5` outward and stands four deep off the ground,
/// so it takes a bite out of the middle of a rod running from `y = −1` to
/// `y = 5`. A chord half a unit off the centre of a unit circle leaves the minor
/// segment `r²·acos(d/r) − d·√(r² − d²)` = `π/3 − ½√¾`, which stands over the
/// four the slab is deep; above and below it the rod is round and whole.
///
/// Seventeen faces, and every one of them accounted: two caps, each cut in two
/// by the chord where the flat's plane crosses it; ten of wall, each half of the
/// cylinder cut by the ruling into two and by the slab's two ends into three,
/// less the one piece of each that the slab swallowed; the flat itself; and two
/// shoulders where the slab's ends stand inside the rod.
#[test]
fn a_flat_milled_down_a_shaft_leaves_the_segment_the_arithmetic_says() {
    let upright = through();
    let slab = block(
        Plane::GROUND,
        &[(1.5, -1.0), (5.0, -1.0), (5.0, 5.0), (1.5, 5.0)],
        4.0,
        TOOL,
    );
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(boolean.combine(&upright.body, &slab, Operation::Cut, &mut into));

    assert_eq!(into.topology().faces().count(), 17);
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");

    let minor = (0.5f64).acos() - 0.5 * (0.75f64).sqrt();
    let want = minor * 4.0 + PI * 2.0;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-4, 1e-5, 1e-6] {
        let slack = (2.0 / 3.0) * sagitta * TAU * 6.0 * 2.0;
        let off = mesher.volume(&into, sagitta) - want;
        assert!(
            off.abs() < slack,
            "the flatted shaft shut in {off} off {want} at a sagitta of {sagitta}",
        );
        assert!(
            off.abs() < last,
            "no closer at a sagitta of {sagitta}: {off}"
        );
        last = off.abs();
    }
}

/// A crossing no face's own parameters can write down is refused rather than
/// guessed at.
///
/// Two cylinders across each other meet in a quartic, which arrives as
/// [`Meeting::Algebraic`](crate::solid::meeting::Meeting::Algebraic) — a curve
/// this parameterizes nowhere, rather than one it parameterizes and cannot
/// carry. Nothing is left behind: a body half sewn is worse than none.
#[test]
fn a_crossing_no_face_can_carry_is_refused() {
    let upright = through();
    let across = rod(
        Plane {
            origin: Plane::GROUND.origin,
            x: Plane::GROUND.normal(),
            y: Plane::GROUND.x,
        },
        DVec2::ZERO,
        1.0,
        6.0,
        TOOL,
    );
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(!boolean.combine(&upright.body, &across.body, Operation::Cut, &mut into));
    assert!(into.is_empty(), "a refusal left half a body behind");
}

/// **A surface reaching no part of the other body cuts none of it**, which is
/// what keeps a wall at the far end of a model out of a face it never touches.
///
/// A body is divided by the other's *surfaces*, and a plane is unbounded where
/// the wall standing on it is not — so the slab's own plane crosses the rod's
/// cylinder in two ruling lines whether the slab is up against the rod or ten
/// above its far end. Read as pieces, that costs faces for nothing; the answer
/// is to ask how far the faces on a surface actually reach.
///
/// Ten faces, which is the rod's four and the slab's six with not one of them
/// cut — the stronger claim, a cut that divided nothing still showing here as
/// pieces. And `3.5 × 6 × 4` of slab beside `π·1²·6` of rod, standing clear so
/// that they add.
#[test]
fn a_surface_reaching_no_part_of_the_other_body_cuts_none_of_it() {
    let upright = through();
    let aloft = block(
        raised(10.0),
        &[(1.5, -1.0), (5.0, -1.0), (5.0, 5.0), (1.5, 5.0)],
        4.0,
        TOOL,
    );
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(
        boolean.combine(&upright.body, &aloft, Operation::Join, &mut into),
        "two solids a model's width apart were turned away",
    );
    assert_eq!(into.topology().faces().count(), 10);

    let want = 84.0 + PI * 6.0;
    let sagitta = 1e-6;
    let slack = (2.0 / 3.0) * sagitta * TAU * 6.0 * 2.0;
    let shut_in = Mesher::default().volume(&into, sagitta);
    assert!(
        (shut_in - want).abs() < slack,
        "the two together shut in {shut_in} rather than {want}",
    );
}

/// **Two rods run alongside each other join into one**, which is a boolean with
/// a round body on both sides of it and the strongest thing the ruling line
/// buys.
///
/// Cylinders whose axes are parallel meet in two lines, and a line on a cylinder
/// is a cut at a constant angle — so each wall is divided where the other tube
/// passes through it, and each cap loses the lens the other covers.
///
/// Grown by the two steps everything here is, neither of them a cube: what a
/// name has to tell apart is which feature made a face, not what shape it made.
/// Two unit rods a unit apart, two deep. What they cover between them is the
/// two circles less the lens they share once over: `2πr² − (2r²·acos(d/2r) −
/// (d/2)√(4r² − d²))`, which at `r = 1` and `d = 1` is `2π − (2·π/3 − ½√3)`.
/// Genus nought, because two overlapping rods shut in one lump with nothing
/// through it — a pair that met in a ring rather than a lens would not.
#[test]
fn two_rods_alongside_each_other_join_into_one() {
    let one = rod(Plane::GROUND, DVec2::ZERO, 1.0, 2.0, CUBE);
    let two = rod(Plane::GROUND, DVec2::new(1.0, 0.0), 1.0, 2.0, TOOL);
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(boolean.combine(&one.body, &two.body, Operation::Join, &mut into));

    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
    // Each tube's wall is still its own, and still one name over the pieces
    // §4.4 cut it into — the boolean took a bite out of each and renamed
    // neither.
    for wall in [one.wall, two.wall] {
        assert!(into.holds(wall), "{wall:?} was lost");
        assert_eq!(into.patches(wall).count(), 2, "{wall:?} came apart");
    }

    let lens = 2.0 * (0.5f64).acos() - 0.5 * (3.0f64).sqrt();
    let want = (TAU - lens) * 2.0;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-4, 1e-5, 1e-6] {
        let slack = (2.0 / 3.0) * sagitta * TAU * 2.0 * 4.0;
        let off = mesher.volume(&into, sagitta) - want;
        assert!(
            off.abs() < slack,
            "the pair shut in {off} off {want} at a sagitta of {sagitta}",
        );
        assert!(
            off.abs() < last,
            "no closer at a sagitta of {sagitta}: {off}"
        );
        last = off.abs();
    }
}

/// **A pipe mitred across**, which is a plane meeting a cylinder obliquely and
/// was the last crossing `imprinted` could not carry.
///
/// The curve is an ellipse, and it needs writing down twice over: on the
/// cutting plane it is an ellipse in that plane's own parameters, which is what
/// [`Cut::Round`](super::splitting::cut::Cut) became; on the cylinder it is a *graph
/// over the angle*, `v = level + swing·cos(θ − phase)`, which is
/// [`Cut::Wave`](super::splitting::cut::Cut).
///
/// A unit rod up `+y`, cut by a plane through its axis at half height leaning
/// forty-five degrees. What comes back is four faces — the base, the two halves
/// of the wall §4.4 splits it into, and the elliptical lid — over four
/// vertices and six edges, which is genus nought.
///
/// **The lid's curve is asserted exactly.** Leaning at forty-five degrees
/// stretches the circle by `1/cos 45° = √2` along the lean and leaves it alone
/// across, so the ellipse is `√2` by `1`, centred where the plane crosses the
/// axis and square to the plane's own normal. Both halves of it sweep half a
/// turn of that ellipse's frame, one each way round, which is what says the two
/// walls meet it along one edge apiece rather than sharing one.
#[test]
fn a_pipe_mitred_across_keeps_the_ellipse_exact() {
    let upright = rod(Plane::GROUND, DVec2::ZERO, 1.0, 6.0, CUBE);
    let leaning = Plane {
        origin: DVec3::new(0.0, 3.0, 0.0),
        x: DVec3::X,
        y: DVec3::new(0.0, 1.0, -1.0).normalize(),
    };
    let lid = block(
        leaning,
        &[(-5.0, -5.0), (5.0, -5.0), (5.0, 5.0), (-5.0, 5.0)],
        10.0,
        TOOL,
    );
    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(boolean.combine(&upright.body, &lid, Operation::Cut, &mut into));

    let topology = into.topology();
    assert_eq!(topology.faces().count(), 4);
    assert_eq!(topology.edges().count(), 6);
    assert_eq!(topology.vertices().count(), 4);
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");

    // The rod's wall came through as its own two halves, on the tool's own
    // exact cylinder — the mitre took a bite out of each and renamed neither.
    assert_eq!(into.patches(upright.wall).count(), 2);

    let want = Ellipse {
        axis: Axis::new(
            leaning.origin,
            leaning.normal(),
            DVec3::new(0.0, 1.0, -1.0).normalize(),
        ),
        major: SQRT_2,
        minor: 1.0,
    };
    let lid: Vec<&Edge> = topology
        .edges()
        .map(|(_, edge)| edge)
        .filter(|edge| matches!(edge.curve, Curve::Ellipse(_)))
        .collect();
    assert_eq!(lid.len(), 2, "the mitre came back as {} arcs", lid.len());
    for edge in &lid {
        let Curve::Ellipse(oval) = edge.curve else {
            unreachable!("filtered above")
        };
        assert!(
            oval.axis.origin.abs_diff_eq(want.axis.origin, 1e-12)
                && oval.axis.direction.abs_diff_eq(want.axis.direction, 1e-12)
                && (oval.major - want.major).abs() < 1e-12
                && (oval.minor - want.minor).abs() < 1e-12,
            "an arc landed on {oval:?} rather than {want:?}",
        );
        let swept = edge.bounds[1] - edge.bounds[0];
        assert!(
            (swept.abs() - PI).abs() < 1e-12,
            "an arc swept {swept} rather than half the frame",
        );
    }
    // The two together are the whole ellipse, once round — one loop of the lid
    // walked in two pieces, so they run the *same* way and their sweeps add to
    // a whole turn of the frame. Two halves running against each other would be
    // the lid covering half of itself twice.
    let round: f64 = lid.iter().map(|edge| edge.bounds[1] - edge.bounds[0]).sum();
    assert!(
        (round.abs() - TAU).abs() < 1e-12,
        "the mitre came round by {round} rather than a whole turn",
    );

    // **And it shuts in what the arithmetic says.** A cylinder cut by a plane
    // through its axis holds `πr²` times the height where the axis crosses it,
    // the two halves of the wedge above and below cancelling exactly — three
    // here, so `3π`. Chorded, so it reads short by about the wall inscribed in
    // itself: two thirds of the sagitta over the circumference, along the
    // height, which is the same bound the round tool above is held to.
    let want = 3.0 * PI;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-2, 1e-3, 1e-4] {
        let off = mesher.volume(&into, sagitta) - want;
        let slack = (2.0 / 3.0) * sagitta * TAU * 3.0;
        assert!(
            off < 0.0 && -off < slack,
            "the mitre shut in {off} off {want} at a sagitta of {sagitta}",
        );
        assert!(-off < last, "no closer at a sagitta of {sagitta}: {off}");
        last = -off;
    }
}

// Already inside a `cfg(test)` module, so it needs no gate of its own.
mod matrix;

/// **Two equal cylinders on crossing axes intersect in the Steinmetz solid**,
/// whose volume is exactly `16r³/3`.
///
/// The analytic cross-check `.notes/KERNEL.md` M5 owes, and it catches nearly
/// every error there is to make: a wall kept where it should be cut, an ellipse
/// walked the wrong way round, a region classified inside out, a shell sewn
/// with a seam left open — each of them moves this number, and none of them
/// moves it to something else that is right.
///
/// **Why it is a cross-check and not a restatement.** Every other volume here
/// is the arithmetic of the shapes that were put together, worked out the same
/// way the kernel would. This one is a classical closed form with no cylinder
/// in it at all: the bicylinder is `16r³/3` by Archimedes, where a sphere of
/// the same radius is `4πr³/3` — so the answer is `4/π` of a ball, and nothing
/// about how a boolean works could produce it by accident.
///
/// Two unit rods four deep, one up the world's `y` and one along its `z`, each
/// standing a radius clear of the other at both ends so no cap plays any part.
/// Their walls meet in two ellipses, which is the one reducible entry
/// cylinder-meets-cylinder has (§7.3), so nothing here waits on the algebraic
/// route.
///
/// Genus nought: a bicylinder is a ball with corners, not a ring.
#[test]
fn two_crossing_rods_intersect_in_the_steinmetz_solid() {
    let upright = rod(raised(-2.0), DVec2::ZERO, 1.0, 4.0, CUBE);
    let along = rod(off(Plane::FRONT, -2.0), DVec2::ZERO, 1.0, 4.0, TOOL);

    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(
        boolean.combine(&upright.body, &along.body, Operation::Intersect, &mut into),
        "two crossing rods were turned away",
    );

    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
    // Each rod keeps its own wall, in the two pieces the other cuts it into.
    for wall in [upright.wall, along.wall] {
        assert!(into.holds(wall), "{wall:?} was lost");
        assert_eq!(into.patches(wall).count(), 2, "{wall:?} came apart wrongly");
    }

    let want = 16.0 / 3.0;
    let mut mesher = Mesher::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-4, 1e-5, 1e-6] {
        // Both walls are chorded round their whole turn, and each covers two
        // units of the other's diameter: what a chord cuts off goes as two
        // thirds of the sagitta times the area it spans.
        let slack = (2.0 / 3.0) * sagitta * TAU * 2.0 * 2.0;
        let off = (mesher.volume(&into, sagitta) - want).abs();
        assert!(off < slack, "{off} off {want} at a sagitta of {sagitta}");
        assert!(off < last, "{sagitta} read no nearer than the last");
        last = off;
    }
}

/// **A cut that removes everything leaves nothing, and says it did.**
///
/// The case `.notes/KERNEL.md` M5 owes, and the one a modeller reaches by
/// typing a depth one digit too long. A unit block wholly inside a larger one,
/// cut the wrong way round: what is left of the small block after the big one
/// is taken out of it is nothing at all.
///
/// **Nothing is an answer and not a refusal.** The combine comes back true —
/// it knew what to do and did it — and what it wrote is a body with no faces.
/// A caller that read the refusal instead would show the tool where the model
/// used to be, which is the one thing a total cut must not look like.
#[test]
fn a_cut_that_removes_everything_leaves_a_body_with_nothing_in_it() {
    let small = block(
        Plane::GROUND,
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
        1.0,
        CUBE,
    );
    let large = block(
        raised(-1.0),
        &[(-1.0, -1.0), (2.0, -1.0), (2.0, 2.0), (-1.0, 2.0)],
        3.0,
        TOOL,
    );

    let mut boolean = Boolean::default();
    let mut into = Body::default();
    assert!(
        boolean.combine(&small, &large, Operation::Cut, &mut into),
        "a cut that swallows its whole body was refused",
    );
    assert_eq!(into.topology().faces().count(), 0, "something was left");
    assert_eq!(Mesher::default().volume(&into, 1e-6), 0.0);

    // And the other way round the same pair is the small block itself, so the
    // fixture is one that genuinely overlaps rather than one that misses.
    let mut kept = Body::default();
    assert!(boolean.combine(&small, &large, Operation::Intersect, &mut kept));
    assert_eq!(Mesher::default().volume(&kept, 1e-6), 1.0);
}
