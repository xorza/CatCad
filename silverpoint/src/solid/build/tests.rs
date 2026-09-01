use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use crate::solid::build::builder::Extrusion;
use crate::solid::build::revolving::{MOST, Revolution};
use crate::solid::build::sector::Sector;
use crate::solid::geometry::curve::Curve;
use crate::solid::geometry::natural::Natural;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::mesh::Mesher;
use crate::solid::named::{Named, Step};
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};
use std::f64::consts::{FRAC_PI_2, PI, TAU};

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

    let up = Extrusion::new(&found, &[0], Plane::GROUND, 3.0, STEP).body();
    let faces: Vec<Named> = up.names().collect();
    assert_eq!(faces.len(), 6, "{faces:?}");
    assert_eq!(faces[0], STEP.grew(Grown::Base));
    assert_eq!(faces[1], STEP.grew(Grown::Far));
    assert!(
        (volume(&up) - 12.0).abs() < 1e-9,
        "it shut in {}",
        volume(&up)
    );

    let down = Extrusion::new(&found, &[0], Plane::GROUND, -3.0, STEP).body();
    assert!(
        (volume(&down) - 12.0).abs() < 1e-9,
        "grown against the normal it shut in {}, so it is inside out",
        volume(&down),
    );

    let raised = Plane {
        origin: DVec3::new(-7.0, 4.0, 11.0),
        ..Plane::FRONT
    };
    let elsewhere = Extrusion::new(&found, &[0], raised, 3.0, STEP).body();
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
    let body = Extrusion::new(&found, &[0], Plane::GROUND, 3.0, STEP).body();

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

    let body = Extrusion::new(&found, &[ring], Plane::GROUND, 5.0, STEP).body();

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
    let body = Extrusion::new(&found, &[0], Plane::GROUND, 4.0, STEP).body();

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
        let Surface::Natural(Natural::Cylinder(cylinder)) = face.surface else {
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
    let body = Extrusion::new(&found, &[0], Plane::GROUND, 2.0, STEP).body();

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
    let plain = Extrusion::new(&Arrangement::of(&sketch), &[0], Plane::GROUND, 2.0, STEP).body();

    let tip = sketch.add_point(DVec2::new(2.0, 2.0));
    sketch.add_segment(corners[0], tip);
    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 1, "the spur enclosed something");
    let spurred = Extrusion::new(&found, &[0], Plane::GROUND, 2.0, STEP).body();

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
        .map(|at| volume(&Extrusion::new(&found, &[at], Plane::GROUND, 3.0, STEP).body()))
        .sum();
    assert!(
        (together - 72.0).abs() < 1e-9,
        "the pieces cover {together}"
    );

    let bar = (0..2)
        .map(|at| Extrusion::new(&found, &[at], Plane::GROUND, 3.0, STEP).body())
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
    let body = Extrusion::new(&found, &[0], Plane::GROUND, 0.0, STEP).body();

    assert!(body.is_empty());
    assert_eq!(body.names().count(), 0);
    assert!(
        !body.holds(STEP.grew(Grown::Base)),
        "it holds a face it has not got"
    );
    assert_eq!(body.topology().faces().count(), 0);
    assert_eq!(volume(&body), 0.0);
}

