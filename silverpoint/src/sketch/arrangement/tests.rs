use super::*;
use crate::math::triangulate::Fill;
use crate::sketch::PointId;
use crate::sketch::arrangement::filler::Filler;
use std::f64::consts::PI;

/// Every face's area, largest first — which is the order they come back in, and
/// enough to say what a drawing enclosed without naming a single edge.
fn areas(of: &Arrangement) -> Vec<f64> {
    of.faces().iter().map(Face::area).collect()
}

/// Whether the areas are these, to within a rounding, in any order.
///
/// Order is not what the areas say — it is the order the curves are walked in,
/// which is what a caller names a face *by* rather than something the sizes
/// decide. What pins that is
/// `the_order_faces_come_back_in_survives_the_geometry_moving`.
fn covers(of: &Arrangement, want: &[f64]) -> bool {
    let sorted = |mut of: Vec<f64>| {
        of.sort_by(|a, b| a.partial_cmp(b).expect("areas are finite"));
        of
    };
    let (found, want) = (sorted(areas(of)), sorted(want.to_vec()));
    found.len() == want.len()
        && found
            .iter()
            .zip(&want)
            .all(|(got, want)| (got - want).abs() < 1e-9)
}

/// One arrangement of `sketch`, through an arrangement stood up for the call.
///
/// Most tests here ask about one drawing, so nothing is saved by keeping the
/// arrangement — what keeping it saves is pinned by
/// `a_reused_arrangement_answers_exactly_as_a_fresh_one_would` and by the
/// application's allocation gates.
fn arranged(sketch: &Sketch) -> Arrangement {
    let mut found = Arrangement::default();
    found.rebuild(sketch);
    found
}

fn point(sketch: &mut Sketch, x: f64, y: f64) -> PointId {
    sketch.add_point(DVec2::new(x, y))
}

/// A closed run of segments through the given corners.
fn outline(sketch: &mut Sketch, corners: &[(f64, f64)]) {
    let placed: Vec<_> = corners.iter().map(|&(x, y)| point(sketch, x, y)).collect();
    for at in 0..placed.len() {
        sketch.add_segment(placed[at], placed[(at + 1) % placed.len()]);
    }
}

/// A closed run of edges shuts in one face, and an open one shuts in nothing.
#[test]
fn a_closed_outline_encloses_a_face_and_an_open_one_does_not() {
    let mut sketch = Sketch::default();
    outline(
        &mut sketch,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)],
    );
    assert!(
        covers(&arranged(&sketch), &[12.0]),
        "{:?}",
        areas(&arranged(&sketch))
    );

    // The same run with one edge missing shuts in nothing at all: three sides
    // of a rectangle is a line, however nearly it closes.
    let mut open = Sketch::default();
    let placed: Vec<_> = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
        .iter()
        .map(|&(x, y)| point(&mut open, x, y))
        .collect();
    for at in 0..3 {
        open.add_segment(placed[at], placed[at + 1]);
    }
    assert!(arranged(&open).faces().is_empty());
}

/// A lone circle is its own loop, with no corner on it until one is planted.
#[test]
fn a_circle_nothing_crosses_encloses_its_own_disc() {
    let mut sketch = Sketch::default();
    let middle = point(&mut sketch, 1.0, -2.0);
    sketch.add_circle(middle, 3.0);

    let found = arranged(&sketch);
    assert!(covers(&found, &[PI * 9.0]), "{:?}", areas(&found));
    assert_eq!(found.faces()[0].holes(), 0);
}

/// **A segment cutting a circle in half makes two faces.**
///
/// The first of the three cases that say an arrangement is more than finding
/// loops in what was drawn: neither half exists as anything the sketch holds,
/// and both are made by cutting a circle and a segment at the two places they
/// cross.
#[test]
fn a_segment_across_a_circle_cuts_it_into_two_faces() {
    let mut sketch = Sketch::default();
    let middle = point(&mut sketch, 0.0, 0.0);
    sketch.add_circle(middle, 2.0);
    // Straight through the centre and out the far side, so the chord is a
    // diameter and the two halves come out the same size.
    let left = point(&mut sketch, -5.0, 0.0);
    let right = point(&mut sketch, 5.0, 0.0);
    sketch.add_segment(left, right);

    let found = arranged(&sketch);
    let half = PI * 4.0 / 2.0;
    assert!(covers(&found, &[half, half]), "{:?}", areas(&found));
    assert!(found.faces().iter().all(|face| face.holes() == 0));

    // Off centre, the two halves differ — which is what says the cut is being
    // measured rather than assumed. A chord at y = 1 on a radius of 2 cuts off
    // a cap of r²(θ − sin θ)/2 with θ = 2·acos(1/2) = 2π/3.
    let mut offset = Sketch::default();
    let middle = point(&mut offset, 0.0, 0.0);
    offset.add_circle(middle, 2.0);
    let left = point(&mut offset, -5.0, 1.0);
    let right = point(&mut offset, 5.0, 1.0);
    offset.add_segment(left, right);

    let found = arranged(&offset);
    let turn = 2.0 * (0.5_f64).acos();
    let cap = 4.0 * (turn - turn.sin()) / 2.0;
    assert!(
        covers(&found, &[PI * 4.0 - cap, cap]),
        "{:?} against a cap of {cap}",
        areas(&found)
    );
}

