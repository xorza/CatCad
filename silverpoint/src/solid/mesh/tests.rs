use crate::math::arc;
use crate::math::plane::Plane;
use crate::number::predicate;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use crate::solid::boolean::{Boolean, Operation};
use crate::solid::build::extrusion::Extrusion;
use crate::solid::geometry::axis::Axis;
use crate::solid::geometry::cylinder::Cylinder;
use crate::solid::geometry::surface::Surface;
use crate::solid::grown::Grown;
use crate::solid::mesh::lattice::Lattice;
use crate::solid::mesh::{Mesher, Patch};
use crate::solid::named::Step;
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};
use std::f64::consts::{PI, TAU};

/// The step every body below is grown by.
///
/// Which one it is says nothing here: what a step tells apart is one feature's
/// faces from another's, and every test in this file meshes one body.
const STEP: Step = Step(0);

/// A two-by-two block three deep, standing with one corner on the origin.
fn block() -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, 0, Plane::GROUND, 3.0, STEP).body()
}

/// **Every corner faces out of the solid**, in position and in winding alike.
///
/// Two readings of the same claim, which is why they are asserted together. The
/// stored normal has to point away from the middle of the block; and the
/// triangle it belongs to has to be wound so that its own cross product points
/// the same way. A face whose material side was recorded backwards passes
/// neither, and one whose triangles were flipped without its normals passes
/// only the first.
#[test]
fn every_corner_and_every_triangle_faces_out_of_the_solid() {
    let body = block();
    let middle = DVec3::new(1.0, 1.5, -1.0);
    let mut mesher = Mesher::default();
    let mut patch = Patch::default();

    for named in body.names() {
        mesher.cut(&body, named, 1e-3, &mut patch);
        assert_eq!(patch.corners.len(), patch.normals.len());
        assert!(!patch.triangles.is_empty(), "{named:?} cut to nothing");

        for (&corner, &normal) in patch.corners.iter().zip(&patch.normals) {
            assert!(
                (normal.length() - 1.0).abs() < 1e-12,
                "{named:?} is not unit"
            );
            assert!(
                (corner - middle).dot(normal) > 0.0,
                "{named:?} faces inward at {corner:?}",
            );
        }
        for &[a, b, c] in &patch.triangles {
            let corner = |at: u32| patch.corners[at as usize];
            let wound = (corner(b) - corner(a)).cross(corner(c) - corner(a));
            assert!(
                wound.dot(patch.normals[a as usize]) > 0.0,
                "{named:?} is wound against its own normals",
            );
        }
    }
}

/// **The caps and the walls meet exactly**, corner for corner.
///
/// The one failure a picture shows as a hairline of background between two
/// faces that are meant to touch, and the reason a walk reads the stored vertex
/// at either end of an edge rather than evaluating the curve there. Asserted on
/// a cylinder, where the two faces invert *different* surfaces to reach the
/// same corners and so have every chance to disagree.
#[test]
fn a_wall_lands_on_the_cap_it_was_raised_from() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(3.0, 1.0));
    let ring = sketch.add_circle(middle, 2.0);
    let found = Arrangement::of(&sketch);
    let body = Extrusion::new(&found, 0, Plane::GROUND, 4.0, STEP).body();

    let mut mesher = Mesher::default();
    let (mut base, mut wall) = (Patch::default(), Patch::default());
    mesher.cut(&body, STEP.grew(Grown::Base), 1e-2, &mut base);
    mesher.cut(
        &body,
        STEP.grew(Grown::Side(Bound {
            of: Entity::Circle(ring),
            along: true,
        })),
        1e-2,
        &mut wall,
    );

    // Every corner of the wall that lies in the base plane is a corner the cap
    // has too, bit for bit: both came off the same stored vertex or the same
    // evaluation of the same curve.
    let ground = Plane::GROUND.normal();
    let footed: Vec<DVec3> = wall
        .corners
        .iter()
        .copied()
        .filter(|&corner| corner.dot(ground).abs() < 1e-12)
        .collect();
    assert!(!footed.is_empty(), "the wall never reached the base");
    for corner in footed {
        assert!(
            base.corners.contains(&corner),
            "the wall stands at {corner:?} and the cap does not",
        );
    }
}