/// **A drawing whose curves meet where they were drawn raises vertices that
/// stand for nothing at all.**
///
/// The ceiling `.notes/KERNEL.md` §4.1 puts on a body's exactness is the
/// drawing's own fold, and the fold is per corner rather than a blanket over
/// the body: a corner two curves handed in bit for bit swallowed nothing, so
/// it records nothing and a vertex raised there is exact. Which is every
/// corner of an ordinary drawing — this used to be a nanometre on every vertex
/// of every body, whether anything had folded or not.
///
/// **Five drawings, because the corners reach the builder several ways.** A
/// square's come from the fold. A circle's come from the cutting, which mints
/// one to start a loop nothing crossed, and from the halving that keeps a face
/// from wrapping — both worked out exactly and neither passing through the
/// fold at all. A square with a circle inside it has both at once.
///
/// **And three whose corners nobody drew.** A run straight through a square
/// crosses two of its sides, and a chord across a circle comes off a quadratic
/// — corners the arrangement worked out rather than read off the drawing, and
/// the ones a body's exactness used to be capped by. They stand for nothing
/// because `math::intersect` decides whether a crossing lands on what made it
/// rather than reading it off a parameter.
///
/// The third is the same run a hundred million units out, where a coordinate is
/// written down to sixty nanometres of its own size. It stands for nothing too,
/// and it is the one that says the *check* allows for a rounding being a
/// proportion rather than a fixed width.
///
/// The edges are asked too, and for a different reason: an edge is the true
/// intersection of the two surfaces either side of it — §4.3's own definition
/// — and those are exact whatever corner they were placed through.
#[test]
fn a_drawing_that_folded_nothing_raises_a_body_that_stands_for_nothing() {
    let square = |sketch: &mut Sketch, from: DVec2, side: f64| {
        let corners: Vec<_> = [(0.0, 0.0), (side, 0.0), (side, side), (0.0, side)]
            .map(|(u, v)| sketch.add_point(from + DVec2::new(u, v)))
            .into();
        for at in 0..corners.len() {
            sketch.add_segment(corners[at], corners[(at + 1) % corners.len()]);
        }
    };
    let mut drawings = Vec::new();

    let mut sketch = Sketch::default();
    square(&mut sketch, DVec2::ZERO, 4.0);
    drawings.push(("a square", sketch));

    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(3.0, 1.0));
    sketch.add_circle(middle, 2.0);
    drawings.push(("a circle", sketch));

    let mut sketch = Sketch::default();
    square(&mut sketch, DVec2::ZERO, 6.0);
    let middle = sketch.add_point(DVec2::new(3.0, 3.0));
    sketch.add_circle(middle, 1.0);
    drawings.push(("a square about a circle", sketch));

    // A run straight through the square, crossing two of its sides where
    // nobody drew a point: two corners the arrangement worked out rather than
    // read off the drawing.
    let mut sketch = Sketch::default();
    square(&mut sketch, DVec2::ZERO, 4.0);
    let across = [(-1.0, 2.0), (5.0, 2.0)].map(|(x, y)| sketch.add_point(DVec2::new(x, y)));
    sketch.add_segment(across[0], across[1]);
    drawings.push(("a square run through", sketch));

    // And a chord across a circle, where the two corners come off a quadratic
    // rather than off a pair of straight lines.
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(2.0, 2.0));
    sketch.add_circle(middle, 1.0);
    let chord = [(0.0, 2.0), (4.0, 2.0)].map(|(x, y)| sketch.add_point(DVec2::new(x, y)));
    sketch.add_segment(chord[0], chord[1]);
    drawings.push(("a circle cut by a chord", sketch));

    // The same run through a square, out where a product of two coordinates
    // needs more bits than a float holds and one place in the last is worth
    // sixty nanometres. Every crossing here is worked out through arithmetic
    // that rounds, and the *check* has to allow for a rounding being a
    // proportion — see [`slack`](crate::number::predicate::slack).
    let far = 100000001.0;
    let mut sketch = Sketch::default();
    square(&mut sketch, DVec2::ZERO, 4.0 * far);
    let across = [(-far, 2.0 * far), (5.0 * far, 2.0 * far)]
        .map(|(x, y)| sketch.add_point(DVec2::new(x, y)));
    sketch.add_segment(across[0], across[1]);
    drawings.push(("a square run through, far from the origin", sketch));

    for (drawn, sketch) in drawings {
        let found = Arrangement::of(&sketch);
        assert!(
            found.reached().iter().all(|&reach| reach == 0.0),
            "{drawn} folded something: {:?}",
            found.reached(),
        );
        let body = Extrusion::new(&found, &[0], Plane::GROUND, 4.0, STEP).body();
        body.check();
        let topology = body.topology();
        assert!(
            topology.vertices().count() > 0,
            "{drawn} raised no vertices to ask about",
        );
        for (at, vertex) in topology.vertices() {
            assert_eq!(
                vertex.tolerance, 0.0,
                "{drawn}: vertex {at:?} stands for something"
            );
        }
        for (at, edge) in topology.edges() {
            assert_eq!(
                edge.tolerance, 0.0,
                "{drawn}: edge {at:?} stands for something"
            );
        }
    }
}

