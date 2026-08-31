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

/// The same block with a two-by-two notch out of one corner, which is what a
/// reflex edge is made from — the one place here a blend is filled in rather
/// than cut out.
fn notch() -> Body {
    block(
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
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Round, ROUND),
            &cube,
            &mut into
        ),
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
    assert!(Rounding::default().round(
        &Round::new(&along, 1.0, Bevel::Round, ROUND),
        &cube,
        &mut into
    ));

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
    let notched = notch();
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
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Round, ROUND),
            &notched,
            &mut into
        ),
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
        Rounding::default().round(
            &Round::new(&along, 0.5, Bevel::Round, ROUND),
            &cube,
            &mut into
        ),
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
    assert!(Rounding::default().round(
        &Round::new(&along, 1.0, Bevel::Round, ROUND),
        &cube,
        &mut rounded
    ));

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

/// **A pick whose edge a boolean cut into pieces is one blend**, which is what
/// a body the kernel hands on actually looks like.
///
/// A cut is taken by whole surfaces, so a pocket's four walls divide the *whole*
/// of every face they reach and every edge bounding one — see
/// `.notes/KERNEL.md` §9.3, where those splits are the answer's contract for
/// the next boolean. Here a two-by-two pocket through a four-cube leaves the
/// far cap in nine patches and its edge against the first wall in three, and a
/// pick naming the pair finds all three.
///
/// **One face, and the arithmetic of one blend.** The three pieces lie on one
/// line between one pair of planes, so what goes down them is one cylinder: the
/// corner it takes is the whole four-long edge's, `(1 − π/4)·r²·l`, off the
/// forty-eight the pocket left.
#[test]
fn a_pick_a_boolean_cut_into_pieces_is_one_blend() {
    let cube = cube();
    let mut sketch = Sketch::default();
    sketch.outline(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
    let found = Arrangement::of(&sketch);
    let pocket = Extrusion::new(
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
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&cube, &pocket, Operation::Cut, &mut cut),
        "a pocket through a block was refused",
    );

    // Named off the block rather than off what the cut left, which is the whole
    // point: a face of the answer answers to the name the step that grew it
    // gave it, however many patches the cut left it in.
    let names: Vec<_> = cube.names().collect();
    let along = [[names[1], names[2]]];
    let pieces = cut
        .topology()
        .edges()
        .filter(|(_, edge)| {
            let here = edge.between.map(|face| cut.topology().face(face).name);
            here == along[0] || here == [along[0][1], along[0][0]]
        })
        .count();
    assert_eq!(pieces, 3, "the cut left the edge in {pieces} pieces");

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 0.5, Bevel::Round, ROUND),
            &cut,
            &mut into
        ),
        "a pick of three pieces of one edge was refused",
    );

    let want = 48.0 - corner(0.5, 4.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the rounded pocket shuts in {} where {want} is the corner off forty-eight",
        volume(&into),
    );
    assert_eq!(
        into.patches(ROUND.grew(Grown::Rounded(0))).count(),
        1,
        "three pieces of one edge raised other than one blend",
    );
    assert_eq!(
        into.reckoning().genus,
        1,
        "a pocket right through is a hole, rounded or not",
    );
    assert!(
        into.exact(),
        "a blend on two planes is a cylinder and exact"
    );
}

