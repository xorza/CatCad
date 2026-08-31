//! What a rounding makes of a body, and what it will not make of one.
//!
//! **Volumes, and they are hand-computed rather than compared.** A blend of
//! radius `r` down an edge of length `l` between two faces meeting square takes
//! the corner the cylinder does not fill — `(1 − π/4)·r²·l` — off a convex edge
//! and puts the same back into a concave one. Nothing about that reading
//! depends on how the answer was built, which is what makes it worth asserting.

use super::*;
use crate::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::Boolean;
use crate::solid::boolean::operation::Operation;
use crate::solid::build::builder::Extrusion;
use crate::solid::mesh::Mesher;
use glam::{DVec2, DVec3};
use std::f64::consts::PI;

/// The step the block below is grown by, and the one the blends are.
///
/// Two, and that is the point: a blend answers to
/// [`Grown::Rounded`](crate::Grown) and so does the blend of the next rounding,
/// so what tells the two apart is the step — see
/// [`Named`](crate::solid::named::Named).
const SOLID: Step = Step(1);
const ROUND: Step = Step(2);

/// How finely a body here is meshed to read its volume back.
const SAGITTA: f64 = 1e-4;

/// How far a volume read off a mesh may stand from the arithmetic.
///
/// **The chording, and it lands on whichever side of the cylinder the material
/// is.** A chorded arc stands inside the arc, so a blend cut into a convex edge
/// reads a shade small and one filled into a concave edge a shade large — which
/// is what the two rows below show, in opposite directions.
///
/// **And it goes as [`SAGITTA`], measured rather than derived.** The widest row
/// is the bored one, which carries a whole cylinder's worth of chords as well:
/// it reads `1.1e-3` at this sagitta and `8.8e-3` at ten times it, linear over
/// the decade — which is what says the reading is the chording and not a body
/// of the wrong shape. A blend row on its own reads `4.1e-4`, a twentieth of
/// one per cent of the corner it asserts about.
const CLOSES: f64 = 2e-3;

/// A block from `corners`, carried `deep` off `plane`.
fn block(plane: Plane, corners: &[(f64, f64)], deep: f64) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(corners);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, &[0], plane, deep, SOLID).body()
}

/// The four-by-four-by-four block most rows here are taken off.
fn cube() -> Body {
    block(
        Plane::GROUND,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
    )
}

/// The pair of names on the edge running between `here` and `there`.
///
/// **Found by where it stands rather than by counting faces**, which is what
/// keeps a row here readable: a caller of a rounding names an edge by the two
/// faces it divides, and a test that worked those names out from the order the
/// builder emits them would be asserting about the builder.
fn between(body: &Body, here: DVec3, there: DVec3) -> [Named; 2] {
    let topology = body.topology();
    let (_, edge) = topology
        .edges()
        .find(|(_, edge)| {
            let ends = [edge.from, edge.to].map(|end| topology.vertex(end).at);
            (ends[0].abs_diff_eq(here, PLACED) && ends[1].abs_diff_eq(there, PLACED))
                || (ends[0].abs_diff_eq(there, PLACED) && ends[1].abs_diff_eq(here, PLACED))
        })
        .expect("the body has an edge between those two places");
    edge.between.map(|face| topology.face(face).name)
}

/// How much a blend of `radius` down an edge `long` takes off a square corner,
/// or puts back into one.
fn corner(radius: f64, long: f64) -> f64 {
    (1.0 - PI / 4.0) * radius * radius * long
}

/// How much `body` shuts in.
fn volume(body: &Body) -> f64 {
    Mesher::default().volume(body, SAGITTA)
}

/// How many of a body's edges are flagged as no crease.
fn smooth(body: &Body) -> usize {
    body.topology()
        .edges()
        .filter(|(_, edge)| edge.artificial)
        .count()
}