/// **How finely a body is cut is the caller's, and nothing about it reaches the
/// model.**
///
/// A coarser sagitta gives fewer triangles and undershoots the true volume by
/// more, because a flattened circle is inscribed in the real one. Both readings
/// come off the same body — the surfaces never changed — which is what says the
/// sagitta is a way of *looking* rather than a property of the solid.
#[test]
fn a_finer_sagitta_cuts_more_finely_and_reads_nearer_the_truth() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 2.0);
    let found = Arrangement::of(&sketch);
    let body = Extrusion::new(&found, 0, Plane::GROUND, 4.0, STEP).body();

    let true_volume = PI * 4.0 * 4.0;
    let mut mesher = Mesher::default();
    let mut patch = Patch::default();
    let mut last = (0usize, f64::INFINITY);
    for sagitta in [0.5, 0.05, 5e-3, 5e-5] {
        mesher.cut(&body, STEP.grew(Grown::Base), sagitta, &mut patch);
        let cut = patch.triangles.len();
        let read = mesher.volume(&body, sagitta);
        let off = true_volume - read;

        assert!(off > 0.0, "a flattened circle read over the true {read}");
        assert!(cut > last.0, "{sagitta} cut no more finely than the last");
        assert!(off < last.1, "{sagitta} read no nearer than the last");
        // **Bounded by what was asked for**, rather than merely improving: the
        // error falls with the sagitta, so a triangulation that fanned across
        // the solid instead of following it would fail here however fine it was
        // cut. A hundredfold is the shape's own constant at this size — what
        // the assertion is about is that the two move together at all.
        assert!(
            off < 100.0 * sagitta,
            "{sagitta} left {off} of the volume out"
        );
        last = (cut, off);
    }
}

/// A name no face of the body carries cuts to nothing, rather than to whatever
/// happened to be in the buffer.
#[test]
fn a_name_the_body_does_not_hold_cuts_to_nothing() {
    let body = block();
    let mut mesher = Mesher::default();
    let mut patch = Patch::default();

    mesher.cut(&body, STEP.grew(Grown::Base), 1e-3, &mut patch);
    assert!(!patch.corners.is_empty());

    // A circle of another drawing entirely. Every wall of a block is swept off
    // a segment, so no name it holds could be this one.
    let mut elsewhere = Sketch::default();
    let middle = elsewhere.add_point(DVec2::ZERO);
    let stranger = STEP.grew(Grown::Side(Bound {
        of: Entity::Circle(elsewhere.add_circle(middle, 1.0)),
        along: true,
    }));
    assert!(!body.holds(stranger));
    mesher.cut(&body, stranger, 1e-3, &mut patch);
    assert!(patch.corners.is_empty(), "it cut a face it does not have");
    assert!(patch.normals.is_empty());
    assert!(patch.triangles.is_empty());
}