/// **What a rounding cannot make is refused rather than guessed at**, which is
/// the standing every unanswerable case in this kernel takes.
///
/// Six, and each is a different thing being asked for: a reach that is no blend
/// at all; one so large the blend runs off the end of the edges it has to meet,
/// which wants those edges rounded too; a pair of names with no edge between
/// them; an edge on a face that is not flat, which wants a variable-radius
/// blend; two picks meeting at a corner from opposite sides, which is a corner
/// no rolling ball reaches from one side; and three *flat* picks meeting at a
/// corner, which wants three lines where three round ones want a patch.
#[test]
fn what_a_rounding_cannot_make_is_refused() {
    let cube = cube();
    let top = between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0));
    let ends = [
        cube.names().next().expect("a block has a base"),
        cube.names().nth(1).expect("a block has a far end"),
    ];

    let mut rounding = Rounding::default();
    let mut into = Body::default();
    let rows: [(&str, Vec<[Named; 2]>, f64); 3] = [
        ("a radius of nothing", vec![top], 0.0),
        (
            // The edges it would run out onto are four long, so a corner four
            // along one of them lands exactly on its far end.
            "a radius the edges it meets cannot hold",
            vec![top],
            4.0,
        ),
        ("two faces with no edge between them", vec![ends], 1.0),
    ];
    for (what, along, radius) in rows {
        assert!(
            !rounding.round(
                &Round::new(&along, radius, Bevel::Round, ROUND),
                &cube,
                &mut into
            ),
            "{what} was answered",
        );
        assert!(into.is_empty(), "{what}: a refusal left half a body behind");
    }

    // Three chamfers at one corner. Their planes meet at a point and leave no
    // patch between them, so what fills the corner is three lines rather than a
    // face — which is a routine of its own, where three fillets leave a patch
    // of a sphere.
    let corner = [
        between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 4.0, -4.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 0.0, 0.0)),
    ];
    assert!(
        !rounding.round(
            &Round::new(&corner, 1.0, Bevel::Flat, ROUND),
            &cube,
            &mut into
        ),
        "three chamfers meeting at a corner were answered",
    );
    assert!(into.is_empty(), "a refusal left half a body behind");

    // A corner where a reflex edge meets a convex one. Both cylinders stand a
    // radius off the face they share and the two stand off it on opposite
    // sides, so the pair never cross there — see [`Junction`].
    let notched = notch();
    let leaning = [
        between(
            &notched,
            DVec3::new(2.0, 0.0, -2.0),
            DVec3::new(2.0, 4.0, -2.0),
        ),
        between(
            &notched,
            DVec3::new(2.0, 0.0, -2.0),
            DVec3::new(2.0, 0.0, -4.0),
        ),
    ];
    assert!(
        !rounding.round(
            &Round::new(&leaning, 0.5, Bevel::Round, ROUND),
            &notched,
            &mut into
        ),
        "a corner a blend reaches from either side was answered",
    );
    assert!(into.is_empty(), "a refusal left half a body behind");

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
        !rounding.round(
            &Round::new(&round, 0.25, Bevel::Round, ROUND),
            &rod,
            &mut into
        ),
        "the rim of a rod was answered, and a blend there is not a cylinder",
    );
    assert!(into.is_empty(), "a refusal left half a body behind");
}

/// **Two picked edges meeting at a corner close against each other**, and what
/// they leave is one arc and no face at all.
///
/// Both cylinders are tangent to the one face the two edges share, so both axes
/// stand a radius off it and the pair cross in an ellipse — which `Meeting::of`
/// writes down exactly. Nothing is left over for a patch to fill, which is what
/// tells this from the corner a *third* picked edge runs to.
///
/// **The arithmetic is the two corners less what they share.** Each blend takes
/// `(1 − π/4)·r²·l` off its own four-long edge; near the corner the two take
/// the same material, and what is counted twice is the corner over the two of
/// them: `∫₀ʳ (r − √(2rw − w²))² dw`, which comes to `r³·(5/3 − π/2)`.
#[test]
fn two_blends_meeting_at_a_corner_close_against_each_other() {
    let cube = cube();
    let along = [
        between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 4.0, -4.0)),
    ];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Round, ROUND),
            &cube,
            &mut into
        ),
        "two picks meeting at a corner were refused",
    );

    let want = 64.0 - 8.0 * (1.0 - PI / 4.0) + 5.0 / 3.0 - PI / 2.0;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the twice-rounded block shuts in {} where {want} is the two corners \
         less the one they share",
        volume(&into),
    );

    // Eight faces, seventeen edges and eleven corners, which Euler holds to a
    // ball: the block's twelve edges less the two replaced, four rulings, one
    // arc across each blend's far end, and the one arc the two share.
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "a twice-rounded block is still a ball");
    let topology = into.topology();
    assert_eq!(topology.faces().count(), 8, "six faces and two blends");
    assert_eq!(topology.edges().count(), 17);
    assert_eq!(
        smooth(&into),
        4,
        "two rulings apiece, and the arc the two blends share is a crease",
    );
    // The one edge of the answer that divides two blends, which is what says
    // the pair closed against each other rather than against a face.
    let raised: Vec<_> = (0..2)
        .map(|pick| ROUND.grew(Grown::Rounded(pick)))
        .collect();
    let met = topology
        .edges()
        .filter(|(_, edge)| {
            edge.between
                .iter()
                .all(|&face| raised.contains(&topology.face(face).name))
        })
        .count();
    assert_eq!(met, 1, "the two blends do not close against each other");
    assert!(
        into.exact(),
        "an ellipse on a cylinder is of the exact tier"
    );
}

