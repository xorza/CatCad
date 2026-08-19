use super::*;
use crate::Plane;
use crate::math::winding;
use crate::sketch::Sketch;
use crate::sketch::arrangement::Arrangement;
use crate::solid::build::extrusion::Extrusion;
use crate::solid::grown::Grown;
use crate::solid::named::Step;

/// The two steps the blocks below are grown by.
///
/// Two, and that is the point: a boolean's answer holds faces of both operands,
/// and which feature grew a face is half of what names it — see
/// [`Named`](crate::solid::named::Named). Both bodies calling their base
/// `Grown::Base` is exactly the collision the other half is for.
const CUBE: Step = Step(1);
const TOOL: Step = Step(2);

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
    let raised = Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal(),
        ..Plane::GROUND
    };
    block(
        raised,
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
    let onto = Plane {
        origin: Plane::GROUND.origin + Plane::GROUND.normal() * 4.0,
        ..Plane::GROUND
    };
    block(
        onto,
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

/// A body with anything curved in it is refused rather than guessed at.
#[test]
fn a_curved_body_is_refused_by_a_planar_boolean() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(2.0, 2.0));
    sketch.add_circle(middle, 1.0);
    let found = Arrangement::of(&sketch);
    let round = Extrusion::new(&found, 0, Plane::GROUND, 4.0, TOOL).body();

    let mut combining = Combining::default();
    assert!(!combining.combine(&cube(), &round, Operation::Cut));
    assert!(!combining.combine(&round, &cube(), Operation::Cut));
}

// Already inside a `cfg(test)` module, so it needs no gate of its own.
mod matrix;