/// **A circle spun about a line beside it is the ring it traces**, which is the
/// first body anywhere here to stand on a surface of the fitted tier without
/// being written down by hand.
///
/// A circle of one about a centre three out, spun about the world's `+Y`. Every
/// figure is the ring's own: `2π²·major·minor²` by Pappus, genus one, and a
/// count of faces, edges and vertices that is forced rather than chosen — the
/// circle wraps, so the drawing hands over two arcs; each is spun a whole turn
/// and so is cut in three again; and `6 − 12 + 6` is nought, which is
/// `2(1 − 1)`.
///
/// **And it is not exact**, a torus being the fitted tier's own surface. That
/// is the whole point of the feature: nothing else in the tree builds one.
#[test]
fn a_circle_spun_about_a_line_beside_it_is_the_ring_it_traces() {
    let (major, minor) = (3.0_f64, 1.0_f64);
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(major, 0.0));
    let drawn = sketch.add_circle(middle, minor);
    let found = Arrangement::of(&sketch);
    let body = Revolution::new(
        &found,
        &[0],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        STEP,
    )
    .body();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 1, "a ring is a ring: {reckoning:?}");
    assert_eq!(
        body.topology().faces().count(),
        2 * MOST,
        "the wall was not cut in three"
    );
    assert!(!body.exact(), "a body standing on a torus is not exact");

    // One name over the four, a wall being named by the curve it came off
    // rather than by how the kernel had to cut it.
    let wall = STEP.grew(Grown::Side(Bound {
        of: Entity::Circle(drawn),
        along: true,
    }));
    let named: Vec<Named> = body.names().collect();
    assert_eq!(named, [wall], "{named:?}");

    // **Coarser than [`FINE`]**, which is what a doubly-round surface costs: a
    // torus is cut in both of its angles at once, so the cells go as the
    // *square* of what a cylinder's do and a sagitta fine enough for a flat
    // answer is a million of them. A thousandth is three parts in ten thousand
    // of the answer here, which is tighter than the figure needs.
    let want = 2.0 * PI * PI * major * minor * minor;
    let off = (Mesher::default().volume(&body, 1e-3) - want).abs();
    // The whole surface is chorded: `4π²·major·minor` of it. What a chord cuts
    // off goes as two thirds of the sagitta times the area it spans.
    let slack = (2.0 / 3.0) * 1e-3 * 4.0 * PI * PI * major * minor;
    assert!(off < slack, "{off} off {want}");
}

/// **A trapezoid spun sweeps all four of the exact tier's own surfaces**, and
/// the body it makes says it is exact.
///
/// A side parallel to the line sweeps a cylinder, one square across it an
/// annulus of a plane, and one that leans a cone. `(1,0) → (3,0) → (2,2) →
/// (1,2)` has one of each and closes.
///
/// **Held by Pappus, which is the whole check a revolve wants.** A plane figure
/// spun a whole turn shuts in `2π` times its first moment about the line — its
/// area times how far out its own middle stands. The trapezoid's shoelace is
/// six, so its area is three, and its middle stands `32/18` out, which makes
/// the answer `32π/3`. Read the other way for the same figure: at height `y`
/// the solid is the annulus from one out to `3 − y/2`, and `π∫₀²((3−y/2)² − 1)`
/// is `32π/3` again.
#[test]
fn a_trapezoid_spun_sweeps_a_cylinder_a_cone_and_two_annuli() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(1.0, 0.0), (3.0, 0.0), (2.0, 2.0), (1.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    let body = Revolution::new(
        &found,
        &[0],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        STEP,
    )
    .body();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 1, "a spun ring is a ring: {reckoning:?}");
    // Four walls: a cylinder and a cone cut in three apiece, and two annuli
    // that are one face each — a plane's parameters do not wrap, so §4.4 has
    // nothing to split.
    assert_eq!(
        body.topology().faces().count(),
        2 * MOST + 2,
        "the annuli were cut into sectors"
    );
    assert!(body.exact(), "a cylinder, a cone and two planes are exact");
    assert_eq!(body.strays(), 0.0, "an exact body strays nowhere");

    // Four surfaces and one of each kind, asked of the body rather than of the
    // arithmetic that made them.
    let (mut cylinders, mut cones, mut planes) = (0, 0, 0);
    for (_, face) in body.topology().faces() {
        match face.surface {
            Surface::Natural(Natural::Cylinder(_)) => cylinders += 1,
            Surface::Natural(Natural::Cone(_)) => cones += 1,
            Surface::Natural(Natural::Plane(_)) => planes += 1,
            other => panic!("a trapezoid swept {other:?}"),
        }
    }
    assert_eq!(
        [cylinders, cones, planes],
        [MOST, MOST, 2],
        "the wrong surfaces"
    );

    let want = 32.0 * PI / 3.0;
    let off = (volume(&body) - want).abs();
    // Only the cylinder and the cone are chorded, the annuli being flat: the
    // cylinder is `2π·1` round by two tall and the cone `2π·2.5` round by its
    // own slant. What a chord cuts off goes as two thirds of the sagitta times
    // the area it spans.
    let slack = (2.0 / 3.0) * FINE * TAU * (2.0 + 2.5 * 5.0_f64.sqrt());
    assert!(off < slack, "{off} off {want}");
}