/// **Three picked edges meeting at a corner leave a patch of a sphere between
/// their cylinders**, which is what a rolling ball leaves when it pivots there.
///
/// The sphere has the blends' own radius and stands a radius off all three
/// faces, which is the one point every cylinder's axis runs through — so it is
/// inscribed in each of them and touches each along a whole circle. The patch is
/// the triangle those three circles cut out, and every one of its edges is a
/// smooth join.
///
/// **The arithmetic is the corner cube and the three edges either side of it.**
/// Outside the unit cube at the corner the three blends do not meet, so each
/// takes `(1 − π/4)·r²·(l − r)`. Inside it what is *kept* is the ball octant,
/// `πr³/6`, so the cube gives up `r³(1 − π/6)`. A unit blend down all three
/// four-long edges of a four-cube therefore shuts in `54 + 9π/4 + π/6`.
#[test]
fn three_blends_meeting_at_a_corner_leave_a_patch_of_a_sphere() {
    let cube = cube();
    let along = [
        between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 4.0, -4.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 0.0, 0.0)),
    ];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Round, ROUND),
            &cube,
            &mut into
        ),
        "three picks meeting at a corner were refused",
    );

    let want = 54.0 + 9.0 * PI / 4.0 + PI / 6.0;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the thrice-rounded block shuts in {} where {want} is the three corners \
         and the ball octant between them",
        volume(&into),
    );

    // Ten faces, twenty-one edges and thirteen corners, which Euler holds to a
    // ball: the block's twelve edges less the three replaced, six rulings, one
    // arc across each blend's far end, and the patch's own three.
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "a thrice-rounded block is still a ball");
    let topology = into.topology();
    assert_eq!(
        topology.faces().count(),
        10,
        "six faces, three blends and the patch",
    );
    assert_eq!(topology.edges().count(), 21);
    assert_eq!(
        smooth(&into),
        9,
        "two rulings apiece, and the patch runs out into all three blends",
    );

    // Named by the three picks that met, which is the one thing a corner has to
    // name itself with — see [`Grown::Cornered`].
    let patch = ROUND.grew(Grown::Cornered([0, 1, 2]));
    let (_, face) = into
        .patches(patch)
        .next()
        .expect("the patch is a face of the answer");
    let Surface::Natural(Natural::Sphere(sphere)) = face.surface else {
        panic!("a patch between three cylinders lies on a sphere, not {face:?}");
    };
    assert!(
        (sphere.radius - 1.0).abs() < PLACED,
        "the patch has radius {}",
        sphere.radius,
    );
    // A unit inside all three faces of the corner at `(4, 4, 0)`, whose walls
    // are `y = 4`, `x = 4` and `z = 0`.
    assert!(
        sphere.centre().distance(DVec3::new(3.0, 3.0, -1.0)) < PLACED,
        "the patch stands at {} rather than a unit inside all three faces",
        sphere.centre(),
    );
    assert!(
        face.outward,
        "material lies inside a patch cut into a corner"
    );
    assert!(into.exact(), "a sphere is of the exact tier");
}

