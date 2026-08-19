//! What a heap of curves shuts in, and what fills it.

use crate::math::triangulate::Fill;
use crate::sketch::arrangement::filler::Filler;
use crate::sketch::arrangement::tests::drawings::{
    CLOSE, Halved, areas, bowtie, covers, nested, open, pierced, square,
};
use crate::sketch::arrangement::*;
use std::f64::consts::PI;

/// A closed run of edges shuts in one face, and an open one shuts in nothing.
#[test]
fn a_closed_outline_encloses_a_face_and_an_open_one_does_not() {
    let found = Arrangement::of(&square());
    assert!(covers(&found, &[12.0]), "{:?}", areas(&found));

    // The same run with one edge missing shuts in nothing at all: three sides
    // of a rectangle is a line, however nearly it closes.
    assert!(Arrangement::of(&open()).faces().is_empty());
}

/// A lone circle is its own loop, with no corner on it until one is planted.
#[test]
fn a_circle_nothing_crosses_encloses_its_own_disc() {
    let mut sketch = Sketch::default();
    let middle = sketch.add_point(DVec2::new(1.0, -2.0));
    sketch.add_circle(middle, 3.0);

    let found = Arrangement::of(&sketch);
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
    let middle = sketch.add_point(DVec2::ZERO);
    sketch.add_circle(middle, 2.0);
    // Straight through the centre and out the far side, so the chord is a
    // diameter and the two halves come out the same size.
    let left = sketch.add_point(DVec2::new(-5.0, 0.0));
    let right = sketch.add_point(DVec2::new(5.0, 0.0));
    sketch.add_segment(left, right);

    let found = Arrangement::of(&sketch);
    let half = PI * 4.0 / 2.0;
    assert!(covers(&found, &[half, half]), "{:?}", areas(&found));
    assert!(found.faces().iter().all(|face| face.holes() == 0));

    // Off centre, which is what says the cut is being measured rather than
    // assumed.
    let found = Arrangement::of(&Halved::new().sketch);
    let cap = Halved::cap();
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
    let found = Arrangement::of(&pierced());
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
    let found = Arrangement::of(&bowtie());
    // Each lobe is a triangle two across its base and one tall, so one apiece
    // — and the crossing at the origin is a corner no point of the sketch
    // sits at.
    assert!(covers(&found, &[1.0, 1.0]), "{:?}", areas(&found));
    assert!(found.faces().iter().all(|face| face.holes() == 0));
}

/// A hole goes in the face it is actually cut from, not the outermost one.
#[test]
fn a_ring_inside_a_ring_puts_each_hole_where_it_belongs() {
    let found = Arrangement::of(&nested());
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
    let mut sketch = pierced();

    // Flattening cuts corners off every circumference, so a fill lands a shade
    // under what the true curves enclose — and not by much at this sagitta.
    // Asked of both fills below, which is what says the second hole cost the
    // face nothing it should have kept.
    let just_under = |covered: f64, area: f64| covered < area && covered > area * 0.9999;

    let found = Arrangement::of(&sketch);
    let ring = &found.faces()[0];
    let mut fill = Fill::default();
    Filler::default().fill(&found, ring, 1e-4, &mut fill);

    let covered = fill.covered();
    assert!(
        just_under(covered, ring.area()),
        "{covered} against {}",
        ring.area()
    );

    // And it says what bounds it, which is what a later feature names it by.
    // The ring is walked round the outer circle counterclockwise, and the hole
    // it is cut from is no part of what names it.
    let named = ring.named();
    assert_eq!(
        named.len(),
        1,
        "the ring is bounded by one circle: {named:?}"
    );
    assert!(named[0].along, "the ring walks its own outline backwards");

    // What the region has walls on is the other reading of the same boundary,
    // and the difference between the two is exactly the hole: a bore carried
    // off the plane is as much a face of the solid as its outside, where a hole
    // appearing or vanishing must not change which region a name means. That
    // reading belongs to whatever raises the solid — see
    // `solid::build`'s own tests, which assert it on the body.
    assert_eq!(ring.holes(), 1, "the bore stopped being a hole");

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
    let far = sketch.add_point(DVec2::new(-1.2, 1.0));
    sketch.add_circle(far, 0.6);
    let found = Arrangement::of(&sketch);
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
    let covered = fill.covered();
    assert!(
        just_under(covered, ring.area()),
        "{covered} against {} — a hole was filled over",
        ring.area()
    );
    // The area itself is the three circles, so the fill is being checked
    // against a number the drawing states rather than one it computed.
    let stated = PI * (3.0 * 3.0 - 1.0 * 1.0 - 0.6 * 0.6);
    assert!(
        (ring.area() - stated).abs() < CLOSE,
        "{} against {stated}",
        ring.area()
    );
}

/// **Crossings a hair apart are one corner, not several.**
///
/// Three lines drawn through almost the same place cross in three places rather
/// than one, none of them where any was drawn to be: the arithmetic puts them a
/// fraction of a unit apart, and what the drawing means is a single junction.
/// Left as three, the drawing gains a triangle nobody drew and the two edges
/// meeting at each of them gain a sliver of face between them.
///
/// Asked of the corners rather than of the faces, because the faces cannot tell
/// the two apart where the miss is small: a triangle that size is under
/// [`SLIVER`] and is thrown away for its area whether or not the corners were
/// folded. What says the fold happened is that there is one corner where there
/// would otherwise be three.
///
/// Both misses over one fixture, because a count means nothing on its own — the
/// same three lines missing by a quarter of a unit keep their three crossings
/// and shut in the triangle between them, which is what says the count above is
/// the fold rather than the arithmetic.
#[test]
fn crossings_within_a_rounding_of_each_other_fold_to_one_corner() {
    // A tenth of the fold's own reach, and a quarter of a unit.
    for (miss, corners, faces) in [(1e-10, 7, 0), (0.25, 9, 1)] {
        let mut sketch = Sketch::default();
        let flat = [
            sketch.add_point(DVec2::new(-1.0, 0.0)),
            sketch.add_point(DVec2::new(1.0, 0.0)),
        ];
        sketch.add_segment(flat[0], flat[1]);
        let upright = [
            sketch.add_point(DVec2::new(0.0, -1.0)),
            sketch.add_point(DVec2::new(0.0, 1.0)),
        ];
        sketch.add_segment(upright[0], upright[1]);
        // `y = x + miss`, which meets the flat one at `(−miss, 0)` and the
        // upright one at `(0, miss)` — neither of them the origin, where the
        // other two meet.
        let across = [
            sketch.add_point(DVec2::new(-1.0, -1.0 + miss)),
            sketch.add_point(DVec2::new(1.0, 1.0 + miss)),
        ];
        sketch.add_segment(across[0], across[1]);

        // Six ends, all far apart, and then either one folded junction or the
        // three separate crossings.
        let found = Arrangement::of(&sketch);
        assert_eq!(
            found.corners().len(),
            corners,
            "missing by {miss} left {:?}",
            found.corners()
        );
        assert_eq!(
            found.faces().len(),
            faces,
            "missing by {miss} enclosed {:?}",
            areas(&found)
        );
    }
}