/// **A wall the sagitta rules into cells comes out a strip**, and there is no
/// line across it to cut at — which is the whole of what [`Lattice`] is for.
///
/// A face on a unit cylinder covering half a turn and standing three tall. At a
/// sagitta of a thousandth the surface allows `2 acos(1 − 1e-3)` of a turn per
/// chord — the same figure [`chords`](crate::math::arc::chords) cuts its
/// boundary at, which
/// `each_surface_allows_the_step_its_own_arcs_are_chorded_at` pins — and it
/// allows *anything* along the axis, a cylinder not bending that way. So the
/// wall comes out thirty-five cells round and *one* tall, which is a shape a
/// shortest-edge clipper has only one sensible way to cut. A plane bends
/// neither way and comes out the single cell it may as well be.
///
/// **And the rule for cutting is a cell, not a line.** Written out because it
/// is the whole of the stopping rule and it has two edges that are easy to get
/// wrong: a run reaching over more than a cell is cut wherever it lies, and a
/// run reaching over a cell or less is left alone *even when it straddles a
/// line* — cutting those would double the mesh of every curved face to buy
/// nothing, and cutting one over by an ulp never stops at all.
#[test]
fn a_face_is_ruled_into_the_cells_its_surface_allows() {
    let sagitta = 1e-3;
    let wall = Surface::Cylinder(Cylinder {
        axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
        radius: 1.0,
    });
    let cell = arc::widest(1.0, sagitta);
    let around = [
        DVec2::new(0.0, 0.0),
        DVec2::new(PI, 0.0),
        DVec2::new(PI, 3.0),
        DVec2::new(0.0, 3.0),
    ];
    let ruled = Lattice::of(&wall, &around, sagitta);
    let corner = ruled.celled(DVec2::new(PI, 3.0));
    assert!((corner.x - 35.12).abs() < 0.01, "{corner:?} cells round");
    assert_eq!(corner.y, 1.0, "a cylinder does not bend along its axis");
    assert_eq!(
        ruled.celled(DVec2::new(cell, 0.0)).x,
        1.0,
        "a step is a cell"
    );
    assert_eq!(ruled.parameters(corner), DVec2::new(PI, 3.0));

    // **Cut across the turn and never along it.** A run reaching over three
    // cells is cut, on a line and between its ends; one straight up the wall
    // reaches over nothing, the only lines that way being its own top and
    // bottom.
    let cut = ruled
        .cutting(DVec2::new(cell, 0.0), DVec2::new(4.0 * cell, 3.0), 0)
        .expect("three cells is more than one");
    let counted = ruled.celled(cut).x;
    assert!(counted > 1.0 && counted < 4.0);
    assert!(
        (counted - counted.round()).abs() < 1e-12,
        "{cut:?} is not on a line",
    );
    assert_eq!(
        ruled.cutting(DVec2::new(cell, 0.0), DVec2::new(4.0 * cell, 3.0), 1),
        None,
    );

    // **Exactly a cell, and a cell straddling a line, are both left alone** —
    // the two edges of the rule. Straddling one is the ordinary case for a
    // strip and cutting it would buy nothing; being over by an ulp is what
    // `ROUNDING` in [`Lattice::cutting`] is there for.
    for run in [
        [DVec2::new(cell, 0.0), DVec2::new(2.0 * cell, 3.0)],
        [DVec2::new(0.6 * cell, 0.0), DVec2::new(1.4 * cell, 3.0)],
        [DVec2::new(cell, 0.5), DVec2::new(1.2 * cell, 2.5)],
    ] {
        assert_eq!(ruled.cutting(run[0], run[1], 0), None, "{run:?} was cut");
    }

    // A face narrower than one step of its own surface is one cell wide, not a
    // fraction of one: there is nothing to divide it into.
    let sliver = [DVec2::ZERO, DVec2::new(0.01, 0.0), DVec2::new(0.01, 3.0)];
    let thin = Lattice::of(&wall, &sliver, sagitta);
    assert_eq!(thin.celled(DVec2::new(0.01, 3.0)), DVec2::ONE);

    let flat = Surface::Plane(Plane::GROUND);
    let sheet = [
        DVec2::new(-2.0, -5.0),
        DVec2::new(6.0, -5.0),
        DVec2::new(6.0, 15.0),
        DVec2::new(-2.0, 15.0),
    ];
    let one = Lattice::of(&flat, &sheet, sagitta);
    assert_eq!(one.celled(sheet[2]) - one.celled(sheet[0]), DVec2::ONE);
    for axis in 0..2 {
        assert_eq!(
            one.cutting(sheet[0], sheet[2], axis),
            None,
            "a plane has a line"
        );
    }
}

