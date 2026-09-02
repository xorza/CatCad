//! What a rounding makes of a body, and what it will not make of one.
//!
//! **Volumes, and they are hand-computed rather than compared.** A blend of
//! radius `r` down an edge of length `l` between two faces meeting square takes
//! the corner the cylinder does not fill — `(1 − π/4)·r²·l` — off a convex edge
//! and puts the same back into a concave one. Nothing about that reading
//! depends on how the answer was built, which is what makes it worth asserting.

use super::*;
use crate::Plane;
use crate::math::chorded::Chorded;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::boolean::Boolean;
use crate::solid::boolean::operation::Operation;
use crate::solid::build::builder::Extrusion;
use crate::solid::build::revolving::{MOST, Revolution};
use crate::solid::build::sector::Sector;
use crate::solid::mesh::{Mesher, Patch};
use crate::solid::topology::face::Face;
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

/// The step the block below is grown by, and the one the blends are.
///
/// Two, and that is the point: a blend answers to
/// [`Grown::Rounded`](crate::Grown) and so does the blend of the next rounding,
/// so what tells the two apart is the step — see
/// [`Named`](crate::solid::named::Named).
const SOLID: Step = Step(1);
const ROUND: Step = Step(2);

/// The step a tool cuts by, where a row builds one before it blends.
const TOOL: Step = Step(3);

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
fn block(plane: Plane, corners: &[(f64, f64)], deep: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(corners);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, &[0], plane, deep, by).body()
}

/// The four-by-four-by-four block most rows here are taken off.
fn cube() -> Body {
    block(
        Plane::GROUND,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)],
        4.0,
        SOLID,
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
        SOLID,
    )
}

/// The four-cube with a two-by-two slot through it, and the *block's* names.
///
/// **Named off the block rather than off what the cut left**, which is what
/// makes each pick below one run: a face of the answer answers to the name the
/// step that grew it gave it, however many patches the cut left it in.
///
/// The slot's walls divide the two faces at the corner `(4, 4, 0)`, so two of
/// the three edges running to it come back in three pieces and the third in
/// one. Every row using this asserts that count through [`Slotted::pieces`],
/// because a row that quietly got one piece would be asserting nothing about a
/// run.
fn slotted() -> Slotted {
    let cube = cube();
    let slot = block(
        Plane {
            origin: DVec3::new(0.0, -1.0, 0.0),
            ..Plane::GROUND
        },
        &[(1.0, 1.0), (3.0, 1.0), (3.0, 3.0), (1.0, 3.0)],
        6.0,
        TOOL,
    );
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&cube, &slot, Operation::Cut, &mut cut),
        "a slot through a block was refused",
    );
    Slotted {
        names: cube.names().collect(),
        cut,
    }
}

/// A block a boolean has cut, and the names its picks are made of.
#[derive(Debug)]
struct Slotted {
    cut: Body,
    names: Vec<Named>,
}

impl Slotted {
    /// How many pieces the cut left of the edge each pick names.
    fn pieces(&self, along: &[[Named; 2]]) -> Vec<usize> {
        along
            .iter()
            .map(|&pick| {
                self.cut
                    .topology()
                    .edges()
                    .filter(|(_, edge)| {
                        let here = edge.between.map(|face| self.cut.topology().face(face).name);
                        here == pick || here == [pick[1], pick[0]]
                    })
                    .count()
            })
            .collect()
    }
}

/// The three picks that meet at the corner `(4, 4, 0)` of a block.
///
/// **Named by index rather than by where they stand**, which the rows using
/// this want and [`between`] below cannot give them: on a body a boolean has
/// cut, the pick is the pair of names the *block* carried and no edge of the
/// answer runs the whole way between the two corners.
fn at_a_corner(names: &[Named]) -> [[Named; 2]; 3] {
    [
        [names[1], names[2]],
        [names[1], names[3]],
        [names[2], names[3]],
    ]
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

/// The two faces the circular edge of `radius` about `centre` divides.
///
/// What names a rim, where [`between`] names a straight edge by its two ends: a
/// rim a body split in halves has no one pair of ends, and both halves answer
/// the same pair of faces.
fn ringed(body: &Body, radius: f64, centre: DVec3) -> [Named; 2] {
    let topology = body.topology();
    let (_, edge) = topology
        .edges()
        .find(|(_, edge)| match edge.curve {
            Curve::Circle(circle) => {
                (circle.radius - radius).abs() < PLACED
                    && circle.axis.origin.abs_diff_eq(centre, PLACED)
            }
            _ => false,
        })
        .expect("the body has a circular edge of that radius there");
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

/// Whether the body has a corner standing at `at`.
fn stands(body: &Body, at: DVec3) -> bool {
    body.topology()
        .vertices()
        .any(|(_, vertex)| vertex.at.abs_diff_eq(at, PLACED))
}

/// How many edges of the body run to the corner at `at`.
fn legs(body: &Body, at: DVec3) -> usize {
    let topology = body.topology();
    topology
        .edges()
        .filter(|(_, edge)| {
            [edge.from, edge.to]
                .iter()
                .any(|&end| topology.vertex(end).at.abs_diff_eq(at, PLACED))
        })
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
/// `.notes/KERNEL.md` §7.4, where those splits are the answer's contract for
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
    let slotted = slotted();
    let along = [[slotted.names[1], slotted.names[2]]];
    assert_eq!(
        slotted.pieces(&along),
        [3],
        "the cut left the picked edge in other than three pieces",
    );
    let cut = &slotted.cut;

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 0.5, Bevel::Round, ROUND),
            cut,
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
/// Four, and each is a different thing being asked for: a reach that is no
/// blend at all; one so large the blend runs off the end of the edges it has to
/// meet, which wants those edges rounded too; a pair of names with no edge
/// between them; and a fillet as wide as half the rod whose rim it runs down,
/// where the torus pinches on its own axis.
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

    // A fillet down a rim as wide as the rod's own half. The tube's centre
    // circle stands `radius - reach` out, so at half the radius it closes on
    // the axis and the torus pinches — which is no surface a body can be made
    // of. See [`Torus`].
    let rod = rod(1.0, 4.0, SOLID);
    let whole = rim(&rod);
    assert!(
        !rounding.round(
            &Round::new(&whole, 0.5, Bevel::Round, ROUND),
            &rod,
            &mut into
        ),
        "a fillet as wide as half the rod was answered, and its torus pinches",
    );
    assert!(into.is_empty(), "a refusal left half a body behind");
}

/// A rod of `radius`, carried `deep` off the ground.
///
/// **The step is the caller's**, because two rods grown by the one step carry
/// the one set of names: a pick naming a cap and a wall would then find the
/// rims of both, which is `.notes/KERNEL.md` §5 working exactly as it says and
/// not what a fixture of two rods means.
fn rod(radius: f64, deep: f64, by: Step) -> Body {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, radius);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, &[0], Plane::GROUND, deep, by).body()
}

/// The base rim of a rod and its wall, which is one closed run of two halves.
fn rim(rod: &Body) -> [[Named; 2]; 1] {
    [[
        rod.names().next().expect("a rod has a base"),
        rod.names().nth(2).expect("a rod has a wall"),
    ]]
}