/// **An arc about a centre on the line sweeps a sphere**, which is the fifth
/// surface and the one a straight run cannot give.
///
/// A circle of three about the origin, cut by the chord at `x = 0.4`. The piece
/// beyond that chord is a circular segment whose arc is centred on the line and
/// stands clear of it — so spun it is a zone of a sphere, closed by the
/// cylinder the chord sweeps.
///
/// **Pappus again.** A segment cut from a disc of `radius` at `off` from its
/// middle has `(2/3)(radius² − off²)^{3/2}` of first moment about that middle —
/// the figure the ring tests are measured by — and here that middle is on the
/// line, so the moment is the whole of it.
#[test]
fn an_arc_about_a_centre_on_the_line_sweeps_a_sphere() {
    let (radius, chord) = (3.0_f64, 0.4_f64);
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, radius);
    let low = sketch.add_point(DVec2::new(chord, -radius - 0.5));
    let high = sketch.add_point(DVec2::new(chord, radius + 0.5));
    sketch.add_segment(low, high);
    let found = Arrangement::of(&sketch);

    let mut spun = None;
    for at in 0..found.faces().len() {
        let body = Revolution::new(
            &found,
            &[at],
            Plane::FRONT,
            DVec2::ZERO,
            DVec2::Y,
            Sector::WHOLE,
            STEP,
        )
        .body();
        // Every other face of the drawing straddles the line, and a revolve of
        // one is refused — see [`Revolving::raise`].
        if !body.is_empty() {
            assert!(spun.is_none(), "two faces of the drawing were spun");
            spun = Some(body);
        }
    }
    let body = spun.expect("the segment beyond the chord is spun");
    let reckoning = body.reckoning();
    assert_eq!(
        reckoning.genus, 1,
        "a spun segment is a ring: {reckoning:?}"
    );
    assert_eq!(
        body.topology().faces().count(),
        2 * MOST,
        "two walls, cut in three"
    );
    assert!(body.exact(), "a sphere and a cylinder are exact");
    let mut spheres = 0;
    for (_, face) in body.topology().faces() {
        if let Surface::Natural(Natural::Sphere(ball)) = face.surface {
            assert!((ball.radius - radius).abs() < 1e-12, "{ball:?}");
            assert!(
                ball.axis.origin.length() < 1e-12,
                "{ball:?} is off the line"
            );
            spheres += 1;
        }
    }
    assert_eq!(spheres, MOST, "the arc swept no sphere");

    // Coarser than [`FINE`] for the reason the ring is: a sphere is cut in
    // both of its angles at once, so the cells go as the square of what a
    // cylinder's do.
    let want = TAU * (2.0 / 3.0) * (radius * radius - chord * chord).powf(1.5);
    let off = (Mesher::default().volume(&body, 1e-4) - want).abs();
    // The sphere's zone is `2π·radius` by its own height and the cylinder
    // `2π·chord` by the same. What a chord cuts off goes as two thirds of the
    // sagitta times the area it spans.
    let tall = 2.0 * (radius * radius - chord * chord).sqrt();
    let slack = (2.0 / 3.0) * 1e-4 * TAU * (radius + chord) * tall;
    assert!(off < slack, "{off} off {want}");
}

/// **A profile spun about one of its own sides closes at a pole**, which is the
/// commonest revolve there is and the one a corner standing on the line makes.
///
/// A right triangle `(0,0) → (1,0) → (0,2)` about the drawing's own `y`: the
/// side up the axis sweeps nothing at all, the base sweeps a disc closing to the
/// point at the origin, and the slanted side a cone closing at `(0,2)`. So both
/// ends of the solid are poles, and the whole of it is two walls.
///
/// **A pole is one vertex, not one per part.** A corner on the line sweeps a
/// *point*, and a face reaching it is a side of the region collapsed to that
/// point — the same shape the hand-built ball has at each of its own two. So
/// the loop of such a face is three edges where every other is four.
///
/// **A cone by Pappus, and by the schoolbook.** The triangle's area is one and
/// its middle stands a third out, so `2π·(1/3)·1` is `2π/3` — which is
/// `πr²h/3` for a radius of one and a height of two.
#[test]
fn a_profile_spun_about_its_own_side_closes_at_a_pole() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (1.0, 0.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    let body = Revolution::new(
        &found,
        &[0],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        STEP,
    )
    .body();

    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 0, "a cone is a ball: {reckoning:?}");
    assert!(body.exact(), "a cone and a plane are exact");
    // The side lying *on* the line sweeps nothing, so two walls rather than
    // three — the cone cut in three, no face being allowed to wrap, and the
    // base one face, a plane's parameters not wrapping at all.
    assert_eq!(
        body.topology().faces().count(),
        MOST + 1,
        "the third side raised a wall",
    );
    // **A pole is one vertex, and only where a seam ends at it.** A corner on
    // the line sweeps a point rather than a circle, so nothing but a seam ever
    // reaches one — and the base is one face with no seams, which leaves its
    // own centre off the body altogether. The apex the cone's three seams meet
    // at is the one pole left, and the rim carries a vertex per part.
    assert_eq!(
        body.topology().vertices().count(),
        1 + MOST,
        "a pole raised more than one vertex",
    );

    let want = 2.0 * PI / 3.0;
    let off = (volume(&body) - want).abs();
    // Only the cone is chorded, the disc being flat: `2π·1` round by its own
    // slant of `√5`. What a chord cuts off goes as two thirds of the sagitta
    // times the area it spans.
    let slack = (2.0 / 3.0) * FINE * TAU * 5.0_f64.sqrt();
    assert!(off < slack, "{off} off {want}");
}

