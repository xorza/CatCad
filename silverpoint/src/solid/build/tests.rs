use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use crate::solid::build::extrusion::Extrusion;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::mesh::Mesher;
use crate::solid::named::{Named, Step};
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};
use std::f64::consts::PI;

/// The step every body below is grown by.
///
/// Which one it is says nothing here: what a step tells apart is one feature's
/// faces from another's, and every test in this file raises one body.
const STEP: Step = Step(0);

/// How fine the walls and caps below are cut when a volume is read off them.
///
/// Fine enough that a flattened circle lands within a rounding of the true one
/// at the sizes here, so an arc's answer can be checked against the area it
/// really encloses rather than against whatever the flattening happened to
/// give.
const FINE: f64 = 1e-5;

/// How much space a body shuts in, read off its triangles.
fn volume(of: &Body) -> f64 {
    Mesher::default().volume(of, FINE)
}

/// **An extrusion closes the right way out, whichever way it grows.**
///
/// A two-by-two square carried three deep is twelve, and the sign is the half
/// worth having: every face of it — two caps and four walls — has to be wound
/// counterclockwise seen from *outside*, and any one of them turned over would
/// take its own contribution off instead of adding it.
///
/// Then the same solid grown the other way. A negative distance is the same
/// solid on the other side of the plane, so it encloses the same twelve and is
/// no more inside out — which is the whole reason every winding follows the
/// sign rather than being fixed one way. And then off the world's own origin,
/// since a volume read this way must not depend on where that is.
#[test]
fn an_extrusion_closes_the_right_way_out_whichever_way_it_grows() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 1);

    let up = Extrusion::new(&found, 0, Plane::GROUND, 3.0, STEP).body();
    let faces: Vec<Named> = up.names().collect();
    assert_eq!(faces.len(), 6, "{faces:?}");
    assert_eq!(faces[0], STEP.grew(Grown::Base));
    assert_eq!(faces[1], STEP.grew(Grown::Far));
    assert!(
        (volume(&up) - 12.0).abs() < 1e-9,
        "it shut in {}",
        volume(&up)
    );

    let down = Extrusion::new(&found, 0, Plane::GROUND, -3.0, STEP).body();
    assert!(
        (volume(&down) - 12.0).abs() < 1e-9,
        "grown against the normal it shut in {}, so it is inside out",
        volume(&down),
    );

    let raised = Plane {
        origin: DVec3::new(-7.0, 4.0, 11.0),
        ..Plane::FRONT
    };
    let elsewhere = Extrusion::new(&found, 0, raised, 3.0, STEP).body();
    assert!((volume(&elsewhere) - 12.0).abs() < 1e-9);
}

/// **A box is eight corners, twelve edges and six faces**, and its shell is a
/// sphere.
///
/// Euler–Poincaré, `V − E + F − R = 2(S − G)`, asserted on the numbers rather
/// than on the checker having passed: `8 − 12 + 6 − 0 = 2`, so the genus is
/// nought. It is the one reading that asks the whole of a shell at once, and it
/// catches what no local check can — a face left out, an edge counted into the
/// wrong loop.
///
/// **Every one of its twelve edges is a real crease.** Nobody drew the four
/// upright ones, but a fillet would round each of them and an export has to
/// keep them — which is why the flag that says "no crease here" is about the
/// two surfaces meeting rather than about which pass raised the edge.
#[test]
fn a_box_counts_up_to_a_sphere_and_knows_which_edges_nobody_drew() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    let body = Extrusion::new(&found, 0, Plane::GROUND, 3.0, STEP).body();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.characteristic, 2, "{reckoning:?}");
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");

    let topology = body.topology();
    assert_eq!(topology.faces().count(), 6);
    assert_eq!(topology.edges().count(), 12);

    let smooth = topology.edges().filter(|(_, it)| it.artificial).count();
    assert_eq!(
        smooth, 0,
        "a box has no smooth edges: every one is a corner"
    );
    for (id, edge) in topology.edges() {
        let straight = matches!(edge.curve, Curve::Line(_));
        assert!(straight, "edge {id:?} came off a square curved");
    }
}