/// **A fillet down a rim is a torus**, where one down a straight edge is a
/// cylinder — and it is the first blend of the fitted tier, so the body it
/// leaves is no longer exact. See `.notes/KERNEL.md` §4.1.
///
/// The rolling ball's centre traces the circle standing a reach inside both
/// faces, which is `radius - reach` out and a reach up. So the corner it takes
/// off is the square of the reach less the quarter disc, turned about the axis
/// — and Pappus reads that off the region's own centroid:
///
/// ```text
/// removed = 2π·[ r²·(R − r/2) − ¼π·r²·(R − r) − ⅓·r³ ]
/// ```
///
/// The run closes, so the blend is **two** faces and not one: a single face
/// over the whole turn of a torus would be the seam §4.4 refuses.
#[test]
fn a_fillet_down_a_rim_is_a_torus_of_two_faces() {
    let (radius, reach, deep) = (1.0, 0.25, 4.0);
    let rod = rod(radius, deep, SOLID);
    let along = rim(&rod);
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Round, ROUND),
            &rod,
            &mut into
        ),
        "a fillet down the rim of a rod was refused",
    );

    let corner = reach * reach * (radius - reach / 2.0)
        - 0.25 * PI * reach * reach * (radius - reach)
        - reach * reach * reach / 3.0;
    let want = PI * radius * radius * deep - 2.0 * PI * corner;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the filleted rod shuts in {} where {want} is the rod less the rim it broke",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(raised.len(), 2, "a run that closes is a face per piece");
    for (_, face) in &raised {
        let Surface::Fitted(Fitted::Torus(torus)) = face.surface else {
            panic!("a blend down a rim lies on a torus, not {face:?}");
        };
        assert!(
            (torus.major - (radius - reach)).abs() < PLACED && (torus.minor - reach).abs() < PLACED,
            "the blend's tube is {torus:?} rather than a reach about the circle a              reach inside both faces",
        );
        assert!(
            torus
                .axis
                .origin
                .abs_diff_eq(Plane::GROUND.normal() * reach, PLACED),
            "the tube's centre circle stands at {}",
            torus.axis.origin,
        );
    }

    // Six corners, ten edges and six faces, which Euler holds to a ball: the
    // rod's top rim and two uprights, a ruling apiece on the base and the wall,
    // and the two arcs the blend is cut apart at.
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "a filleted rod is still a ball");
    let topology = into.topology();
    assert_eq!(topology.faces().count(), 6, "the rod's four and two blends");
    assert_eq!(topology.edges().count(), 10);
    assert_eq!(topology.vertices().count(), 6);
    // Six more than the rod's own seam: a ruling apiece on the two faces of
    // each blend, and the arc between the two blends at each of the two
    // corners.
    assert_eq!(
        smooth(&into),
        smooth(&rod) + 6,
        "a fillet runs out smoothly onto the faces it lies tangent to",
    );
    assert!(
        !into.exact(),
        "a torus is of the fitted tier, and a body holding one is not exact",
    );
}

/// **A rim a cut has broken is blended like any other run**, and what closes it
/// at each end is a curve of the fitted tier: a torus meets the plane beyond the
/// corner in something no exact route parameterizes, so the arc is *marched*.
///
/// The flat milled down the rod cuts the base rim in half, so what the pick
/// finds is one arc with two ends rather than a run that closes. Everything the
/// whole rim does it does over half a turn — Pappus again, through `π` rather
/// than `2π`:
///
/// ```text
/// removed = π·[ r²·(R − r/2) − ¼π·r²·(R − r) − ⅓·r³ ]
/// ```
///
/// **And the body says what it strays.** A marched edge carries the bound its
/// walk was measured to, and the corners it ends at carry at least that — which
/// is §4.1's tier read off the body rather than argued about.
#[test]
fn a_rim_a_cut_broke_is_closed_by_a_marched_arc() {
    let (radius, reach, deep, stand) = (2.0, 0.25, 3.0, 1.0);
    let milled = flatted(radius, deep, stand);
    let along = [[milled.base, milled.round]];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Round, ROUND),
            &milled.cut,
            &mut into
        ),
        "a rim a cut broke into a run with ends was refused",
    );

    let corner = reach * reach * (radius - reach / 2.0)
        - 0.25 * PI * reach * reach * (radius - reach)
        - reach * reach * reach / 3.0;
    // The wall survives everywhere the flat does not reach, which is the turn
    // outside the chord it cuts — twice the angle to it, taken off the whole.
    let sweep = TAU - 2.0 * (stand / radius).acos();
    let want = volume(&milled.cut) - sweep * corner;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the flatted rod shuts in {} where {want} is the rod less the stretch of \
         rim it broke",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(
        raised.len(),
        1,
        "a run that ends is one face over the whole of it"
    );
    let Surface::Fitted(Fitted::Torus(torus)) = raised[0].1.surface else {
        panic!("a blend down a rim lies on a torus, not {:?}", raised[0].1);
    };
    assert!(
        (torus.major - (radius - reach)).abs() < PLACED && (torus.minor - reach).abs() < PLACED,
        "the blend's tube is {torus:?} rather than a reach about the circle a \
         reach inside both faces",
    );

    // The arcs closing the two ends are walked rather than written down, so the
    // body carries a bound where the milled rod carried none.
    assert_eq!(milled.cut.strays(), 0.0, "a milled rod is walked nowhere");
    assert!(
        into.strays() > 0.0,
        "a marched arc went in and the body claims to stray nothing",
    );
    assert!(!into.exact(), "a torus is of the fitted tier");
}

/// A truncated cone standing on the ground, `wide` across the bottom and
/// `narrow` across the top.
///
/// Spun rather than cut, so the rim below is one the build made and not one a
/// boolean left.
fn taper(wide: f64, narrow: f64, deep: f64) -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (wide, 0.0), (narrow, deep), (0.0, deep)]);
    Revolution::new(
        &Arrangement::of(&sketch),
        &[0],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        SOLID,
    )
    .body()
}

/// **A chamfer down the rim of a taper is a cone**, which is the flat half of
/// what an offset cone buys: the rulings stand the setback along each face, and
/// what goes between them is the line through the two turned about the axis.
///
/// **The setback runs down a ruling of the wall**, which is the one walk a cone
/// answers — a cone is a line turned about a line, so a step down one is the
/// step through space. See `Natural::walked`.
///
/// What comes off is the triangle between the corner and the two rulings,
/// turned about the axis. Pappus over its own centroid:
///
/// ```text
/// removed = 2π · ½·r²·sin φ · ⅓·(W + (W − r) + (W − r·sin lean))
/// ```
#[test]
fn a_chamfer_down_the_rim_of_a_taper_is_a_cone() {
    let (wide, narrow, deep, reach) = (2.0, 1.0, 2.0, 0.25);
    let taper = taper(wide, narrow, deep);
    let along = [ringed(&taper, wide, Plane::FRONT.point(DVec2::ZERO))];

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Flat, ROUND),
            &taper,
            &mut into
        ),
        "a chamfer down the rim of a taper was refused",
    );

    let lean = ((wide - narrow) / deep).atan();
    let opening = FRAC_PI_2 - lean;
    let up = wide - reach * lean.sin();
    let corner = 0.5 * reach * reach * opening.sin();
    let want = volume(&taper) - TAU * corner * (wide + (wide - reach) + up) / 3.0;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the chamfered taper shuts in {} where {want} is the taper less the rim \
         it cut off",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(raised.len(), MOST, "a run that closes is a face per piece");
    for (_, face) in &raised {
        let Surface::Natural(Natural::Cone(cone)) = face.surface else {
            panic!("a chamfer down a rim lies on a cone, not {face:?}");
        };
        // The two rulings stand a reach along each face, so the line through
        // them rises by `reach·cos lean` over a run of `reach·(1 − sin lean)`.
        let want = ((reach - reach * lean.sin()) / (reach * lean.cos())).atan();
        assert!(
            (cone.half_angle - want).abs() < ALIGNED,
            "the chamfer's cone opens at {} rather than the {want} its two \
             setbacks make",
            cone.half_angle,
        );
    }
    assert!(
        into.exact(),
        "a cone is a quadric, and the body is still exact"
    );
}