/// **A flat blend cuts the corner off square**, which is the other thing a
/// rounding can put between the two rulings.
///
/// Everything but the surface is what a round one leaves: the same two faces
/// cut back to the same two rulings for a corner they meet square at, the same
/// corner swallowed, the same edges shortened. What differs is the face between
/// them and the two joins, which are creases where a fillet's are not.
///
/// The corner it takes is a right triangle with both legs the setback, so a
/// unit chamfer down a four-long edge takes `½·1²·4` off sixty-four.
#[test]
fn a_flat_blend_cuts_the_corner_off_square() {
    let cube = cube();
    let along = [between(
        &cube,
        DVec3::new(0.0, 4.0, 0.0),
        DVec3::new(4.0, 4.0, 0.0),
    )];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Flat, ROUND),
            &cube,
            &mut into
        ),
        "a unit chamfer down a four-long edge was refused",
    );

    let want = 64.0 - 0.5 * 4.0;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the chamfered block shuts in {} where {want} is the corner taken off sixty-four",
        volume(&into),
    );

    // The same counts a fillet leaves, which is the whole claim: one topology,
    // two surfaces.
    assert_eq!(
        into.reckoning().genus,
        0,
        "a chamfered block is still a ball"
    );
    let topology = into.topology();
    assert_eq!(topology.faces().count(), 7, "six faces and the chamfer");
    assert_eq!(topology.edges().count(), 15);
    assert_eq!(
        smooth(&into),
        0,
        "a chamfer meets both faces at an angle, so neither join is smooth",
    );

    let (_, face) = into
        .patches(ROUND.grew(Grown::Rounded(0)))
        .next()
        .expect("the chamfer is a face of the answer");
    let Surface::Natural(Natural::Plane(plane)) = face.surface else {
        panic!("a chamfer between two planes is a plane, not {face:?}");
    };
    // The top of the block is `y = 4` and the wall it meets is `z = 0`, so a
    // unit chamfer between them runs from `(·, 3, 0)` to `(·, 4, −1)` — and the
    // plane square to that run leans equally on both faces.
    let leaning = DVec3::new(0.0, 1.0, 1.0).normalize();
    assert!(
        predicate::parallel(plane.normal(), leaning),
        "the chamfer faces {} rather than equally on both",
        plane.normal(),
    );
    // And out of the material, which is away from the edge for a corner cut off
    // rather than filled in.
    assert!(
        face.normal(face.surface.uv(DVec3::new(0.0, 4.0, -1.0)))
            .abs_diff_eq(leaning, PLACED),
        "the chamfer faces into the material",
    );
    assert!(into.exact(), "a plane is of the exact tier");
}

/// **Two flat blends meeting at a corner close against each other in a line**,
/// which is the junction two round ones make with the ellipse straightened.
///
/// The two chamfer planes cross in a line, and `Meeting::of` writes it down —
/// so nothing about the corner is a case of its own. What the two share near
/// the corner is `∫₀ˢ (s − v)² dv`, which comes to `s³/3`, so a unit chamfer
/// down two adjacent four-long edges shuts in `64 − 4 + ⅓`.
#[test]
fn two_flat_blends_meeting_at_a_corner_close_against_each_other() {
    let cube = cube();
    let along = [
        between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 4.0, -4.0)),
    ];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Flat, ROUND),
            &cube,
            &mut into
        ),
        "two chamfers meeting at a corner were refused",
    );

    let want = 64.0 - 4.0 + 1.0 / 3.0;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the twice-chamfered block shuts in {} where {want} is the two corners \
         less the one they share",
        volume(&into),
    );
    let reckoning = into.reckoning();
    assert_eq!(
        reckoning.genus, 0,
        "a twice-chamfered block is still a ball"
    );
    let topology = into.topology();
    assert_eq!(topology.faces().count(), 8, "six faces and two chamfers");
    assert_eq!(topology.edges().count(), 17);
    assert_eq!(smooth(&into), 0, "no join of a chamfer is smooth");
    assert!(into.exact(), "two planes crossing in a line stay exact");
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
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Round, ROUND),
            &trapezoid,
            &mut into
        ),
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