/// **A hole is carried through, and the wall it raises faces inward.**
///
/// A square with a square bore has a shell of genus one — `16 − 24 + 10 − 2 =
/// 0`, the two rings being the bore's loop in each cap — and the volume of the
/// block less the volume of the bore. The volume is the half that says the
/// bore's four walls face *into* the hole: turned the other way they would add
/// where they subtract, and the answer would come out over rather than under.
#[test]
fn a_hole_is_carried_through_and_its_walls_face_into_it() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (6.0, 0.0), (6.0, 6.0), (0.0, 6.0)]);
    sketch.outline(&[(2.0, 2.0), (4.0, 2.0), (4.0, 4.0), (2.0, 4.0)]);
    let found = Arrangement::of(&sketch);
    let ring = found
        .faces()
        .iter()
        .position(|face| face.holes() == 1)
        .expect("the bore is a hole of the block");

    let body = Extrusion::new(&found, ring, Plane::GROUND, 5.0, STEP).body();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.characteristic, 0, "{reckoning:?}");
    assert_eq!(reckoning.genus, 1, "a bore is one handle: {reckoning:?}");
    assert_eq!(body.topology().faces().count(), 10, "two caps, eight walls");

    // Ten faces and ten names: every wall is one whole curve of the drawing.
    assert_eq!(body.names().count(), 10);
    let want = (36.0 - 4.0) * 5.0;
    assert!(
        (volume(&body) - want).abs() < 1e-9,
        "it shut in {}",
        volume(&body)
    );
}

/// **A circle raises two walls with one name**, and the solid is a cylinder.
///
/// No face of a body may cover its own surface the whole way round — see
/// `.notes/KERNEL.md` §4.4 — so a full circle is swept into two half cylinders
/// with an upright edge nobody drew between them. Both carry the same
/// [`Grown::Side`], because a wall is named by the curve it came off rather
/// than by how the kernel had to cut it, so nothing above can tell.
///
/// The surface is asked for its own radius and axis rather than sampled: a
/// cylinder here is exact, and reading it back off the parameters is what says
/// so.
#[test]
fn a_circle_raises_two_walls_with_one_name_on_one_exact_cylinder() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(3.0, 1.0));
    let ring = sketch.add_circle(middle, 2.0);
    let found = Arrangement::of(&sketch);
    let body = Extrusion::new(&found, 0, Plane::GROUND, 4.0, STEP).body();

    // Three names — a base, a far end and one wall — over four faces.
    let named: Vec<Named> = body.names().collect();
    let wall = STEP.grew(Grown::Side(Bound {
        of: Entity::Circle(ring),
        along: true,
    }));
    let (base, far) = (STEP.grew(Grown::Base), STEP.grew(Grown::Far));
    assert_eq!(named, [base, far, wall], "{named:?}");
    assert_eq!(body.topology().faces().count(), 4, "the wall was not split");
    assert_eq!(body.patches(wall).count(), 2, "one face covers the turn");

    // And the body answers for exactly those three, which is the question
    // anything keeping hold of a face across an edit asks. The wall walked the
    // other way is the name of a face this body does not have.
    for name in [base, far, wall] {
        assert!(body.holds(name), "{name:?} is a face and was not held");
    }
    let backwards = STEP.grew(Grown::Side(Bound {
        of: Entity::Circle(ring),
        along: false,
    }));
    assert!(!body.holds(backwards), "a name it never grew");
    assert!(
        !body.holds(Step(STEP.0 + 1).grew(Grown::Base)),
        "another step"
    );

    // Each half lies on the same exact cylinder: the drawing's own centre
    // carried onto the plane, radius two, running along the plane's normal.
    for (id, face) in body.patches(wall) {
        let Surface::Cylinder(cylinder) = face.surface else {
            panic!("face {id:?} swept a circle into something else");
        };
        assert_eq!(cylinder.radius, 2.0);
        assert_eq!(
            cylinder.axis.origin,
            Plane::GROUND.point(DVec2::new(3.0, 1.0))
        );
        assert_eq!(cylinder.axis.direction, Plane::GROUND.normal());
        assert!(face.outward, "the wall of a disc faces out of the axis");
    }

    // A shell of genus nought, two corners and six edges: two arcs at each end
    // and the two upright ones the split raised.
    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 0, "{reckoning:?}");
    assert_eq!(body.topology().edges().count(), 6);
    // The two upright edges are where the split fell, and nothing creases
    // there: both walls lie on the one cylinder, so a picture and an export
    // may pass straight over them.
    let smooth = body
        .topology()
        .edges()
        .filter(|(_, it)| it.artificial)
        .count();
    assert_eq!(smooth, 2, "the split left a crease behind");

    // Under the truth rather than merely near it: a flattened circle is
    // inscribed in the real one, so a volume read off triangles can only
    // undershoot. How far under is the caller's sagitta, not the body's.
    let want = PI * 4.0 * 4.0;
    let read = volume(&body);
    assert!(read < want, "a chorded circle read over the true {want}");
    assert!((want - read) < 1e-3, "it shut in {read} against {want}");
}