/// **A circle inside a circle makes two faces — the inner disc and the ring
/// around it.**
///
/// The case with no crossings anywhere. Nothing here is cut, so the ring exists
/// only because the small circle's outside is found to sit within the large
/// one's inside — which is the containment step, and the reason loops alone
/// cannot answer this.
#[test]
fn a_circle_inside_a_circle_makes_a_disc_and_a_ring_around_it() {
    let mut sketch = Sketch::default();
    let middle = point(&mut sketch, 0.0, 0.0);
    sketch.add_circle(middle, 3.0);
    // Off-centre, so this is containment rather than anything concentric might
    // be got away with.
    let inner = point(&mut sketch, 0.5, -0.25);
    sketch.add_circle(inner, 1.0);

    let found = arranged(&sketch);
    assert!(covers(&found, &[PI * 9.0 - PI, PI]), "{:?}", areas(&found));
    // The ring is the one with something missing from it.
    assert_eq!(found.faces()[0].holes(), 1, "the ring has no hole in it");
    assert_eq!(found.faces()[1].holes(), 0, "the disc has one");
}

/// **A polyline crossing itself in a figure of eight makes two faces.**
///
/// The crossing is a corner no point of the sketch sits at, and the two lobes
/// are loops no run of the drawn edges traces — both halves of why walking the
/// sketch's own handles could never find these.
#[test]
fn a_polyline_crossing_itself_makes_a_face_on_either_side() {
    let mut sketch = Sketch::default();
    // A bowtie: along the bottom, up the rising diagonal, along the top, and
    // back down the falling one — which puts the two diagonals across each
    // other at the origin.
    outline(
        &mut sketch,
        &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)],
    );

    let found = arranged(&sketch);
    // Each lobe is a triangle two across its base and one tall, so one apiece
    // — and the crossing at the origin is a corner no point of the sketch
    // sits at.
    assert!(covers(&found, &[1.0, 1.0]), "{:?}", areas(&found));
    assert!(found.faces().iter().all(|face| face.holes() == 0));
}

/// A hole goes in the face it is actually cut from, not the outermost one.
#[test]
fn a_ring_inside_a_ring_puts_each_hole_where_it_belongs() {
    let mut sketch = Sketch::default();
    for radius in [4.0, 3.0, 2.0, 1.0] {
        let middle = point(&mut sketch, 0.0, 0.0);
        sketch.add_circle(middle, radius);
    }

    let found = arranged(&sketch);
    // Four circles nested make four faces: three rings and the disc in the
    // middle, each the difference of two consecutive discs.
    assert!(
        covers(
            &found,
            &[PI * (16.0 - 9.0), PI * (9.0 - 4.0), PI * (4.0 - 1.0), PI]
        ),
        "{:?}",
        areas(&found)
    );
    let holes: Vec<usize> = found.faces().iter().map(Face::holes).collect();
    assert_eq!(holes, [1, 1, 1, 0], "a hole landed in the wrong ring");
}