/// **A blend down one convex edge takes the corner and nothing else**, which is
/// the whole of what a fillet is asked to be.
///
/// The block is four across and its top edge four long, so a unit blend takes
/// `(1 − π/4)·4` off sixty-four. Everything else it does is topology: two
/// corners swallowed and four raised, one edge gone and four put in, and one
/// face more than the block had.
#[test]
fn a_block_with_one_edge_rounded_loses_the_corner_the_arithmetic_says() {
    let cube = cube();
    let along = [between(
        &cube,
        DVec3::new(0.0, 4.0, 0.0),
        DVec3::new(4.0, 4.0, 0.0),
    )];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(&Round::new(&along, 1.0, ROUND), &cube, &mut into),
        "a unit blend down a four-long edge of a four-across block was refused",
    );

    let want = 64.0 - corner(1.0, 4.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the rounded block shuts in {} where {want} is the corner taken off sixty-four",
        volume(&into),
    );
    assert_eq!(
        into.names().count(),
        7,
        "the block's six faces and the blend"
    );
    assert!(
        into.holds(ROUND.grew(Grown::Rounded(0))),
        "the blend does not answer to the pick that asked for it",
    );
    assert!(
        into.exact(),
        "a blend on two planes is a cylinder and exact"
    );

    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "a rounded block is still a ball");
    let topology = into.topology();
    assert_eq!(topology.faces().count(), 7, "one face per name and no more");
    assert_eq!(
        topology.edges().count(),
        15,
        "the block's twelve, less the one rounded, and four round the blend",
    );
    assert_eq!(
        smooth(&into),
        2,
        "the two rulings the blend runs out along, and nothing else",
    );
}

/// **The blend stands tangent to both faces**, which is what says it is a
/// fillet rather than a chamfer that happens to be round.
///
/// Written down rather than measured: the top of the block is `y = 4` and the
/// wall it meets is `z = 0`, so a unit blend between them lies on the cylinder
/// of radius one about the line `y = 3, z = −1`. Both distances come to the
/// radius, which is the only thing tangency is.
#[test]
fn a_blend_lies_on_the_cylinder_tangent_to_both_faces() {
    let cube = cube();
    let along = [between(
        &cube,
        DVec3::new(0.0, 4.0, 0.0),
        DVec3::new(4.0, 4.0, 0.0),
    )];
    let mut into = Body::default();
    assert!(Rounding::default().round(&Round::new(&along, 1.0, ROUND), &cube, &mut into));

    let (_, face) = into
        .patches(ROUND.grew(Grown::Rounded(0)))
        .next()
        .expect("the blend is a face of the answer");
    let Surface::Natural(Natural::Cylinder(cylinder)) = face.surface else {
        panic!("a blend between two planes lies on a cylinder, not {face:?}");
    };
    assert!(
        (cylinder.radius - 1.0).abs() < PLACED,
        "the blend has radius {}",
        cylinder.radius,
    );
    assert!(
        predicate::parallel(cylinder.axis.direction, DVec3::X),
        "the blend runs {} where the edge ran along +x",
        cylinder.axis.direction,
    );
    assert!(
        cylinder.axis.off(DVec3::new(0.0, 3.0, -1.0)) < PLACED,
        "the blend's axis misses the line a unit inside both faces",
    );
    assert!(
        face.outward,
        "material lies inside a blend cut into a convex edge",
    );
}