/// **A blend down the rim of a taper is a torus like any other**, which is what
/// an offset cone being a cone buys: the two faces' offsets meet in a circle,
/// and everything after that is the one statement every pair here answers to.
///
/// A truncated cone standing on the ground, `wide` across the bottom and
/// `narrow` across the top. Its wall leans *in* by `lean`, so the material at
/// the base rim opens at `φ = π/2 − lean` rather than at a right angle — and a
/// ball of the reach touching both faces stands `reach / tan(½φ)` back along
/// each, which is further than the reach wherever the corner is the sharper.
///
/// **What comes off is a kite less a sector**, turned about the axis. The kite
/// is the two right triangles between the corner, the two rulings and the
/// centre — legs `setback` and `reach` apiece — and the sector is the piece of
/// the tube inside it, opening `π − φ` at the centre. Pappus reads the volume
/// off the two moments, each an area times how far out its own centroid
/// stands.
#[test]
fn a_blend_down_the_rim_of_a_taper_is_the_torus_the_offsets_say() {
    let (wide, narrow, deep, reach) = (2.0, 1.0, 2.0, 0.25);
    let taper = taper(wide, narrow, deep);
    let along = [ringed(&taper, wide, Plane::FRONT.point(DVec2::ZERO))];

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Round, ROUND),
            &taper,
            &mut into
        ),
        "a fillet down the rim of a taper was refused",
    );

    let lean = ((wide - narrow) / deep).atan();
    let opening = FRAC_PI_2 - lean;
    let setback = reach / (opening / 2.0).tan();
    // Where the tube's centres run, and where the ruling on the leaning wall
    // stands: a setback along each face from the rim.
    let middle = wide - setback;
    let up = wide - setback * lean.sin();
    // The kite, as two right triangles about the corner.
    let kite = 0.5 * setback * reach * ((wide + 2.0 * middle) + (wide + middle + up)) / 3.0;
    // The tube's own piece of it, opening `π − φ` about the centre and leaning
    // toward the corner.
    let turn = PI - opening;
    let apart = 2.0 * reach * (turn / 2.0).sin() / (3.0 * turn / 2.0);
    let across = setback / setback.hypot(reach);
    let sector = 0.5 * reach * reach * turn * (middle + apart * across);
    let want = volume(&taper) - TAU * (kite - sector);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the filleted taper shuts in {} where {want} is the taper less the rim \
         it broke",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(raised.len(), MOST, "a run that closes is a face per piece");
    for (_, face) in &raised {
        let Surface::Fitted(Fitted::Torus(torus)) = face.surface else {
            panic!("a blend down a rim lies on a torus, not {face:?}");
        };
        assert!(
            (torus.minor - reach).abs() < PLACED,
            "the tube is {} across rather than the reach",
            torus.minor,
        );
        assert!(
            (torus.major - middle).abs() < PLACED,
            "the tube's circle stands {} out where a setback inside the wall is \
             {middle}",
            torus.major,
        );
        // A reach up from the ground, which is the one face the lean does not
        // move the answer for.
        assert!(
            (torus.axis.origin - Plane::FRONT.point(DVec2::new(0.0, reach))).length() < PLACED,
            "the tube's circle stands at {}",
            torus.axis.origin,
        );
    }
    assert!(!into.exact(), "a torus is of the fitted tier");
}

/// **A fillet fills a rim as readily as it breaks one**, which is the same
/// statement about the offsets read the other way round: a concave rim's ball
/// rolls on the outside of the corner, so its centres run `R + r` out rather
/// than `R - r` and the tube grows where the convex one's shrank.
///
/// The shaft is a boss joined to a plate, so what the rounding is handed is a
/// body a boolean built. What goes in is the corner the tube does not fill, by
/// Pappus as above:
///
/// ```text
/// added = 2π·[ r²·(R + r/2) − ¼π·r²·(R + r) + ⅓·r³ ]
/// ```
#[test]
fn a_fillet_filled_into_a_rim_grows_its_tube_rather_than_shrinking_it() {
    let (radius, reach, step) = (1.0, 0.25, 1.0);
    let boss = rod(radius, 4.0, TOOL);
    let plate = rod(2.0 * radius, step, SOLID);
    let mut shaft = Body::default();
    assert!(
        Boolean::default().combine(&plate, &boss, Operation::Join, &mut shaft),
        "a boss standing on a plate would not join",
    );
    let root = Plane::GROUND.normal() * step;
    let along = [ringed(&shaft, radius, root)];

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Round, ROUND),
            &shaft,
            &mut into
        ),
        "a fillet filled into the root of a shaft was refused",
    );

    let corner = reach * reach * (radius + reach / 2.0)
        - 0.25 * PI * reach * reach * (radius + reach)
        + reach * reach * reach / 3.0;
    let want = volume(&shaft) + 2.0 * PI * corner;
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the filleted shaft shuts in {} where {want} is the shaft and the root it \
         filled",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(raised.len(), 2, "a run that closes is a face per piece");
    for (_, face) in &raised {
        let Surface::Fitted(Fitted::Torus(torus)) = face.surface else {
            panic!("a blend down a rim lies on a torus, not {face:?}");
        };
        assert!(
            (torus.major - (radius + reach)).abs() < PLACED,
            "a fillet filled into a rim runs its centres {} out, not a reach past \
             the wall",
            torus.major,
        );
        assert!(
            !face.outward,
            "a fillet filling a corner holds its material outside its own tube",
        );
    }
    assert_eq!(
        smooth(&into),
        smooth(&shaft) + 6,
        "a fillet runs out smoothly onto the faces it lies tangent to",
    );
}

/// **A chamfer down a rim is a cone**, which is the same routine and a
/// different surface between the rulings — and a cone is a quadric, so the body
/// stays exact where the fillet's torus leaves it fitted.
///
/// The setback stands `s` back along each face, so what comes off is the right
/// triangle between the two rulings and the corner. Pappus again, its centroid
/// standing `R − s/3` out:
///
/// ```text
/// removed = 2π·(R − s/3)·½s² = π·s²·(R − s/3)
/// ```
#[test]
fn a_chamfer_down_a_rim_is_a_cone_and_stays_exact() {
    let (radius, reach, deep) = (1.0, 0.25, 4.0);
    let rod = rod(radius, deep, SOLID);
    let along = rim(&rod);
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Flat, ROUND),
            &rod,
            &mut into
        ),
        "a chamfer down the rim of a rod was refused",
    );

    let want = PI * radius * radius * deep - PI * reach * reach * (radius - reach / 3.0);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the chamfered rod shuts in {} where {want} is the rod less the corner it          cut off",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(raised.len(), 2, "a run that closes is a face per piece");
    for (_, face) in &raised {
        let Surface::Natural(Natural::Cone(cone)) = face.surface else {
            panic!("a chamfer down a rim lies on a cone, not {face:?}");
        };
        // The two rulings stand at `radius - reach` on the base and `radius` a
        // reach up the wall, so the line through them falls a right angle and
        // reaches the axis `radius - reach` below the base.
        assert!(
            (cone.half_angle - PI / 4.0).abs() < ALIGNED,
            "the chamfer's cone opens at {} rather than the right angle its two              equal setbacks make",
            cone.half_angle,
        );
        assert!(
            cone.axis
                .origin
                .abs_diff_eq(Plane::GROUND.normal() * -(radius - reach), PLACED),
            "the cone's apex stands at {}",
            cone.apex(),
        );
    }

    let topology = into.topology();
    assert_eq!(
        topology.faces().count(),
        6,
        "the rod's four and two chamfers"
    );
    assert_eq!(topology.edges().count(), 10);
    assert_eq!(topology.vertices().count(), 6);
    // Two more than the rod's own seam, and they are the arcs between the two
    // chamfers: a chamfer's own two joins are creases, which is what tells it
    // from the fillet above.
    assert_eq!(
        smooth(&into),
        smooth(&rod) + 2,
        "a chamfer creases where it runs out, and joins its own other half",
    );
    assert!(
        into.exact(),
        "a cone is a quadric, and the body is still exact"
    );
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