/// What a face is filled with covers exactly what it says it encloses.
///
/// The join between the topology above and the triangles below: an arrangement
/// that found the right faces and a fill that cut the wrong polygon would each
/// pass their own tests and disagree here.
#[test]
fn a_face_fills_to_the_area_it_encloses() {
    let mut sketch = Sketch::default();
    let middle = point(&mut sketch, 0.0, 0.0);
    sketch.add_circle(middle, 3.0);
    let inner = point(&mut sketch, 0.5, -0.25);
    sketch.add_circle(inner, 1.0);

    let found = arranged(&sketch);
    let ring = &found.faces()[0];
    let mut fill = Fill::default();
    Filler::default().fill(&found, ring, 1e-4, &mut fill);

    let covered: f64 = fill
        .triangles
        .iter()
        .map(|&[a, b, c]| {
            let (a, b, c) = (
                fill.corners[a as usize],
                fill.corners[b as usize],
                fill.corners[c as usize],
            );
            (b - a).perp_dot(c - a) / 2.0
        })
        .sum();
    // Flattening cuts corners off both circumferences, so the fill lands a
    // shade under what the true curves enclose — and not by much at this
    // sagitta.
    assert!(
        covered < ring.area() && covered > ring.area() * 0.9999,
        "{covered} against {}",
        ring.area()
    );

    // And it says what draws it, which is what a later feature selects by.
    let mut drawn = Vec::new();
    found.drawn_by(ring, &mut drawn);
    assert_eq!(drawn.len(), 1, "the ring is drawn by one circle");

    // A second hole, clear of the first, and the fill still covers exactly what
    // the face says it does.
    //
    // The one that has to be asked separately rather than trusted to follow.
    // Every hole of a face is traced into *one* buffer, each recording where it
    // landed in it — so a tracer that emptied that buffer rather than appending
    // to it would leave the first hole overwritten by the second and the second
    // recorded as the fragment past where the first had ended. Neither hole
    // then closes, and the region lost outright gets filled over: with one hole
    // there is nothing to overwrite and nothing to notice.
    let far = point(&mut sketch, -1.2, 1.0);
    sketch.add_circle(far, 0.6);
    let found = arranged(&sketch);
    let ring = found
        .faces()
        .iter()
        .find(|face| face.holes() == 2)
        .expect("the outer circle has both smaller ones cut from it");
    Filler::default().fill(&found, ring, 1e-4, &mut fill);

    // Both holes are there to be cut out: the corners are the outline's and
    // then each hole's, so a hole that went missing shows up as a short list
    // before it shows up as area.
    assert!(
        fill.corners.len() > 3,
        "the fill kept no corners: {}",
        fill.corners.len()
    );
    let covered: f64 = fill
        .triangles
        .iter()
        .map(|&[a, b, c]| {
            let (a, b, c) = (
                fill.corners[a as usize],
                fill.corners[b as usize],
                fill.corners[c as usize],
            );
            (b - a).perp_dot(c - a) / 2.0
        })
        .sum();
    assert!(
        covered < ring.area() && covered > ring.area() * 0.9999,
        "{covered} against {} — a hole was filled over",
        ring.area()
    );
    // The area itself is the three circles, so the fill is being checked
    // against a number the drawing states rather than one it computed.
    let stated = PI * (3.0 * 3.0 - 1.0 * 1.0 - 0.6 * 0.6);
    assert!(
        (ring.area() - stated).abs() < 1e-9,
        "{} against {stated}",
        ring.area()
    );
}

/// The order faces come back in survives the geometry moving.
///
/// What a caller naming a face by its position rests on. A drag changes where
/// every corner is without changing what crosses what, and the walk has to
/// rebuild the same list in the same places — so the region that was third
/// before is the region that is third after, whatever the sizes did.
///
/// Three squares, drawn smallest first and well apart, so that the order they
/// are walked in and the order their areas would sort in disagree from the
/// start. Then the first is grown past the last, out into empty ground where it
/// crosses nothing: the drawing's shape is untouched, and only its sizes move.
#[test]
fn the_order_faces_come_back_in_survives_the_geometry_moving() {
    let mut sketch = Sketch::default();
    outline(
        &mut sketch,
        &[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)],
    );
    outline(
        &mut sketch,
        &[(10.0, 0.0), (12.0, 0.0), (12.0, 2.0), (10.0, 2.0)],
    );
    outline(
        &mut sketch,
        &[(20.0, 0.0), (23.0, 0.0), (23.0, 3.0), (20.0, 3.0)],
    );

    // One arrangement rebuilt twice rather than two of them, because that is
    // what a drag does — and because a position only means anything if it
    // means the same thing across a rebuild in place.
    let mut found = Arrangement::default();
    found.rebuild(&sketch);
    let before = areas(&found);
    assert!(
        before
            .iter()
            .zip([1.0, 4.0, 9.0])
            .all(|(got, want)| (got - want).abs() < 1e-9),
        "{before:?} is not the order the squares were drawn in"
    );

    // The first square's near corner, dragged away from everything.
    let corner = sketch.points().next().expect("the first square has one").0;
    sketch.set_point(corner, DVec2::new(-10.0, -10.0));

    found.rebuild(&sketch);
    let after = areas(&found);
    assert!(
        (after[2] - 9.0).abs() < 1e-9,
        "{after:?} moved an untouched square out of third place"
    );
    assert!(
        (after[1] - 4.0).abs() < 1e-9,
        "{after:?} moved an untouched square out of second place"
    );
    // And the growth was real: sorting by size would have put this one first
    // and shuffled the other two down, which is exactly what must not happen.
    assert!(after[0] > after[2], "{after:?} did not grow past the third");
}