/// **A curved face whose parameters are not a rectangle still follows the
/// surface** — the one thing a boundary flattened finely does not buy on its
/// own.
///
/// A unit rod cut across at forty-five degrees, which leaves each half of its
/// wall bounded below by a circle and above by an ellipse. The region is wide
/// in `u` and *fat*: a triangulation that joined corners by how near they look
/// in raw parameters lays a triangle clean across it, whose chord cuts through
/// the rod, and no sagitta ever asked for buys it back.
///
/// The two halves cover `3π + 2` and `3π − 2` exactly — `∫(3 + sin θ) dθ` over
/// each half turn, the wall standing `3 + sin θ` tall where the plane `y + z =
/// 3` crosses it. Both are asserted to converge, and every triangle is held
/// against what the sagitta allows it to stray, which is the claim itself
/// rather than a likeness of it.
#[test]
fn a_curved_face_wider_than_it_is_tall_still_follows_its_surface() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 1.0);
    let found = Arrangement::of(&sketch);
    let rod = Extrusion::new(&found, 0, Plane::GROUND, 6.0, STEP).body();
    let leaning = Plane {
        origin: DVec3::new(0.0, 3.0, 0.0),
        x: DVec3::X,
        y: DVec3::new(0.0, 1.0, -1.0).normalize(),
    };
    let mut cutting = Sketch::default();
    cutting.outline(&[(-5.0, -5.0), (5.0, -5.0), (5.0, 5.0), (-5.0, 5.0)]);
    let lid = Extrusion::new(&Arrangement::of(&cutting), 0, leaning, 10.0, Step(1)).body();
    let mut into = Body::default();
    assert!(Boolean::default().combine(&rod, &lid, Operation::Cut, &mut into));

    let want = [3.0 * PI + 2.0, 3.0 * PI - 2.0];
    let mut mesher = Mesher::default();
    let mut patch = Patch::default();
    let mut last = f64::INFINITY;
    for sagitta in [1e-2, 1e-3, 1e-4] {
        let mut covered = Vec::new();
        for (at, face) in into.topology().faces() {
            let Surface::Cylinder(_) = face.surface else {
                continue;
            };
            mesher.shut_in(&into, &[at], sagitta, &mut patch);
            let mut area = 0.0;
            for &[a, b, c] in &patch.triangles {
                let (a, b, c) = (
                    patch.corners[a as usize],
                    patch.corners[b as usize],
                    patch.corners[c as usize],
                );
                area += (b - a).cross(c - a).length() * 0.5;
                let corner = face.surface.uv(a);
                // Read back unwrapped, the way a face's own walk reads them:
                // an inversion answers in a half turn either side of the
                // reference, so a triangle over the seam would otherwise look
                // like one covering the whole cylinder.
                let mut uv = [a, b, c].map(|at| face.surface.uv(at));
                for uv in &mut uv[1..] {
                    uv.x += TAU * ((corner.x - uv.x) / TAU).round();
                }
                // To within a rounding, a cell being allowed to come out that
                // much wide — the same reading `Refining::held` takes.
                let strayed = face.surface.straying(uv);
                assert!(
                    predicate::touching(strayed, predicate::slack(sagitta)),
                    "a triangle stands {strayed} off the wall at a sagitta of {sagitta}",
                );
            }
            covered.push(area);
        }
        assert_eq!(
            covered.len(),
            2,
            "the wall is not the two faces §4.4 splits"
        );
        covered.sort_by(f64::total_cmp);
        // A chorded wall is inscribed in the true one, so it always reads short.
        let off: f64 = want
            .iter()
            .zip(covered.iter().rev())
            .map(|(want, read)| {
                assert!(read < want, "{read} covers more than the {want} there is");
                want - read
            })
            .sum();
        assert!(off < last, "{sagitta} read no nearer than the last: {off}");
        last = off;
    }
    assert!(last < 1e-3, "the wall never converged: {last} short");
}