/// **Three chamfers meeting at a corner leave a star and no face**, which is
/// what tells a flat corner from a round one.
///
/// A chamfer is a plane, so the three cross at one point. Three *cylinders* of
/// one radius do not — see
/// `three_blends_meeting_at_a_corner_leave_a_patch_of_a_sphere`, where a patch
/// fills the gap they leave — so a flat corner wants nothing between them. What
/// goes in is that point, and a line to it from each of the three places a pair
/// of the chamfers cross on the face they share.
///
/// **So each blend closes on two edges at that end and one at the other**, and
/// bounds five where every other blend in this file bounds four.
///
/// **The arithmetic is the three wedges less what they share.** Each chamfer
/// alone takes a right triangle of `s²/2` down its whole four-long edge. Each
/// pair of them overlaps in `s³/3` and all three in `s³/4`, so a unit chamfer
/// down the three edges of one corner of a four-cube takes
/// `3·½·s²·l − 3·s³/3 + s³/4`, which is `6 − 1 + ¼`.
#[test]
fn three_flat_blends_meeting_at_a_corner_leave_a_star() {
    let cube = cube();
    let along = [
        between(&cube, DVec3::new(0.0, 4.0, 0.0), DVec3::new(4.0, 4.0, 0.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 4.0, -4.0)),
        between(&cube, DVec3::new(4.0, 4.0, 0.0), DVec3::new(4.0, 0.0, 0.0)),
    ];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 1.0, Bevel::Flat, ROUND),
            &cube,
            &mut into
        ),
        "three chamfers meeting at a corner were refused",
    );

    let want = 64.0 - (6.0 - 1.0 + 0.25);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the thrice-chamfered block shuts in {} where {want} is the three \
         wedges less what they share",
        volume(&into),
    );

    // Nine faces, twenty-one edges and fourteen corners, which Euler holds to a
    // ball: the block's twelve edges less the three replaced, six rulings, one
    // arc across each chamfer's far end, and the star's own three legs. No face
    // is raised at the corner, which is the whole of the difference from a
    // round one.
    let reckoning = into.reckoning();
    assert_eq!(
        reckoning.genus, 0,
        "a thrice-chamfered block is still a ball"
    );
    let topology = into.topology();
    assert_eq!(
        topology.faces().count(),
        9,
        "six faces and three chamfers, and nothing between them",
    );
    assert_eq!(topology.edges().count(), 21);
    assert_eq!(topology.vertices().count(), 14);
    assert_eq!(smooth(&into), 0, "no join of a chamfer is smooth");

    // The point the three planes cross at. Each stands a setback back from two
    // of the faces meeting at `(4, 4, 0)`, so the three cross half a setback
    // inside all three of them.
    let point = DVec3::new(3.5, 3.5, -0.5);
    assert_eq!(
        legs(&into, point),
        3,
        "three planes crossing leave one corner on three legs"
    );

    // And each pair of them crosses where their two rulings do on the face they
    // share, a setback back along both.
    for met in [
        DVec3::new(3.0, 3.0, 0.0),
        DVec3::new(3.0, 4.0, -1.0),
        DVec3::new(4.0, 3.0, -1.0),
    ] {
        assert!(
            stands(&into, met),
            "no corner of the answer stands at {met}, where a pair of the \
             chamfers cross on the face they share",
        );
    }
    assert!(into.exact(), "three planes crossing in a point stay exact");
}

/// The rod of `radius` with a flat milled down it through its own axis.
///
/// **What §7.4 made buildable**, and the one body in this file whose blend runs
/// out onto something that is not a plane: the flat meets the rod in a straight
/// edge, and the two faces stand square to each other there.
fn milled(radius: f64, deep: f64) -> Milled {
    flatted(radius, deep, 0.0)
}

/// A rod of `radius` carried `deep`, with a flat milled down it `stand` off its
/// own axis.
///
/// **Where it stands decides what a blend down the base rim closes on.** A
/// plane *through* the axis cuts the blend's torus in two circles, which
/// `Meeting::of` writes down exactly; one standing off it cuts a quartic no
/// exact route parameterizes, and the arc is marched. Both are wanted, so the
/// stand is the caller's.
fn flatted(radius: f64, deep: f64, stand: f64) -> Milled {
    let mut sketch = Sketch::default();
    let centre = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(centre, radius);
    let rod = Extrusion::new(&Arrangement::of(&sketch), &[0], Plane::GROUND, deep, SOLID).body();
    let wide = 2.0 * radius;
    let tool = block(
        Plane {
            origin: DVec3::new(0.0, -1.0, 0.0),
            ..Plane::GROUND
        },
        &[(stand, -wide), (wide, -wide), (wide, wide), (stand, wide)],
        deep + 2.0,
        TOOL,
    );
    let mut cut = Body::default();
    assert!(
        Boolean::default().combine(&rod, &tool, Operation::Cut, &mut cut),
        "milling a flat down a rod was refused",
    );
    let named = |wanted: fn(&Surface) -> bool| {
        cut.topology()
            .faces()
            .find(|(_, face)| wanted(&face.surface))
            .map(|(_, face)| face.name)
            .expect("the flatted rod has the face asked for")
    };
    Milled {
        base: named(|surface| {
            matches!(surface, Surface::Natural(Natural::Plane(plane))
                if plane.origin.abs_diff_eq(DVec3::ZERO, PLACED))
        }),
        round: named(|surface| matches!(surface, Surface::Natural(Natural::Cylinder(_)))),
        flat: named(|surface| {
            matches!(surface, Surface::Natural(Natural::Plane(plane))
                if plane.normal().x.abs() > 0.5)
        }),
        cut,
    }
}

/// A rod a flat was milled down, and the names a pick reaches its faces by.
#[derive(Debug)]
struct Milled {
    cut: Body,
    /// The rod's base, which the rim below runs round.
    base: Named,
    /// The rod's wall, one name over several patches: an extrusion splits a
    /// whole turn in half, and the flat then cuts each half again.
    round: Named,
    flat: Named,
}