/// **A corner drawn straight through leaves no crease.**
///
/// A polyline that carries on in the same direction past a vertex raises two
/// walls that are one plane — the same normal, different origins — so the
/// upright edge between them is a place where two faces meet smoothly rather
/// than a corner. A display may pass over it and an export may merge across it,
/// and neither could if the two surfaces were told apart by comparing their
/// descriptions instead of their geometry. See
/// [`Meeting::Same`](crate::solid::meeting::Meeting).
#[test]
fn a_corner_drawn_straight_through_leaves_no_crease() {
    let mut sketch = Sketch::default();
    // Four by three, with the bottom drawn as two segments end to end.
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]);
    let found = Arrangement::of(&sketch);
    let body = Extrusion::new(&found, 0, Plane::GROUND, 2.0, STEP).body();

    let smooth: Vec<_> = body
        .topology()
        .edges()
        .filter(|(_, edge)| edge.artificial)
        .collect();
    assert_eq!(smooth.len(), 1, "{smooth:?}");
    // And it is the one standing where the bottom runs straight on, rather than
    // at any of the four real corners.
    let (_, edge) = smooth[0];
    let standing = body.topology().vertex(edge.from).at;
    assert!(
        standing.distance(Plane::GROUND.point(DVec2::new(2.0, 0.0))) < 1e-12,
        "the smooth edge stands at {standing:?}",
    );

    // Five walls, because the drawing has five curves — one name each, and the
    // straight-through corner takes none of them away.
    assert_eq!(body.names().count(), 7, "two caps and five walls");
    assert!((volume(&body) - 24.0).abs() < 1e-9);
}

/// **A spur raises no wall**, because it has no thickness to raise one from.
///
/// A line dangling into a region is walked out and straight back, so it bounds
/// nothing at all — and a solid grown from that region has exactly the faces it
/// had before the line was drawn. Which is the same rule that keeps a name from
/// moving, read on the solid rather than on the drawing: nothing built on a
/// wall is lost by somebody drawing a stray line inside the profile.
///
/// Left in, it would be a wall of no width, walked twice by one loop — which
/// the checker would refuse outright.
#[test]
fn a_spur_dangling_into_a_region_raises_no_wall() {
    let mut sketch = Sketch::default();
    let corners = sketch.outline(&[(0.0, 0.0), (4.0, 0.0), (4.0, 4.0), (0.0, 4.0)]);
    let plain = Extrusion::new(&Arrangement::of(&sketch), 0, Plane::GROUND, 2.0, STEP).body();

    let tip = sketch.add_point(DVec2::new(2.0, 2.0));
    sketch.add_segment(corners[0], tip);
    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 1, "the spur enclosed something");
    let spurred = Extrusion::new(&found, 0, Plane::GROUND, 2.0, STEP).body();

    let before: Vec<Named> = plain.names().collect();
    let after: Vec<Named> = spurred.names().collect();
    assert_eq!(before, after, "the spur changed what the solid is made of");
    assert_eq!(spurred.topology().faces().count(), 6);
    assert!((volume(&spurred) - 32.0).abs() < 1e-9);
}