/// **A hole of the profile sweeps a cavity, not a hole through it.**
///
/// An extrusion's two caps join the wall of a hole to the wall outside it, so
/// one shell goes round both. A whole turn raises no cap, so what a hub inside
/// a region sweeps is a shell of its own with the solid all around it — and a
/// body whose lump did not say so would list faces its own outer shell cannot
/// reach.
///
/// A ring of two about `(5, 0)` with a hole of one in it, spun about the
/// drawing's `y`. By Pappus the solid is `2π·5·(π·2² − π·1²)`, the two circles
/// sharing a middle five out.
#[test]
fn a_hole_in_the_profile_sweeps_a_cavity_of_its_own() {
    let (out, at, hole) = (5.0_f64, 2.0_f64, 1.0_f64);
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(out, 0.0));
    sketch.add_circle(middle, at);
    sketch.add_circle(middle, hole);
    let found = Arrangement::of(&sketch);

    let mut ringed = None;
    for face in 0..found.faces().len() {
        let body = Revolution::new(
            &found,
            &[face],
            Plane::FRONT,
            DVec2::ZERO,
            DVec2::Y,
            Sector::WHOLE,
            STEP,
        )
        .body();
        let (_, lump) = body
            .topology()
            .lumps()
            .next()
            .expect("a spun region encloses a lump");
        if !lump.voids.is_empty() {
            assert!(ringed.is_none(), "two faces of the drawing swept a cavity");
            ringed = Some(body);
        }
    }
    let body = ringed.expect("the ring between the two circles swept no cavity");
    let (_, lump) = body.topology().lumps().next().expect("the lump");
    assert_eq!(lump.voids.len(), 1, "the hub swept more than one cavity");

    let want = TAU * out * PI * (at * at - hole * hole);
    let off = (Mesher::default().volume(&body, 1e-3) - want).abs();
    // Both tubes are chorded, and each is `2π·out` round by `2π·minor`. What a
    // chord cuts off goes as two thirds of the sagitta times the area it spans.
    let slack = (2.0 / 3.0) * 1e-3 * TAU * TAU * out * (at + hole);
    assert!(off < slack, "{off} off {want}");
}