/// **A blend onto a cylinder is a cylinder, and the body stays exact.**
///
/// A rolling ball touching two faces has its centre a reach inside each of
/// them, so where a blend's axis runs is where the two faces' *offsets* meet —
/// see [`Face::offset`](crate::solid::topology::face::Face). Offset a plane and
/// you get a plane, offset a cylinder and you get a cylinder, and a plane
/// parallel to a cylinder's axis meets it in a pair of straight lines. So the
/// axis is a line, the blend on it is a cylinder of the reach, and nothing here
/// leaves the exact tier — which `.notes/KERNEL.md` §7.5 used to say was not
/// so.
///
/// The rulings are straight for the same reason: one is the axis dropped onto
/// the flat and the other is the ruling of the rod the ball touches, and a
/// cylinder is made of straight lines.
///
/// **The arithmetic is the corner between a line and an arc.** The ball of
/// radius `r` sits at `x = −r` and `R − r` from the axis, so it stands
/// `y₀ = √((R−r)² − r²)` up the flat and `φ = asin(r/(R−r))` round the rod.
/// Writing the corner it leaves as a closed walk — up the flat, round the rod
/// through `φ`, back along the ball — the area comes to
/// `½(R² − r²)·φ − ¼πr² − ½r·y₀`. Which is `(1 − π/4)r²` as `R` grows, the
/// square corner two flats leave, and `0.0872911…` at `R = 2, r = ½`.
#[test]
fn a_blend_onto_a_cylinder_is_a_cylinder_and_stays_exact() {
    let (rod, reach, deep) = (2.0, 0.5, 3.0);
    let milled = milled(rod, deep);
    let along = [[milled.round, milled.flat]];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Round, ROUND),
            &milled.cut,
            &mut into
        ),
        "a blend between a plane and a cylinder was refused",
    );

    let up = ((rod - reach) * (rod - reach) - reach * reach).sqrt();
    let round_by = (reach / (rod - reach)).asin();
    let corner =
        0.5 * (rod * rod - reach * reach) * round_by - 0.25 * PI * reach * reach - 0.5 * reach * up;
    let want = deep * (PI * rod * rod / 2.0 - 2.0 * corner);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the filleted rod shuts in {} where {want} is the half rod less the two \
         corners",
        volume(&into),
    );

    // Two blends, one down each straight edge, and both on a cylinder of the
    // reach standing where the rolling ball's centre does.
    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(
        raised.len(),
        2,
        "one pick down two parallel edges is two blends"
    );
    for (_, face) in &raised {
        let Surface::Natural(Natural::Cylinder(cylinder)) = face.surface else {
            panic!("a blend between a plane and a cylinder lies on a cylinder, not {face:?}");
        };
        assert!(
            (cylinder.radius - reach).abs() < PLACED,
            "the blend has radius {}",
            cylinder.radius,
        );
        let axis = cylinder.axis;
        assert!(
            (axis.origin.x + reach).abs() < PLACED && (axis.origin.z.abs() - up).abs() < PLACED,
            "the blend's axis runs through {} rather than a reach inside the flat \
             and a reach inside the rod",
            axis.origin,
        );
    }

    // Ten corners, fifteen edges and seven faces, which Euler holds to a ball:
    // the milled rod's nine edges less the two replaced, four rulings and one
    // arc across each blend's two ends.
    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "a filleted half rod is still a ball");
    let topology = into.topology();
    assert_eq!(
        topology.faces().count(),
        7,
        "the milled rod's five and two blends"
    );
    assert_eq!(topology.edges().count(), 15);
    assert_eq!(topology.vertices().count(), 10);
    // Four more than the body already had, which is the rod's own seam: two
    // patches of one cylinder join smoothly, and the blend adds two such joins
    // apiece.
    assert_eq!(
        smooth(&into),
        smooth(&milled.cut) + 4,
        "a blend runs out smoothly onto a rod as onto a plane",
    );
    assert!(
        into.exact(),
        "a cylinder tangent to a plane and a cylinder is still a cylinder",
    );
}

/// **A chamfer onto a cylinder stands its setback along the face**, which is
/// the one thing "the reach back from the edge" can mean where the face curves.
///
/// A plane gives a straight step and a rod gives an arc — see
/// [`Surface::walked`](crate::solid::geometry::surface::Surface) — so the
/// ruling on the rod stands `reach/R` radians round it rather than a chord
/// away. The plane through the two rulings then holds the rod's edge direction,
/// so it meets the rod in exactly that ruling and the blend stays exact.
///
/// **The arithmetic is the triangle and the segment it cuts off.** Walking the
/// corner — down the flat by the setback, round the rod through `ψ = reach/R`,
/// and back along the chord — leaves `½R²ψ − ½R(R − reach)·sin ψ`. Which is
/// `½·reach²` as `R` grows, the square corner two flats leave, and
/// `0.1288940…` at `R = 2, reach = ½`.
#[test]
fn a_chamfer_onto_a_cylinder_stands_its_setback_along_the_face() {
    let (rod, reach, deep) = (2.0, 0.5, 3.0);
    let milled = milled(rod, deep);
    let along = [[milled.round, milled.flat]];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, reach, Bevel::Flat, ROUND),
            &milled.cut,
            &mut into
        ),
        "a chamfer between a plane and a cylinder was refused",
    );

    let round_by = reach / rod;
    let corner = 0.5 * rod * rod * round_by - 0.5 * rod * (rod - reach) * round_by.sin();
    let want = deep * (PI * rod * rod / 2.0 - 2.0 * corner);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the chamfered rod shuts in {} where {want} is the half rod less the two \
         corners",
        volume(&into),
    );

    let raised: Vec<_> = into.patches(ROUND.grew(Grown::Rounded(0))).collect();
    assert_eq!(
        raised.len(),
        2,
        "one pick down two parallel edges is two chamfers"
    );
    for (_, face) in &raised {
        assert!(
            matches!(face.surface, Surface::Natural(Natural::Plane(_))),
            "a chamfer lies on a plane, not {face:?}",
        );
    }

    // The rod's own edge runs down `x = 0` at `z = ±R`, and the setback goes
    // round the rod from there rather than across the chord — so the corner it
    // leaves stands a whole radius from the axis, not less.
    let touched = DVec3::new(-rod * round_by.sin(), 0.0, rod * round_by.cos());
    assert!(
        stands(&into, touched),
        "no corner of the answer stands at {touched}, where the setback reaches \
         round the rod",
    );

    assert_eq!(
        into.reckoning().genus,
        0,
        "a chamfered half rod is still a ball"
    );
    let topology = into.topology();
    assert_eq!(
        topology.faces().count(),
        7,
        "the milled rod's five and two chamfers"
    );
    assert_eq!(topology.edges().count(), 15);
    assert_eq!(
        smooth(&into),
        smooth(&milled.cut),
        "no join of a chamfer is smooth, onto a rod or onto a plane",
    );
    assert!(
        into.exact(),
        "a plane through two rulings of a cylinder is still a plane",
    );
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
        SOLID,
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
/// **Both fillings over the one fixture**, because both read the corner the
/// same way and the reading is what is under test: three fillets leave a patch
/// of a sphere and three chamfers leave a point, and a run that ends at either
/// stands on the patches its own tip divides.
///
/// **The arithmetic is the corner arithmetic off what the slot left.** The
/// blends stand half a unit off faces the slot comes no nearer than one, so
/// each takes off the forty-eight a two-by-two slot leaves exactly what it
/// would take off a whole block: `(1 − π/4)·r²·(l − r)` outside the corner cube
/// and `r³(1 − π/6)` of the cube for a fillet, and `3·½·s²·l − ¾·s³` for the
/// three chamfers.
#[test]
fn three_picks_meeting_on_a_body_a_boolean_cut_fill_the_corner_either_way() {
    let slotted = slotted();
    let along = at_a_corner(&slotted.names);
    assert_eq!(
        slotted.pieces(&along),
        [3, 3, 1],
        "the slot left the three picked edges in other than three, three and one",
    );
    let cut = &slotted.cut;

    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&along, 0.5, Bevel::Round, ROUND),
            cut,
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

    // And the same three picks as chamfers, which is the other filling a corner
    // takes: three planes meeting at a point rather than a patch of a sphere.
    // The same reading of the tips settles it, and the same runs feed it.
    assert!(
        Rounding::default().round(&Round::new(&along, 0.5, Bevel::Flat, ROUND), cut, &mut into),
        "three chamfers meeting at a corner of a cut body were refused",
    );
    let want = 48.0 - (3.0 * 0.5 * 0.25 * 4.0 - 0.75 * 0.125);
    assert!(
        (volume(&into) - want).abs() < CLOSES,
        "the chamfered slot shuts in {} where {want} is the three wedges less \
         what they share",
        volume(&into),
    );
    let point = DVec3::new(3.75, 3.75, -0.25);
    assert_eq!(
        legs(&into, point),
        3,
        "three planes crossing leave one corner on three legs"
    );
    assert_eq!(
        into.reckoning().genus,
        1,
        "a slot right through is a hole, chamfered or not",
    );
    assert!(into.exact(), "three planes crossing in a point stay exact");
}

/// The notch's step corner rounded at a reach of a half, which is the one
/// corner in this file the two picks do not agree about.
///
/// The fillet filled into the reflex edge below the floor and the round cut
/// into the convex edge above it stand a whole reach apart, so the two blends
/// touch at one place and leave the ruled patch between them.
fn gusseted() -> Body {
    let notched = notch();
    let picks = [
        between(
            &notched,
            DVec3::new(2.0, 0.0, -2.0),
            DVec3::new(2.0, 4.0, -2.0),
        ),
        between(
            &notched,
            DVec3::new(2.0, 0.0, -2.0),
            DVec3::new(0.0, 0.0, -2.0),
        ),
    ];
    let mut into = Body::default();
    assert!(
        Rounding::default().round(
            &Round::new(&picks, 0.5, Bevel::Round, ROUND),
            &notched,
            &mut into,
        ),
        "a corner two picks do not agree about was refused",
    );
    into
}