/// **A blend down a concave edge puts the corner back**, which is the same
/// cylinder tangent to the same two planes with the material on the other side
/// of it.
///
/// The block is a four-by-four square less the two-by-two out of one corner, so
/// it shuts in forty-eight; the reflex edge where the notch turns is four long,
/// and a unit blend fills the corner the cylinder leaves.
#[test]
fn a_notch_rounded_on_the_inside_gains_the_corner_the_arithmetic_says() {
    let notched = block(
        Plane::GROUND,
        &[
            (0.0, 0.0),
            (4.0, 0.0),
            (4.0, 4.0),
            (2.0, 4.0),
            (2.0, 2.0),
            (0.0, 2.0),
        ],
        4.0,
    );
    assert!(
        (volume(&notched) - 48.0).abs() < CLOSES,
        "the notched block shuts in {} where twelve by four is forty-eight",
        volume(&notched),
    );
    let along = [between(
        &notched,
        DVec3::new(2.0, 0.0, -2.0),
        DVec3::new(2.0, 4.0, -2.0),
    )];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(&Round::new(&along, 1.0, ROUND), &notched, &mut into),
        "a unit blend down the reflex edge of a notch was refused",
    );

    let want = 48.0 + corner(1.0, 4.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the rounded notch shuts in {} where {want} is the corner put back",
        volume(&into),
    );
    let (_, face) = into
        .patches(ROUND.grew(Grown::Rounded(0)))
        .next()
        .expect("the blend is a face of the answer");
    assert!(
        !face.outward,
        "material lies outside a blend filled into a concave edge",
    );
    assert_eq!(into.reckoning().genus, 0, "a filled notch is still a ball");
}

/// **Two edges rounded at once cost two corners**, and the two blends answer to
/// the picks that asked for them rather than to each other.
///
/// The two top edges taken are opposite, so neither reaches the other's corners
/// — which is the one thing a rounding of several edges asks of them.
#[test]
fn two_edges_rounded_at_once_take_both_corners() {
    let cube = cube();
    let along = [
        between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
        between(
            &cube,
            DVec3::new(0.0, 4.0, -4.0),
            DVec3::new(4.0, 4.0, -4.0),
        ),
    ];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(&Round::new(&along, 0.5, ROUND), &cube, &mut into),
        "two opposite edges of a block were refused",
    );

    let want = 64.0 - 2.0 * corner(0.5, 4.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the twice-rounded block shuts in {} where {want} is two corners off sixty-four",
        volume(&into),
    );
    for pick in 0..2 {
        assert_eq!(
            into.patches(ROUND.grew(Grown::Rounded(pick))).count(),
            1,
            "pick {pick} raised other than one blend",
        );
    }
    assert_eq!(smooth(&into), 4, "two rulings apiece");
}

/// **A rounded body is a body**, which is what the rest of the kernel has to be
/// able to take it for.
///
/// A bore up the middle of the rounded block stands clear of the blend, so what
/// it takes is the whole cylinder and the arithmetic is the block's own less
/// `π·r²·h`.
#[test]
fn a_rounded_block_is_bored_like_any_other() {
    let cube = cube();
    let along = [between(
        &cube,
        DVec3::new(0.0, 4.0, 0.0),
        DVec3::new(4.0, 4.0, 0.0),
    )];
    let mut rounded = Body::default();
    assert!(Rounding::default().round(&Round::new(&along, 1.0, ROUND), &cube, &mut rounded));

    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(2.0, 2.0));
    sketch.add_circle(middle, 1.0);
    let found = Arrangement::of(&sketch);
    let bore = Extrusion::new(
        &found,
        &[0],
        Plane {
            origin: DVec3::new(0.0, -1.0, 0.0),
            ..Plane::GROUND
        },
        6.0,
        Step(3),
    )
    .body();

    let mut into = Body::default();
    assert!(
        Boolean::default().combine(&rounded, &bore, Operation::Cut, &mut into),
        "a bore through a rounded block was refused",
    );
    let want = 64.0 - corner(1.0, 4.0) - PI * 4.0;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the bored block shuts in {} where {want} is the blend and the bore taken off",
        volume(&into),
    );
    assert_eq!(into.reckoning().genus, 1, "a bore right through is a hole");
}