/// **A curve cut into pieces raises one wall in several patches.**
///
/// A notch standing on the bottom edge of a bar cuts that edge into three, and
/// the region outside the notch is bounded by the two outer pieces — the same
/// curve, the same side, twice. It raises *one* wall out of two patches, which
/// is the decision [`Grown::Side`] already makes carried into the body: one
/// wall per bound rather than one per piece of curve, so the count of a solid's
/// faces does not move every time something new is drawn across the drawing.
#[test]
fn a_curve_cut_in_two_raises_one_wall_out_of_two_patches() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (6.0, 0.0), (6.0, 4.0), (0.0, 4.0)]);
    let up = sketch.add_point(DVec2::new(2.0, 0.0));
    let over = sketch.add_point(DVec2::new(2.0, 1.0));
    let down = sketch.add_point(DVec2::new(4.0, 1.0));
    let back = sketch.add_point(DVec2::new(4.0, 0.0));
    sketch.add_segment(up, over);
    sketch.add_segment(over, down);
    sketch.add_segment(down, back);
    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 2, "the notch is a region of its own");

    // Both regions together are the whole bar, which is what says neither lost
    // a piece of the boundary it shares.
    let together: f64 = (0..2)
        .map(|at| volume(&Extrusion::new(&found, at, Plane::GROUND, 3.0, STEP).body()))
        .sum();
    assert!(
        (together - 72.0).abs() < 1e-9,
        "the pieces cover {together}"
    );

    let bar = (0..2)
        .map(|at| Extrusion::new(&found, at, Plane::GROUND, 3.0, STEP).body())
        .find(|body| body.names().any(|name| body.patches(name).count() == 2))
        .expect("no region is walled by a curve in two pieces");
    let doubled: Vec<Named> = bar
        .names()
        .filter(|&name| bar.patches(name).count() == 2)
        .collect();
    assert_eq!(doubled.len(), 1, "{doubled:?}");
    assert!(matches!(doubled[0].grown, Grown::Side(_)), "{doubled:?}");

    // Two patches, one surface: both lie in the plane of the bottom edge and
    // face the same way out of it, which is what makes them one wall rather
    // than two that happen to share a name.
    let facing: Vec<DVec3> = bar
        .patches(doubled[0])
        .map(|(_, face)| face.normal(DVec2::ZERO))
        .collect();
    assert_eq!(facing.len(), 2);
    assert!(facing[0].distance(facing[1]) < 1e-12, "{facing:?}");
    // The bar rises off the ground plane, so its bottom wall faces −Z there.
    assert!(facing[0].distance(DVec3::Z) < 1e-12, "{facing:?}");
}

/// **An extrusion of no depth is no solid**, and says so by having no faces.
///
/// Six faces enclosing nothing would be a worse answer: there is nothing to
/// draw, nothing to pick and nothing to build on, and a body that admitted to
/// holding faces would have every reader of it handling a degenerate case.
#[test]
fn an_extrusion_of_no_depth_is_a_body_with_nothing_in_it() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
    let found = Arrangement::of(&sketch);
    let body = Extrusion::new(&found, 0, Plane::GROUND, 0.0, STEP).body();

    assert!(body.is_empty());
    assert_eq!(body.names().count(), 0);
    assert!(
        !body.holds(STEP.grew(Grown::Base)),
        "it holds a face it has not got"
    );
    assert_eq!(body.topology().faces().count(), 0);
    assert_eq!(volume(&body), 0.0);
}