/// **Two picks that do not agree about a corner leave a ruled patch between
/// them**, where two that agree cross in an ellipse and leave no face at all.
///
/// The notch's step corner at a reach of a half. The floor is the face both
/// picks share: the fillet filled into the reflex edge below it stands its axis
/// a reach *under* the floor and the round cut into the convex edge above
/// stands its axis a reach *over* it, so the two stand a whole reach apart and
/// the cylinders touch at one place on the floor. `.notes/KERNEL.md` §7.7 is
/// where no quadric is shown to fill what they leave.
///
/// Every corner of the patch is hand-computed. The two rails on the floor are
/// `y = 0.5` and `x = 1.5`, so the touch point is `(1.5, 0.5, −2)`. The line
/// the two unshared planes cross in is `x = 2, y = 0`; the fillet's rail on the
/// face it does not share stands at `z = −2.5` and the round's at `z = −1.5`,
/// so the patch's other two corners are `(2, 0, −2.5)` and `(2, 0, −1.5)`.
///
/// **And the answer is no longer exact.** The patch's edge on the round is
/// walked, so the body carries what that walk strayed — §4.1's tier read off
/// the answer rather than assumed about it.
#[test]
fn two_picks_that_do_not_agree_leave_a_ruled_patch() {
    let into = gusseted();

    let reckoning = into.reckoning();
    assert_eq!(reckoning.genus, 0, "the notch is still a ball");
    let topology = into.topology();
    assert_eq!(
        topology.faces().count(),
        11,
        "the notch's eight faces, a blend apiece and the patch between them",
    );
    assert!(
        !into.exact(),
        "the patch's walked edge puts the answer in the fitted tier",
    );
    assert!(into.strays() > 0.0, "a walked edge that strays by nothing");

    // The patch answers to the two picks that met, the filled one first, and
    // one face of the body carries that name.
    let name = ROUND.grew(Grown::Gusseted([0, 1]));
    assert_eq!(into.patches(name).count(), 1, "{name:?} came apart");
    let (raised, face) = into.patches(name).next().expect("the pair raised a patch");
    let Surface::Fitted(Fitted::Gusset(patch)) = face.surface else {
        panic!("the patch does not lie on a ruled surface");
    };

    let wanted = [
        DVec3::new(1.5, 0.5, -2.0),
        DVec3::new(2.0, 0.0, -2.5),
        DVec3::new(2.0, 0.0, -1.5),
    ];
    assert!(
        patch.singular(wanted[0]),
        "the patch does not close at the tip"
    );
    assert!(patch.from.abs_diff_eq(wanted[1], 1e-12), "{}", patch.from);
    let [from, to] = patch.bounds();
    let landed = patch.at(DVec2::new(from, 1.0));
    assert!(landed.abs_diff_eq(wanted[2], 1e-9), "{landed}");
    for step in 0..=8 {
        let u = from + (to - from) * f64::from(step) / 8.0;
        let head = patch.at(DVec2::new(u, 0.0));
        let foot = patch.at(DVec2::new(u, 1.0));
        assert!(
            (patch.filled.axis.off(head) - 0.5).abs() < 1e-9,
            "{head} is off the fillet",
        );
        assert!(
            (patch.cut.axis.off(foot) - 0.5).abs() < 1e-9,
            "{foot} is off the round",
        );
    }

    // Three sides, and the corners they run between are the three above.
    let walk: Vec<_> = topology
        .loops_of(face)
        .flatten()
        .map(|coedge| coedge.edge)
        .collect();
    assert_eq!(walk.len(), 3, "the patch is three-sided");
    let mut corners: Vec<[i64; 3]> = walk
        .iter()
        .flat_map(|&edge| topology.edge(edge).ends(true))
        .map(|end| keyed(topology.vertex(end).at))
        .collect();
    corners.sort_unstable();
    corners.dedup();
    let mut want: Vec<[i64; 3]> = wanted.map(keyed).into();
    want.sort_unstable();
    assert_eq!(
        corners, want,
        "the patch does not run between its own corners"
    );

    // **One of the three is a crease, and it is the straight side.** The patch
    // runs out tangent to each blend along its own two curved edges, so those
    // two are joins rather than corners; the side between them is a ruling with
    // the cut blend's unshared face across it, and the patch's normal swings
    // from one blend's to the other's along it. What lets the reading see the
    // two joins is the room it takes off each surface rather than off a bare
    // constant — see `Face::smooth` and `Surface::wavering`.
    let creases = walk
        .iter()
        .filter(|&&edge| !topology.edge(edge).artificial)
        .count();
    assert_eq!(creases, 1, "the flag on a patch's joins moved");
    assert!(
        walk.iter()
            .all(|&edge| topology.edge(edge).between.contains(&raised)),
        "an edge of the patch's own loop does not bound it",
    );
}

/// A place written down to the width the kernel places one at, so two readings
/// of the same corner key alike.
fn keyed(at: DVec3) -> [i64; 3] {
    [at.x, at.y, at.z].map(|of| (of / PLACED).round() as i64)
}

/// **The mesher cuts a face on a ruled patch, and what it cuts follows the
/// patch** — the last reading of `.notes/KERNEL.md` §7.7's tier to be asked of
/// this surface.
///
/// **Every corner lands on the patch, to what the body says it strays.** An
/// inside corner is evaluated on the surface and lands on it exactly; a corner
/// off the boundary is a place on a chord of the walked second edge, and a
/// chord of that walk is what the whole body's exactness is measured by. So the
/// mesh is as true as the edge it was cut between, which is §4.1's tier read
/// off a triangulation.
///
/// **And it converges on an area a second reading agrees with.** The mesh
/// falls at every step of the chording and settles near `0.4362`; a quadrature
/// over the surface's own parameters, which reaches the same number from a
/// direction a traced boundary does not, comes to `0.43622`. Falling rather
/// than rising is measured rather than argued — nothing says a ruling between
/// two chords could not read long.
///
/// **And it holds every triangle to the sagitta down to what the walked edge
/// costs.** A face's boundary may not be cut, so an edge arrives chorded as
/// finely as the finer of its two faces asks — see
/// [`Walked::steps`](crate::solid::topology::Walked). The patch's straight side
/// is a whole ruling and a line is exact however coarsely it is cut, so its own
/// rule asks for one chord where the grid wants seventy-one; that is the run
/// this rule mends.
///
/// **What is left is the second edge**, which is a marched run and cannot be
/// laid down again. A triangle carrying one of its chords is as wide as that
/// chord, so the mesh settles at what the *body* says it strays rather than at
/// what was asked for. Measured at a reach of a half: `5.5e-3` of a sagitta of
/// a hundredth, `6.9e-4` of a thousandth, and `5.3e-4` of a ten-thousandth —
/// the last of which is the `3.9e-4` the walk carries and no finer.
#[test]
fn a_face_on_a_ruled_patch_is_cut_into_triangles_that_follow_it() {
    let into = gusseted();
    let strays = into.strays();
    assert!(
        (strays - 3.888e-4).abs() < 1e-6,
        "the walk of the patch's second edge moved: {strays}",
    );
    let name = ROUND.grew(Grown::Gusseted([0, 1]));
    let (at, face) = into.patches(name).next().expect("the pair raised a patch");

    // The coarsest of the three first, and the two readings that cost a search
    // are taken there alone: a nearest place on this surface is sought rather
    // than solved, and the corners of the walked edge stand where they stand
    // whatever sagitta is asked for.
    let mut mesher = Mesher::default();
    let mut patch = Patch::default();
    mesher.shut_in(&into, &[at], 1e-2, &mut patch);
    let worst = patch
        .corners
        .iter()
        .map(|&at| face.surface.off(at))
        .fold(0.0, f64::max);
    assert!(
        worst <= strays,
        "a corner stands {worst} off the patch, past the {strays} the body carries",
    );

    let mut wide = 0.0_f64;
    for &[a, b, c] in &patch.triangles {
        let [a, b, c] = [a, b, c].map(|of| patch.corners[of as usize]);
        // The middle and the three sides: a flat triangle over a ruled patch
        // leaves it furthest on its own boundary — see [`Gusset::straying`] —
        // and a middle alone reads two thirds of the answer here.
        for on in [
            (a + b + c) / 3.0,
            (a + b) / 2.0,
            (b + c) / 2.0,
            (c + a) / 2.0,
        ] {
            wide = wide.max(face.surface.off(on));
        }
    }
    assert!(
        (wide - 5.5e-3).abs() < 1e-3,
        "the patch's coarsest triangle moved: {wide}",
    );

    let mut cut = patch.triangles.len();
    let mut last = covered(&patch);
    for sagitta in [1e-3, 1e-4] {
        mesher.shut_in(&into, &[at], sagitta, &mut patch);
        assert!(
            patch.triangles.len() > cut,
            "a finer sagitta cut no more finely: {}",
            patch.triangles.len(),
        );
        cut = patch.triangles.len();
        let area = covered(&patch);
        assert!(
            area < last,
            "{sagitta} read no nearer than the last: {area}"
        );
        last = area;
    }
    // **Against a second reading rather than against a number this mesh once
    // gave.** A quadrature over the patch's own parameters covers the same
    // surface without tracing a boundary at all, so the two agreeing is the
    // mesh being right rather than being the same as it was.
    let quadrature = integrated(face);
    assert!(
        (last - quadrature).abs() < 1e-3,
        "the mesh covers {last} where a quadrature says {quadrature}",
    );
    assert!(
        (quadrature - 0.43622).abs() < 1e-4,
        "the quadrature moved: {quadrature}",
    );
}