/// **Three picks meeting at a corner, on a body a boolean has cut into
/// patches**, which is where a run's two ends stop being interchangeable.
///
/// A slot through the block leaves both of the faces at that corner in patches,
/// so two of the three picks are runs of three pieces — and the pair of patches
/// a corner of the answer stands between is the pair the spine *at that end*
/// divides, not the pair the run started from. The patch seats each blend
/// against the face it touches there, so reading the run's first spine at its
/// far end seats it against a patch that is not even there.
///
/// **The arithmetic is the corner arithmetic off what the slot left.** The
/// blends stand half a unit off faces the slot comes no nearer than one, so
/// each takes the same `(1 − π/4)·r²·(l − r)` outside the corner cube it would
/// take off a whole block, and the cube gives up `r³(1 − π/6)` — off the
/// forty-eight a two-by-two slot through a four-cube leaves.
#[test]
fn three_picks_meeting_on_a_body_a_boolean_cut_leave_the_same_patch() {
    let cube = cube();
    let mut sketch = Sketch::default();
    sketch.outline(&[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)]);
    let found = Arrangement::of(&sketch);
    let slot = Extrusion::new(
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
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&cube, &slot, Operation::Cut, &mut cut),
        "a slot through a block was refused",
    );

    // Named off the block, which is what makes each pick one run: the patches
    // the slot left all answer to the name of the face they were cut from.
    let names: Vec<_> = cube.names().collect();
    let along = [
        [names[1], names[2]],
        [names[1], names[3]],
        [names[2], names[3]],
    ];
    let pieces = along.map(|pick| {
        cut.topology()
            .edges()
            .filter(|(_, edge)| {
                let here = edge.between.map(|face| cut.topology().face(face).name);
                here == pick || here == [pick[1], pick[0]]
            })
            .count()
    });
    assert_eq!(
        pieces,
        [3, 3, 1],
        "the slot left the three picked edges in other than three, three and one",
    );

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 0.5, Bevel::Round, ROUND),
            &cut,
            &mut into
        ),
        "three picks meeting at a corner of a cut body were refused",
    );

    let want = 48.0 - 3.0 * corner(0.5, 3.5) - 0.125 * (1.0 - PI / 6.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the rounded slot shuts in {} where {want} is the three corners and the \
         ball octant between them",
        volume(&into),
    );
    for pick in 0..3 {
        assert_eq!(
            into.patches(ROUND.grew(Grown::Rounded(pick))).count(),
            1,
            "pick {pick} raised other than one blend",
        );
    }

    let (_, face) = into
        .patches(ROUND.grew(Grown::Cornered([0, 1, 2])))
        .next()
        .expect("the patch is a face of the answer");
    let Surface::Natural(Natural::Sphere(sphere)) = face.surface else {
        panic!("a patch between three cylinders lies on a sphere, not {face:?}");
    };
    // Half a unit inside all three faces of the corner at `(4, 4, 0)`, whose
    // walls are `y = 4`, `x = 4` and `z = 0`.
    assert!(
        (sphere.radius - 0.5).abs() < PLACED
            && sphere.centre().distance(DVec3::new(3.5, 3.5, -0.5)) < PLACED,
        "the patch is {} at {} rather than half a unit inside all three faces",
        sphere.radius,
        sphere.centre(),
    );
    assert_eq!(
        into.reckoning().genus,
        1,
        "a slot right through is a hole, rounded or not",
    );
    assert!(
        into.exact(),
        "a cylinder and a sphere are of the exact tier"
    );
}
