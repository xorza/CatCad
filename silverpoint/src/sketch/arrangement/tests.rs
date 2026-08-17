use super::*;
use crate::math::triangulate::Fill;
use crate::sketch::PointId;
use crate::sketch::arrangement::filler::Filler;
use crate::sketch::entity::Entity;
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

/// Where the face covering `want` fell.
///
/// How a test names a region it did not draw — and most of the regions here are
/// ones nobody drew, being what a heap of curves happened to shut in. What a
/// face covers is the one thing it says about itself that can be worked out by
/// hand, so it is what a test has to find one by.
fn covering(of: &Arrangement, want: f64) -> usize {
    of.faces()
        .iter()
        .position(|face| (face.area() - want).abs() < 1e-9)
        .unwrap_or_else(|| panic!("no face covers {want}: {:?}", areas(of)))
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

    // Its *walls* are the other reading of the same face, and the difference
    // between the two is exactly the hole: a bore carried off the plane is as
    // much a face of the solid as its outside, where a hole appearing or
    // vanishing must not change which region a name means.
    let walls = ring.walls();
    assert_eq!(walls.len(), 2, "the bore raises no wall: {walls:?}");
    assert_eq!(walls[0], named[0], "the outline comes first");
    assert!(
        !walls[1].along,
        "the hole is walked the other way round: {walls:?}"
    );

    // And each wall knows which pieces of curve it is swept from. Nothing
    // crosses either circle, so each is one whole arc — and the two loops the
    // face is walked along are where those two arcs come from.
    assert_eq!(ring.pieces_of(walls[0]), ring.outline());
    assert_eq!(ring.pieces_of(walls[1]).len(), 1);
    // A curve that bounds it on the other side bounds it not at all, which is
    // what keeps a wall named by one from coming back as the far side's.
    let far = Bound {
        of: walls[0].of,
        along: !walls[0].along,
    };
    assert!(ring.pieces_of(far).is_empty());

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

/// **A face is named by what bounds it and which side of each it lies on.**
///
/// The two halves of a cut circle are the case that decides the shape of a
/// name. Both are bounded by the same circle and the same chord, so what a face
/// is drawn by tells them apart not at all. Which way each half *walks* the
/// chord does — a face is walked with what it encloses on the left, so the half
/// above the chord runs along it the way it was drawn and the half below runs
/// back.
#[test]
fn a_face_is_named_by_which_side_of_each_curve_it_lies_on() {
    let mut sketch = Sketch::default();
    let middle = point(&mut sketch, 0.0, 0.0);
    let circle = sketch.add_circle(middle, 2.0);
    // Off centre, so the two halves come out different sizes and each can be
    // told from the other by what it covers rather than by where it fell.
    let left = point(&mut sketch, -5.0, 1.0);
    let right = point(&mut sketch, 5.0, 1.0);
    let chord = sketch.add_segment(left, right);
    // A cap of r²(θ − sin θ)/2 with θ = 2·acos(1/2) = 2π/3.
    let turn = 2.0 * (0.5_f64).acos();
    let cap = 4.0 * (turn - turn.sin()) / 2.0;

    let found = arranged(&sketch);
    let (above, below) = (covering(&found, cap), covering(&found, PI * 4.0 - cap));
    let named = |face: usize| found.faces()[face].named().to_vec();
    let (above_by, below_by) = (named(above), named(below));

    // Panics where the curve bounds nothing here, so the four calls below say
    // between them that both halves are bounded by both curves — which with the
    // count is the whole of "the same two curves bound both", and that is the
    // reason the curves alone could not name either.
    let side = |bounds: &[Bound], of: Entity| {
        bounds
            .iter()
            .find(|bound| bound.of == of)
            .unwrap_or_else(|| panic!("{of:?} bounds nothing here: {bounds:?}"))
            .along
    };
    for bounds in [&above_by, &below_by] {
        assert_eq!(bounds.len(), 2, "not a circle and a chord: {bounds:?}");
    }

    // And the sides are what tell them apart. The chord was drawn left to
    // right, so the half above it is the one that walks it that way.
    assert!(
        side(&above_by, Entity::Segment(chord)),
        "the cap walks the chord backwards"
    );
    assert!(
        !side(&below_by, Entity::Segment(chord)),
        "the larger half walks the chord the way it was drawn"
    );
    // Both walk the circle counterclockwise, being both of them insides — which
    // is what leaves the chord to do the telling apart.
    assert!(side(&above_by, Entity::Circle(circle)));
    assert!(side(&below_by, Entity::Circle(circle)));

    // So each name finds its own region and neither finds its neighbour's.
    assert_eq!(found.face_named_by(&above_by), Some(above));
    assert_eq!(found.face_named_by(&below_by), Some(below));

    // A spur into the cap renames nothing, and this is the half that has to be
    // asked rather than trusted to follow. The chord is cut into more pieces,
    // which the walk goes down one after another; and the walk detours out
    // along the spur and back, so the spur appears in the outline both ways
    // round. Without either rule, drawing a stray line inside a region would
    // rename it and everything built on it would be lost.
    let base = point(&mut sketch, 0.0, 1.0);
    let tip = point(&mut sketch, 0.0, 1.5);
    let spur = sketch.add_segment(base, tip);

    let found = arranged(&sketch);
    assert_eq!(found.faces().len(), 2, "the spur enclosed something");
    for (name, want) in [(&above_by, cap), (&below_by, PI * 4.0 - cap)] {
        let still = found
            .face_named_by(name)
            .unwrap_or_else(|| panic!("{name:?} stopped naming anything"));
        assert!(
            (found.faces()[still].area() - want).abs() < 1e-9,
            "{name:?} found a region covering {} rather than {want}",
            found.faces()[still].area()
        );
    }

    // And a spur raises no wall either, which is the same rule read the other
    // way: a solid grown from the cap has the faces it had before the line was
    // drawn, so nothing built on one of them is lost. Both sides are asked,
    // because a spur is walked out and back and it is having *both* that makes
    // it bound nothing.
    let dangling = &found.faces()[covering(&found, cap)];
    for along in [true, false] {
        let bound = Bound {
            of: Entity::Segment(spur),
            along,
        };
        assert!(
            !dangling.walls().contains(&bound),
            "{bound:?} raised a wall"
        );
        assert!(dangling.pieces_of(bound).is_empty(), "{bound:?} has pieces");
    }
}

/// **A name holds where a position does not.**
///
/// What naming a face by its boundary is for. Every segment is cut before any
/// circle, so a square and a circle drawn apart come back as the square then
/// the disc — and a second square drawn later puts its face *between* them. The
/// position that named the disc then names a region nobody built on, where the
/// name still names the disc.
#[test]
fn a_name_holds_where_a_position_does_not() {
    let mut sketch = Sketch::default();
    outline(
        &mut sketch,
        &[(0.0, 0.0), (4.0, 0.0), (4.0, 3.0), (0.0, 3.0)],
    );
    let middle = point(&mut sketch, 10.0, 0.0);
    sketch.add_circle(middle, 1.0);

    // One arrangement rebuilt rather than two of them, because that is what an
    // edit does — and a position only means anything across a rebuild in place.
    let mut found = Arrangement::default();
    found.rebuild(&sketch);
    let before = areas(&found);
    assert_eq!(before.len(), 2, "{before:?}");
    assert!(
        before
            .iter()
            .zip([12.0, PI])
            .all(|(got, want)| (got - want).abs() < 1e-9),
        "{before:?} is not the square and then the disc"
    );
    let disc = found.faces()[1].named().to_vec();
    let square = found.faces()[0].named().to_vec();

    outline(
        &mut sketch,
        &[(20.0, 0.0), (22.0, 0.0), (22.0, 2.0), (20.0, 2.0)],
    );
    found.rebuild(&sketch);
    let after = areas(&found);
    assert_eq!(after.len(), 3, "{after:?}");
    assert!(
        after
            .iter()
            .zip([12.0, 4.0, PI])
            .all(|(got, want)| (got - want).abs() < 1e-9),
        "{after:?} is not the two squares and then the disc"
    );

    // The position that named the disc now names the square drawn after it —
    // which is the silent failure the name exists to refuse.
    assert!(
        (found.faces()[1].area() - 4.0).abs() < 1e-9,
        "{after:?} left the disc where it was, so this proves nothing"
    );
    assert_eq!(found.face_named_by(&disc), Some(2));

    // Drawing *across* the region is the one thing that does break the name,
    // and it says so rather than answering with whichever half covers most:
    // neither half is bounded by what the disc was bounded by.
    let left = point(&mut sketch, 8.0, 0.5);
    let right = point(&mut sketch, 12.0, 0.5);
    sketch.add_segment(left, right);
    found.rebuild(&sketch);
    assert_eq!(
        found.faces().len(),
        4,
        "the chord did not cut the disc in two: {:?}",
        areas(&found)
    );
    assert_eq!(found.face_named_by(&disc), None);

    // A region bounded by everything the name lists *and something else
    // besides* is not it either. A circle straddling the square's right edge
    // takes a bite out of it — all four edges still bound what is left, on the
    // same sides, and now the circle does too.
    let hub = point(&mut sketch, 4.0, 1.5);
    let bite = sketch.add_circle(hub, 1.0);
    found.rebuild(&sketch);

    // Half the circle falls each side of the edge it is centred on.
    let bitten = covering(&found, 12.0 - PI / 2.0);
    let bitten_by = found.faces()[bitten].named().to_vec();
    assert!(
        bitten_by.len() == 5 && square.iter().all(|bound| bitten_by.contains(bound)),
        "the bitten square is not the square's four edges and one more, so this proves nothing: \
         {bitten_by:?} against {square:?}"
    );
    assert_eq!(found.face_named_by(&square), None);

    // And the same refusal the other way round, which is the one a check on
    // containment alone would miss. Take the circle away and the square comes
    // back bounded by four of the five the bitten one named — everything that
    // name lists but the arc, all on the same sides. It is still not the region
    // the name was minted from: a bite is not the thing it was taken out of.
    sketch.remove_circle(bite);
    found.rebuild(&sketch);
    assert_eq!(found.face_named_by(&bitten_by), None);
    // While the square's own name fits it again — which is what says the
    // refusal above is about the boundary rather than about the face having
    // gone.
    assert_eq!(found.face_named_by(&square), Some(0));
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
                assert_eq!(
                    was.named(),
                    is.named(),
                    "{before} then {after}: face {at} is named differently"
                );
                // The walls and their pieces are two more lists emptied and
                // written over rather than dropped, so a face that used to have
                // a bore is exactly where one would come back holding a wall it
                // no longer has.
                assert_eq!(
                    was.walls(),
                    is.walls(),
                    "{before} then {after}: face {at} is walled differently"
                );
                for &wall in was.walls() {
                    assert_eq!(
                        was.pieces_of(wall),
                        is.pieces_of(wall),
                        "{before} then {after}: face {at} sweeps {wall:?} from other pieces"
                    );
                }
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
        let flat = [point(&mut sketch, -1.0, 0.0), point(&mut sketch, 1.0, 0.0)];
        sketch.add_segment(flat[0], flat[1]);
        let upright = [point(&mut sketch, 0.0, -1.0), point(&mut sketch, 0.0, 1.0)];
        sketch.add_segment(upright[0], upright[1]);
        // `y = x + miss`, which meets the flat one at `(−miss, 0)` and the
        // upright one at `(0, miss)` — neither of them the origin, where the
        // other two meet.
        let across = [
            point(&mut sketch, -1.0, -1.0 + miss),
            point(&mut sketch, 1.0, 1.0 + miss),
        ];
        sketch.add_segment(across[0], across[1]);

        // Six ends, all far apart, and then either one folded junction or the
        // three separate crossings.
        let found = arranged(&sketch);
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