/// How much area the triangles of `patch` cover.
fn covered(patch: &Patch) -> f64 {
    patch
        .triangles
        .iter()
        .map(|&[a, b, c]| {
            let [a, b, c] = [a, b, c].map(|of| patch.corners[of as usize]);
            (b - a).cross(c - a).length() * 0.5
        })
        .sum()
}

/// **An edge arrives chorded as finely as the finer of its two faces asks**,
/// which is what a boundary owes a middle no pass may cut it into.
///
/// The patch's three edges are the whole of the rule in one face.
///
/// - **The straight side is a ruling and a line, so its own rule asks for one
///   chord** however fine the sagitta — straight is exact however coarsely it is
///   cut. The patch's grid wants a cell for every `0.0140514` of the run along
///   the ruling, and the side covers the whole of it, so seventy-two is what it
///   is laid down in.
/// - **The first edge is an ellipse**, chorded at forty-seven by its own major
///   semi-axis, where the patch's own angle wants a hundred and forty-eight —
///   `1.5707963 / 0.0106135`, its whole turn over its own step.
/// - **The second edge is a marched run and keeps its thirty-two.** It is its
///   chords rather than a curve they stand for, so a step between two of them
///   lands on a chord and off both surfaces — see [`Curve::divisible`].
///
/// **And every edge of the body answers alike from both sides**, which is the
/// one thing that lets a face ask for more at all: a count that differed by
/// side would leave one face holding a corner its neighbour has not got.
#[test]
fn an_edge_is_laid_down_as_finely_as_either_face_it_lies_between_asks() {
    let into = gusseted();
    let topology = into.topology();
    let name = ROUND.grew(Grown::Gusseted([0, 1]));
    let (raised, face) = into.patches(name).next().expect("the pair raised a patch");
    let Surface::Fitted(Fitted::Gusset(patch)) = face.surface else {
        panic!("the patch does not lie on a ruled surface");
    };

    let strides = patch.strides(SAGITTA);
    assert!(
        (strides - DVec2::new(0.0106135, 0.0140514))
            .abs()
            .max_element()
            < 1e-6,
        "the patch's own grid moved: {strides}",
    );
    let [from, to] = patch.bounds();
    let wanted = [
        (1.0 / strides.y).ceil() as usize,
        ((to - from).abs() / strides.x).ceil() as usize,
    ];
    assert_eq!(wanted, [72, 148], "the cells the patch asks for moved");

    let mut counted = Vec::new();
    for (at, edge) in topology.edges() {
        // Both sides of every edge, which is the claim the rule stands on.
        let both: Vec<usize> = edge
            .between
            .iter()
            .map(|&of| {
                let coedge = topology
                    .loops_of(topology.face(of))
                    .flatten()
                    .find(|coedge| coedge.edge == at)
                    .copied()
                    .expect("a face on an edge is a face that walks it");
                topology.walked(coedge).steps(SAGITTA)
            })
            .collect();
        assert_eq!(both[0], both[1], "an edge is laid down two ways");
        if edge.between.contains(&raised) {
            counted.push((edge.steps(SAGITTA, topology.carried()), both[0]));
        }
    }
    counted.sort_unstable();
    assert_eq!(
        counted,
        [(1, 72), (32, 32), (47, 148)],
        "the patch's own three edges are not laid down as its grid asks",
    );
}

/// How much area `face` covers, summed over its own parameters.
///
/// **A second reading of the one number**, and the reason it is worth having:
/// a mesh reads an area off a boundary somebody traced, where this reads it off
/// the surface. The two agreeing says the mesh follows the surface; a mesh held
/// against its own last answer says only that nothing moved.
///
/// Coarse, because it is only ever compared against a mesh to a thousandth:
/// this grid reads `0.43622` where two thousand by two hundred and fifty reads
/// `0.43611`.
fn integrated(face: &Face) -> f64 {
    const ACROSS: usize = 200;
    const ALONG: usize = 25;

    let Surface::Fitted(Fitted::Gusset(patch)) = face.surface else {
        panic!("the quadrature is written for a ruled patch");
    };
    let [from, to] = patch.bounds();
    let at = |iu: usize, iv: usize| {
        let u = from + (to - from) * iu as f64 / ACROSS as f64;
        patch.at(DVec2::new(u, iv as f64 / ALONG as f64))
    };
    let mut area = 0.0;
    for iu in 0..ACROSS {
        for iv in 0..ALONG {
            let (a, b) = (at(iu, iv), at(iu + 1, iv));
            let (c, d) = (at(iu + 1, iv + 1), at(iu, iv + 1));
            area += ((b - a).cross(c - a).length() + (c - a).cross(d - a).length()) * 0.5;
        }
    }
    area
}