/// **A profile of two regions raises one body of two lumps**, carried and spun
/// alike.
///
/// Two disjoint squares of a drawing, both swept by one sweep. They are faces
/// of one arrangement, so they cannot overlap — which is what lets each raise
/// lumps of its own with no boolean between them.
///
/// **And the two caps are one face.** Both answer to [`Grown::Base`], a name
/// saying which step grew it and what of that step it is, so §5's rule makes
/// them one face of the feature — the same answer a pocket cut across a cap
/// gives. The walls stay apart, a wall being named by the curve it came off.
///
/// Carried: `2·2·3` twice is twenty-four, over twelve faces under ten names.
/// Spun about the drawing's `y`: Pappus twice, `2π·2·4` and `2π·6·4`, which is
/// `64π` — and each square's four sides sweep two cylinders and two annuli.
#[test]
fn a_profile_of_two_regions_raises_one_body_of_two_lumps() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(1.0, 0.0), (3.0, 0.0), (3.0, 2.0), (1.0, 2.0)]);
    sketch.outline(&[(5.0, 0.0), (7.0, 0.0), (7.0, 2.0), (5.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 2, "two squares, two regions");

    let carried = Extrusion::new(&found, &[0, 1], Plane::GROUND, 3.0, STEP).body();
    assert_eq!(
        carried.topology().faces().count(),
        12,
        "four caps and eight walls",
    );
    assert_eq!(
        carried.names().count(),
        10,
        "the two bases are one name, and so are the two far ends",
    );
    assert_eq!(
        carried.patches(STEP.grew(Grown::Base)).count(),
        2,
        "one base over two regions",
    );
    assert_eq!(
        carried.topology().lumps().count(),
        2,
        "two regions, two lumps",
    );
    // Of the first lump's own shell, which is what a reckoning reads: `8 − 12 +
    // 6 − 0` is `2(1 − 0)`, a box being a box whatever stands beside it.
    let reckoning = carried.reckoning();
    assert_eq!(reckoning.characteristic, 2, "{reckoning:?}");
    assert_eq!(reckoning.genus, 0, "a box: {reckoning:?}");
    assert!(
        (volume(&carried) - 24.0).abs() < 1e-9,
        "it shut in {}",
        volume(&carried),
    );

    let spun = Revolution::new(
        &found,
        &[0, 1],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector::WHOLE,
        STEP,
    )
    .body();
    // Eight walls, four of them square across the line: those sweep annuli,
    // which are one face apiece where a cylinder is cut in three.
    assert_eq!(
        spun.topology().faces().count(),
        2 * (2 * MOST + 2),
        "the annuli were cut into sectors",
    );
    assert_eq!(spun.names().count(), 8, "one name per side of each square");
    assert_eq!(spun.topology().lumps().count(), 2, "two regions, two lumps");
    assert!(spun.exact(), "cylinders and planes are exact");

    let want = TAU * 2.0 * 4.0 + TAU * 6.0 * 4.0;
    let off = (volume(&spun) - want).abs();
    // Only the four cylinders are chorded, the annuli being flat. What a chord
    // cuts off goes as two thirds of the sagitta times the area it spans, and
    // each is `2π·radius` round by the square's own height of two.
    let slack = (2.0 / 3.0) * FINE * TAU * (1.0 + 3.0 + 5.0 + 7.0) * 2.0;
    assert!(off < slack, "{off} off {want}");
}

/// **A profile whose regions stand on both sides of the line sweeps nothing.**
///
/// Two regions of one drawing cannot overlap, and on one side of the line the
/// map to a radius and a height keeps them apart. A region mirrored across the
/// line sweeps the very same space, so the two lumps would share it — which is
/// not a solid, and is what a region *straddling* the line is already refused
/// for.
///
/// Each on its own is a ring, which is what says the pair is refused for
/// standing on two sides rather than for anything either one of them is.
#[test]
fn a_profile_straddling_the_line_in_two_pieces_sweeps_nothing() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(1.0, 0.0), (3.0, 0.0), (3.0, 2.0), (1.0, 2.0)]);
    sketch.outline(&[(-3.0, 0.0), (-1.0, 0.0), (-1.0, 2.0), (-3.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 2, "two squares, two regions");

    let spun = |at: &[usize]| {
        Revolution::new(
            &found,
            at,
            Plane::FRONT,
            DVec2::ZERO,
            DVec2::Y,
            Sector::WHOLE,
            STEP,
        )
        .body()
    };
    for at in 0..2 {
        assert!(
            !spun(&[at]).is_empty(),
            "the square at {at} does not sweep a ring on its own",
        );
    }
    assert!(
        spun(&[0, 1]).is_empty(),
        "two regions either side of the line swept two lumps of one space",
    );
}

/// **A partial turn is capped at both ends**, and the caps are what tell it
/// from a whole one.
///
/// A circle of one, three out, spun a quarter turn about the drawing's `y`. Two
/// arcs sweep a quarter of a torus each; the two ends are the circle itself
/// lying in the half-plane the spin carried it to.
///
/// **Genus nought where a whole turn gives one.** A ring is a handle; a quarter
/// of one is a bent rod, which is a ball. `4 − 6 + 4` is two, and the reckoning
/// is what says the caps closed it rather than leaving it open.
///
/// **And one face per wall, not three.** Every part spans at most a third of a
/// turn — see [`MOST`] — and a quarter is under a third, so nothing is cut. The
/// boundary is asserted beside it: a sweep either side of a third of a turn
/// gives two parts and one, which is what says the count follows the sweep.
///
/// Pappus holds for a part of a turn as it does for the whole: the figure shuts
/// in `|sweep|` times its first moment about the line.
#[test]
fn a_partial_turn_is_capped_at_both_ends() {
    let (major, minor) = (3.0_f64, 1.0_f64);
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(major, 0.0));
    sketch.add_circle(middle, minor);
    let found = Arrangement::of(&sketch);
    let spun = |sweep: f64| {
        Revolution::new(
            &found,
            &[0],
            Plane::FRONT,
            DVec2::ZERO,
            DVec2::Y,
            Sector { from: 0.0, sweep },
            STEP,
        )
        .body()
    };

    let quarter = spun(FRAC_PI_2);
    let reckoning = quarter.reckoning();
    assert_eq!(reckoning.genus, 0, "a bent rod is a ball: {reckoning:?}");
    assert_eq!(
        quarter.topology().faces().count(),
        4,
        "two walls and two caps",
    );
    assert_eq!(
        quarter.topology().lumps().next().expect("one lump").1.voids,
        0..0,
        "a capped turn has no cavity",
    );
    // The two caps are planes, which the two torus walls are not.
    let planes = quarter
        .topology()
        .faces()
        .filter(|(_, face)| matches!(face.surface, Surface::Natural(Natural::Plane(_))))
        .count();
    assert_eq!(planes, 2, "the two ends are not the two planes");

    let want = FRAC_PI_2 * major * PI * minor * minor;
    let off = (Mesher::default().volume(&quarter, 1e-3) - want).abs();
    // The same slack the whole ring is measured with, over a quarter of it.
    let slack = (2.0 / 3.0) * 1e-3 * FRAC_PI_2 * major * TAU * minor;
    assert!(off < slack, "{off} off {want}");

    // **The count of parts follows the sweep**, and a third of a turn is where
    // it steps. One wall each way, so the face counts are twice the parts plus
    // the two caps.
    let third = TAU / 3.0;
    let under = spun(third - 1e-6).topology().faces().count();
    let over = spun(third + 1e-6).topology().faces().count();
    assert_eq!(under, 2 + 2, "a third of a turn was cut in two");
    assert_eq!(
        over,
        4 + 2,
        "just over a third of a turn was not cut in two"
    );
    assert_ne!(under, over, "the sweep does not decide the count");

    // **A whole turn is a whole turn to the room the ladder gives it**, and
    // the same room either side. A sweep worked out rather than typed lands a
    // rounding off `TAU`, and one under read as a partial turn would cap it at
    // two ends standing in the very same place. Held against a sweep well
    // outside that room, which is the capped turn it really is: the caps are
    // the two faces, and the handle they shut is what the genus reads.
    let turned = |sweep: f64| {
        let body = spun(sweep);
        (body.topology().faces().count(), body.reckoning().genus)
    };
    let whole = turned(TAU);
    assert_eq!(whole, (2 * MOST, 1), "a ring is a handle cut into parts");
    assert_eq!(
        turned(TAU - 1e-12),
        whole,
        "a rounding under a turn capped it"
    );
    assert_eq!(
        turned(TAU + 1e-12),
        whole,
        "a rounding over a turn was refused"
    );
    assert_eq!(
        turned(TAU - 1e-6),
        (2 * MOST + 2, 0),
        "a sweep outside the room is the partial turn it is",
    );
}