/// **What a rounding cannot make is refused rather than guessed at**, which is
/// the standing every unanswerable case in this kernel takes.
///
/// Five, and each is a different thing being asked for: a radius that is no
/// blend at all; one so large the blend runs off the end of the edges it has to
/// meet, which wants those edges rounded too; two picks meeting at a corner,
/// which wants a vertex blend; a pair of names with no edge between them; and
/// an edge on a face that is not flat, which wants a variable-radius blend.
#[test]
fn what_a_rounding_cannot_make_is_refused() {
    let cube = cube();
    let top = between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0));
    let beside = between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 4.0, -4.0));
    let ends = [
        cube.names().next().expect("a block has a base"),
        cube.names().nth(1).expect("a block has a far end"),
    ];

    let mut rounding = Rounding::default();
    let mut into = Body::default();
    let rows: [(&str, Vec<[Named; 2]>, f64); 4] = [
        ("a radius of nothing", vec![top], 0.0),
        (
            // The edges it would run out onto are four long, so a corner four
            // along one of them lands exactly on its far end.
            "a radius the edges it meets cannot hold",
            vec![top],
            4.0,
        ),
        ("two picks meeting at a corner", vec![top, beside], 1.0),
        ("two faces with no edge between them", vec![ends], 1.0),
    ];
    for (what, along, radius) in rows {
        assert!(
            !rounding.round(&Round::new(&along, radius, ROUND), &cube, &mut into),
            "{what} was answered",
        );
        assert!(into.is_empty(), "{what}: a refusal left half a body behind");
    }

    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 1.0);
    let found = Arrangement::of(&sketch);
    let rod = Extrusion::new(&found, &[0], Plane::GROUND, 4.0, SOLID).body();
    let round = [[
        rod.names().next().expect("a rod has a base"),
        rod.names().nth(2).expect("a rod has a wall"),
    ]];
    assert!(
        !rounding.round(&Round::new(&round, 0.25, ROUND), &rod, &mut into),
        "the rim of a rod was answered, and a blend there is not a cylinder",
    );
    assert!(into.is_empty(), "a refusal left half a body behind");
}

/// **A blend whose ends lean cuts ellipses rather than arcs of a circle**,
/// which is the second thing a face across the end of one can be.
///
/// The block is a trapezoid — four across at the bottom, two at the top, four
/// high — so the two walls at the ends of its bottom edge lean, and the section
/// each cuts out of the blend is an ellipse. The arithmetic follows the same
/// corner, weighted by how long the blend is where that corner stands: the two
/// walls are `x = −z/4` and `x = 4 + z/4`, so a blend at height `z` is
/// `4 + z/2` long. Over the corner in `t = y − 3, s = z + 1` that comes to
/// `3.5·(1 − π/4)` for the corner's own area and `½·⅙` for the first moment of
/// it, which is `½` for the unit square less `⅓` for the quarter disc.
#[test]
fn a_blend_whose_ends_lean_is_closed_by_two_ellipses() {
    let trapezoid = block(
        Plane::GROUND,
        &[(0.0, 0.0), (4.0, 0.0), (3.0, 4.0), (1.0, 4.0)],
        4.0,
    );
    assert!(
        (volume(&trapezoid) - 48.0).abs() < CLOSES,
        "the trapezoid prism shuts in {} where twelve by four is forty-eight",
        volume(&trapezoid),
    );
    let along = [between(
        &trapezoid,
        DVec3::new(0.0, 4.0, 0.0),
        DVec3::new(4.0, 4.0, 0.0),
    )];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(&Round::new(&along, 1.0, ROUND), &trapezoid, &mut into),
        "a unit blend between two leaning walls was refused",
    );

    let want = 48.0 - (3.5 * (1.0 - PI / 4.0) + 1.0 / 12.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the rounded trapezoid shuts in {} where {want} is the leaning corner taken off",
        volume(&into),
    );
    let (_, face) = into
        .patches(ROUND.grew(Grown::Rounded(0)))
        .next()
        .expect("the blend is a face of the answer");
    let topology = into.topology();
    let leaning = topology
        .loops_of(face)
        .flatten()
        .filter(|coedge| matches!(topology.edge(coedge.edge).curve, Curve::Ellipse(_)))
        .count();
    assert_eq!(leaning, 2, "the blend is closed by two ellipses");
    assert_eq!(into.reckoning().genus, 0, "a rounded prism is still a ball");
    assert!(
        into.exact(),
        "an ellipse on a cylinder is of the exact tier"
    );
}