/// **Two chamfers that do not agree about a corner leave a triangle**, where
/// two rounds leave the ruled patch of `.notes/KERNEL.md` §7.7.
///
/// The same corner as that patch, at the same reaches. **The three corners are
/// the same three** — the touch point where the two rails cross on the floor,
/// and a reach either way along the third edge's line — because none of them
/// depends on what runs between the blends. What the bevel decides is the
/// surface, and three points name one plane.
///
/// **And the answer stays exact.** A chamfer creases against everything it
/// meets, so the filling owes its neighbours nothing but their shared corners:
/// no tangency to keep, no curve to walk, and no reason for the body to leave
/// the exact tier the way the round pair does.
///
/// **The volume is `48 + r²`, hand-computed.** The fill puts a right prism of
/// cross-section `r²/2` down the whole four of the reflex edge and the cut
/// takes one off the whole two of the convex edge, so the two come to
/// `4·r²/2 − 2·r²/2`. The corner adds nothing to either, which is what the
/// triangle's own plane running *through* the body's corner says: it stands a
/// reach either side of it on the third edge's line, and the two wedges that
/// leaves are mirror images.
#[test]
fn two_chamfers_that_do_not_agree_leave_a_triangle() {
    let notched = notch();
    let picks = [
        between(
            &notched,
            DVec3::new(2.0, 0.0, -2.0),
            DVec3::new(2.0, 4.0, -2.0),
        ),
        between(
            &notched,
            DVec3::new(2.0, 0.0, -2.0),
            DVec3::new(0.0, 0.0, -2.0),
        ),
    ];
    for reach in [0.25, 0.5, 0.75, 1.0] {
        let mut into = Body::default();
        assert!(
            Rounding::default().round(
                &Round::new(&picks, reach, Bevel::Flat, ROUND),
                &notched,
                &mut into,
            ),
            "a corner two chamfers do not agree about was refused at {reach}",
        );
        assert_eq!(into.reckoning().genus, 0, "the notch is still a ball");
        assert_eq!(
            into.topology().faces().count(),
            11,
            "the notch's eight faces, a chamfer apiece and the triangle between",
        );
        assert!(
            into.exact(),
            "two planes and a plane between them stay exact"
        );
        let want = 48.0 + reach * reach;
        assert!(
            (volume(&into) - want).abs() < CLOSES,
            "the chamfered corner shuts in {} where the arithmetic says {want}",
            volume(&into),
        );

        let name = ROUND.grew(Grown::Gusseted([0, 1]));
        assert_eq!(into.patches(name).count(), 1, "{name:?} came apart");
        let (raised, face) = into.patches(name).next().expect("the pair raised one");
        let Surface::Natural(Natural::Plane(plane)) = face.surface else {
            panic!("two chamfers left something that is not a plane");
        };
        // **Through the corner it swallowed**, which is what a chamfer between
        // the two unshared faces looks like: it bisects the wedge they leave
        // rather than cutting the corner off.
        let corner = DVec3::new(2.0, 0.0, -2.0);
        let off = (corner - plane.origin).dot(plane.normal()).abs();
        assert!(off < PLACED, "the triangle stands {off} off the corner");

        // The same three corners the ruled patch leaves, and all three of its
        // sides are creases — a chamfer meets everything at an angle.
        let topology = into.topology();
        let walk: Vec<_> = topology
            .loops_of(face)
            .flatten()
            .map(|coedge| coedge.edge)
            .collect();
        assert_eq!(walk.len(), 3, "the triangle is three-sided");
        assert_eq!(
            walk.iter()
                .filter(|&&edge| !topology.edge(edge).artificial)
                .count(),
            3,
            "a chamfer creases against everything it meets",
        );
        assert!(
            walk.iter()
                .all(|&edge| topology.edge(edge).between.contains(&raised)),
            "an edge of the triangle's own loop does not bound it",
        );
        let mut corners: Vec<[i64; 3]> = walk
            .iter()
            .flat_map(|&edge| topology.edge(edge).ends(true))
            .map(|end| keyed(topology.vertex(end).at))
            .collect();
        corners.sort_unstable();
        corners.dedup();
        let mut want: Vec<[i64; 3]> = [
            DVec3::new(2.0 - reach, reach, -2.0),
            DVec3::new(2.0, 0.0, -2.0 - reach),
            DVec3::new(2.0, 0.0, -2.0 + reach),
        ]
        .map(keyed)
        .into();
        want.sort_unstable();
        assert_eq!(
            corners, want,
            "the triangle does not run between its own corners"
        );
    }
}

/// **Three chamfers meeting at a corner leave a star whether or not the picks
/// agree**, which is the whole of what a chamfer has over a round here: three
/// planes cross at one point however each of them was cut, where a rolling
/// ball has to stay on one side of the material and so has no sphere to leave.
/// `.notes/KERNEL.md` §7.5 is where the two part company.
///
/// The notch's step corner with all three of its edges picked: the reflex one
/// filled into the notch, and the two the cap makes with the floor and the wall
/// cut out of it. Every place below is hand-computed off the three planes.
///
/// **The three planes and where they cross.** Measure `(a, b, c)` from the
/// corner along `+x`, `+y` and `+z`. The fill stands a reach back along the
/// floor and the wall, so it is `a + c = −r`; the floor's chamfer stands a
/// reach along the floor and the cap, so it is `b + c = r`; the wall's stands a
/// reach along the wall and the cap, so it is `a + b = r`. Those cross at
/// `(−r/2, 3r/2, −r/2)`, and each pair crosses again where its own two rails
/// cross on the face the pair shares.
///
/// **The volume is `48 − 11r³/12`, hand-computed.** The three prisms cancel —
/// the fill puts `r²/2` down the whole four of the reflex edge and each cut
/// takes `r²/2` off the whole two of its own — so what is left is the corner.
/// The two cuts reach across the fill as far as the planes above let them:
/// `∫∫(r − max(a, c))` over the fill's own triangle is `7r³/12` off what the
/// fill puts back, and the cuts take `r³/3` more than their prisms where they
/// run past the corner onto the cap.
#[test]
fn three_chamfers_that_do_not_agree_leave_a_star() {
    let notched = notch();
    let corner = DVec3::new(2.0, 0.0, -2.0);
    let picks = [
        between(&notched, corner, DVec3::new(2.0, 4.0, -2.0)),
        between(&notched, corner, DVec3::new(0.0, 0.0, -2.0)),
        between(&notched, corner, DVec3::new(2.0, 0.0, -4.0)),
    ];
    for reach in [0.25, 0.5, 0.75, 1.0] {
        let mut into = Body::default();
        assert!(
            Rounding::default().round(
                &Round::new(&picks, reach, Bevel::Flat, ROUND),
                &notched,
                &mut into,
            ),
            "a corner three chamfers do not agree about was refused at {reach}",
        );

        assert_eq!(into.reckoning().genus, 0, "the notch is still a ball");
        assert!(into.exact(), "three planes crossing in a point stay exact");
        assert_eq!(smooth(&into), 0, "no join of a chamfer is smooth");

        // Eleven faces, twenty-seven edges and eighteen corners, which Euler
        // holds to a ball. The notch's eight faces and a chamfer apiece, and no
        // face at the corner — where three *rounds* would leave a patch of a
        // sphere.
        let topology = into.topology();
        assert_eq!(
            topology.faces().count(),
            11,
            "the notch's eight faces, a chamfer apiece and nothing between",
        );
        assert_eq!(topology.edges().count(), 27);
        assert_eq!(topology.vertices().count(), 18);

        let want = 48.0 - 11.0 * reach * reach * reach / 12.0;
        assert!(
            (volume(&into) - want).abs() < CLOSES,
            "the starred corner shuts in {} where the arithmetic says {want}",
            volume(&into),
        );

        // The point the three planes cross at, on three legs and no more.
        let point = corner + DVec3::new(-reach / 2.0, 1.5 * reach, -reach / 2.0);
        assert_eq!(
            legs(&into, point),
            3,
            "three planes crossing leave one corner on three legs at {reach}",
        );

        // And each pair of them crosses where its two rails do on the face it
        // shares: the fill against the floor's cut on the floor, the fill
        // against the wall's cut on the wall, and the two cuts on the cap.
        for met in [
            corner + DVec3::new(-reach, reach, 0.0),
            corner + DVec3::new(0.0, reach, -reach),
            corner + DVec3::new(reach, 0.0, reach),
        ] {
            assert!(
                stands(&into, met),
                "no corner of the answer stands at {met}, where a pair of the \
                 chamfers cross on the face they share",
            );
        }

        // **And the round is still refused there.** A sphere between three
        // blends stands a reach off all three faces on one side, and a corner
        // whose picks name two sides has none for it to be on.
        let mut rounded = Body::default();
        assert!(
            !Rounding::default().round(
                &Round::new(&picks, reach, Bevel::Round, ROUND),
                &notched,
                &mut rounded,
            ),
            "a sphere was fitted at a corner three rounds do not agree about",
        );
    }
}
