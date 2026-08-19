use crate::math::plane::Plane;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::entity::Entity;
use crate::solid::build::extrusion::Extrusion;
use crate::solid::grown::Grown;
use crate::solid::mesh::{Mesher, Patch};
use crate::solid::topology::body::Body;
use glam::{DVec2, DVec3};
use std::f64::consts::PI;

/// A two-by-two block three deep, standing with one corner on the origin.
fn block() -> Body {
    let mut sketch = Sketch::default();
    sketch.outline(&[(0.0, 0.0), (2.0, 0.0), (2.0, 2.0), (0.0, 2.0)]);
    let found = Arrangement::of(&sketch);
    Extrusion::new(&found, 0, Plane::GROUND, 3.0).body()
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

    for grown in body.grown() {
        mesher.cut(&body, grown, 1e-3, &mut patch);
        assert_eq!(patch.corners.len(), patch.normals.len());
        assert!(!patch.triangles.is_empty(), "{grown:?} cut to nothing");

        for (&corner, &normal) in patch.corners.iter().zip(&patch.normals) {
            assert!(
                (normal.length() - 1.0).abs() < 1e-12,
                "{grown:?} is not unit"
            );
            assert!(
                (corner - middle).dot(normal) > 0.0,
                "{grown:?} faces inward at {corner:?}",
            );
        }
        for &[a, b, c] in &patch.triangles {
            let corner = |at: u32| patch.corners[at as usize];
            let wound = (corner(b) - corner(a)).cross(corner(c) - corner(a));
            assert!(
                wound.dot(patch.normals[a as usize]) > 0.0,
                "{grown:?} is wound against its own normals",
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
    let body = Extrusion::new(&found, 0, Plane::GROUND, 4.0).body();

    let mut mesher = Mesher::default();
    let (mut base, mut wall) = (Patch::default(), Patch::default());
    mesher.cut(&body, Grown::Base, 1e-2, &mut base);
    mesher.cut(
        &body,
        Grown::Side(Bound {
            of: Entity::Circle(ring),
            along: true,
        }),
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
    let body = Extrusion::new(&found, 0, Plane::GROUND, 4.0).body();

    let true_volume = PI * 4.0 * 4.0;
    let mut mesher = Mesher::default();
    let mut patch = Patch::default();
    let mut last = (0usize, f64::INFINITY);
    for sagitta in [0.5, 0.05, 5e-3, 5e-5] {
        mesher.cut(&body, Grown::Base, sagitta, &mut patch);
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

    mesher.cut(&body, Grown::Base, 1e-3, &mut patch);
    assert!(!patch.corners.is_empty());

    // A circle of another drawing entirely. Every wall of a block is swept off
    // a segment, so no name it holds could be this one.
    let mut elsewhere = Sketch::default();
    let middle = elsewhere.add_point(DVec2::ZERO);
    let stranger = Grown::Side(Bound {
        of: Entity::Circle(elsewhere.add_circle(middle, 1.0)),
        along: true,
    });
    assert!(!body.holds(stranger));
    mesher.cut(&body, stranger, 1e-3, &mut patch);
    assert!(patch.corners.is_empty(), "it cut a face it does not have");
    assert!(patch.normals.is_empty());
    assert!(patch.triangles.is_empty());
}