/// **A partial turn keeps the pole, and a hole in it is a hole again.**
///
/// The two things a cap changes, asked of the two profiles that show them.
///
/// A right triangle spun a quarter turn about its own side is a wedge of the
/// cone the whole turn gives — one pole at each end still, and the two caps are
/// the triangle itself. By Pappus `(π/2)·(1/3)·1` is `π/6`, which is a quarter
/// of the `2π/3` the whole turn shuts in.
///
/// And a ring of two about `(5, 0)` with a hole of one, spun part way: the caps
/// join the hole's wall to the wall outside it, so there is **one shell and no
/// cavity** — which is the answer an extrusion gives and the opposite of the
/// whole turn's.
#[test]
fn a_partial_turn_keeps_a_pole_and_opens_a_cavity_into_a_hole() {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (1.0, 0.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    let wedge = Revolution::new(
        &found,
        &[0],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector {
            from: 0.0,
            sweep: FRAC_PI_2,
        },
        STEP,
    )
    .body();
    assert_eq!(wedge.reckoning().genus, 0, "a wedge is a ball");
    // The side on the line sweeps nothing, so two walls of one part each — and
    // the two caps, which are the triangle itself.
    assert_eq!(
        wedge.topology().faces().count(),
        2 + 2,
        "the third side raised a wall, or an end raised no cap",
    );
    // **Two poles still.** The rim carries one vertex per seam and a corner on
    // the line carries one however the turn is cut.
    assert_eq!(
        wedge.topology().vertices().count(),
        2 + 2,
        "a pole raised more than one vertex",
    );
    let want = FRAC_PI_2 * (1.0 / 3.0) * 1.0;
    let off = (volume(&wedge) - want).abs();
    let slack = (2.0 / 3.0) * FINE * FRAC_PI_2 * 5.0_f64.sqrt();
    assert!(off < slack, "{off} off {want}");

    let (out, at, hole) = (5.0_f64, 2.0_f64, 1.0_f64);
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(out, 0.0));
    sketch.add_circle(middle, at);
    sketch.add_circle(middle, hole);
    let found = Arrangement::of(&sketch);
    let ringed = found
        .faces()
        .iter()
        .position(|face| face.holes() == 1)
        .expect("the hub is a hole of the ring");
    let sweep = FRAC_PI_2;
    let body = Revolution::new(
        &found,
        &[ringed],
        Plane::FRONT,
        DVec2::ZERO,
        DVec2::Y,
        Sector { from: 0.0, sweep },
        STEP,
    )
    .body();
    let (_, lump) = body.topology().lumps().next().expect("one lump");
    assert!(
        lump.voids.is_empty(),
        "a capped turn left the hub a cavity of its own",
    );

    let want = sweep * out * PI * (at * at - hole * hole);
    let off = (Mesher::default().volume(&body, 1e-3) - want).abs();
    // Both tubes are chorded, each `sweep·out` round by `2π·minor`.
    let slack = (2.0 / 3.0) * 1e-3 * sweep * out * TAU * (at + hole);
    assert!(off < slack, "{off} off {want}");
}