/// A reused arrangement answers exactly as a fresh one would.
///
/// What keeping the room costs, if it costs anything. Every list a rebuild
/// works in is emptied and refilled rather than dropped, and several are
/// emptied by a *count* rather than by clearing — so the failure this is
/// looking for is a rebuild reading something the last one left: a face that
/// keeps a hole it no longer has, a fan of departures with a stale half-edge
/// still in it, an outside loop assigned twice.
///
/// Swept over drawings that differ in every way one rebuild could carry into
/// the next — more faces then fewer, holes then none, curves then none at all —
/// and each is asked in both orders, because a leak only shows going one way.
#[test]
fn a_reused_arrangement_answers_exactly_as_a_fresh_one_would() {
    let nested = || {
        let mut sketch = Sketch::default();
        for radius in [4.0, 3.0, 2.0, 1.0] {
            let middle = point(&mut sketch, 0.0, 0.0);
            sketch.add_circle(middle, radius);
        }
        sketch
    };
    let bowtie = || {
        let mut sketch = Sketch::default();
        outline(
            &mut sketch,
            &[(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)],
        );
        sketch
    };
    let square = || {
        let mut sketch = Sketch::default();
        outline(
            &mut sketch,
            &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)],
        );
        sketch
    };
    // Three sides of a rectangle: curves that shut nothing in, so a rebuild
    // over it has to leave no face behind from whatever came before.
    let open = || {
        let mut sketch = Sketch::default();
        let placed: Vec<_> = [(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)]
            .iter()
            .map(|&(x, y)| point(&mut sketch, x, y))
            .collect();
        for at in 0..3 {
            sketch.add_segment(placed[at], placed[at + 1]);
        }
        sketch
    };
    let empty = Sketch::default;

    /// One drawing in the sweep, and what to call it when it fails.
    type Case = (&'static str, fn() -> Sketch);

    let drawings: [Case; 5] = [
        ("nested", nested),
        ("bowtie", bowtie),
        ("square", square),
        ("open", open),
        ("empty", empty),
    ];

    let mut reused = Arrangement::default();
    let mut filler = Filler::default();
    let (mut one_drawn, mut other_drawn) = (Vec::new(), Vec::new());
    for (before, first) in drawings {
        for (after, build) in drawings {
            // Wound up to the first drawing, then over to the second — against
            // an arrangement that has only ever seen the second.
            reused.rebuild(&first());
            let sketch = build();
            reused.rebuild(&sketch);
            let fresh = arranged(&sketch);

            assert_eq!(
                reused.faces().len(),
                fresh.faces().len(),
                "{before} then {after} left a different number of faces"
            );
            for (at, (was, is)) in fresh.faces().iter().zip(reused.faces()).enumerate() {
                assert!(
                    (was.area() - is.area()).abs() < 1e-12,
                    "{before} then {after}: face {at} covers {} against {}",
                    is.area(),
                    was.area()
                );
                assert_eq!(
                    was.holes(),
                    is.holes(),
                    "{before} then {after}: face {at} kept a hole it does not have"
                );
                fresh.drawn_by(was, &mut one_drawn);
                reused.drawn_by(is, &mut other_drawn);
                assert_eq!(
                    one_drawn, other_drawn,
                    "{before} then {after}: face {at} is drawn by different curves"
                );
            }

            // And the fills agree, which is the other half kept across a
            // rebuild — the tracing buffers and the cutter's contour.
            let (mut one, mut other) = (Fill::default(), Fill::default());
            for (was, is) in fresh.faces().iter().zip(reused.faces()) {
                filler.fill(&fresh, was, 1e-3, &mut one);
                filler.fill(&reused, is, 1e-3, &mut other);
                assert_eq!(
                    one.corners.len(),
                    other.corners.len(),
                    "{before} then {after}: a fill came out a different shape"
                );
                assert_eq!(
                    one.triangles, other.triangles,
                    "{before} then {after}: a fill came out cut differently"
                );
            }
        }
    }
}