/// **A semicircle spun about its own diameter is a ball**, which is the
/// commonest revolve there is and the one an arc reaching the line makes.
///
/// The arc runs from the line round to the line again. It lies on neither —
/// an arc bulges off its own chord — so what it sweeps is a sphere, and its two
/// ends are the poles. The straight side closing the profile *does* lie on the
/// line and sweeps nothing, so the whole ball is one wall.
///
/// **Three faces, three edges, two vertices**, and `2 − 3 + 3` is two: the wall
/// is cut in three, no face being allowed to wrap, and the three seams are
/// meridians running pole to pole. Nothing else bounds it — a corner on the
/// line sweeps a point rather than a circle, so there is no equator.
///
/// `4πr³/3` by the schoolbook, and by Pappus: a half-disc of area `πr²/2` whose
/// middle stands `4r/3π` out gives `2π · (4r/3π) · (πr²/2)`.
#[test]
fn a_semicircle_spun_about_its_own_diameter_is_a_ball() {
    let radius = 1.0_f64;
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, radius);
    // The diameter, drawn past the rim either way so the arrangement cuts both
    // curves rather than leaving the line hanging in the disc.
    let low = sketch.add_point(DVec2::new(0.0, -radius - 1.0));
    let high = sketch.add_point(DVec2::new(0.0, radius + 1.0));
    sketch.add_segment(low, high);
    let found = Arrangement::of(&sketch);

    // Either half is a ball, the two standing on opposite sides of the line —
    // so the first that sweeps anything is the one under test.
    let body = (0..found.faces().len())
        .map(|at| {
            Revolution::new(
                &found,
                &[at],
                Plane::FRONT,
                DVec2::ZERO,
                DVec2::Y,
                Sector::WHOLE,
                STEP,
            )
            .body()
        })
        .find(|body| !body.is_empty())
        .expect("no half of the disc swept a ball");

    let reckoning = body.reckoning();
    assert_eq!(reckoning.genus, 0, "a ball is a ball: {reckoning:?}");
    let topology = body.topology();
    assert_eq!(topology.faces().count(), MOST, "one wall, cut in three");
    assert_eq!(
        topology.edges().count(),
        MOST,
        "the seams are the only edges"
    );
    assert_eq!(topology.vertices().count(), 2, "a ball has two poles");
    assert!(body.exact(), "a sphere is exact");
    for (_, face) in topology.faces() {
        assert!(
            matches!(face.surface, Surface::Natural(Natural::Sphere(_))),
            "the arc swept {:?}",
            face.surface,
        );
    }

    let want = 4.0 * PI * radius * radius * radius / 3.0;
    // **Coarser than [`FINE`]**, on the terms the sphere zone above states: a
    // sphere is cut in both of its angles at once, so the cells go as the
    // *square* of what a cylinder's do and a sagitta fine enough for a flat
    // answer is a million of them.
    let off = (Mesher::default().volume(&body, 1e-4) - want).abs();
    let slack = (2.0 / 3.0) * 1e-4 * TAU * radius * (PI * radius);
    assert!(off < slack, "{off} off {want}");
}
