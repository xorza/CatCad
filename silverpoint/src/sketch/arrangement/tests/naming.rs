//! What names a face, and what that name still finds once the drawing moves
//! under it.

use crate::math::triangulate::Fill;
use crate::sketch::arrangement::filler::Filler;
use crate::sketch::arrangement::tests::drawings::{
    CLOSE, Halved, areas, bowtie, covering, follows, nested, open, square,
};
use crate::sketch::arrangement::*;
use crate::sketch::entity::Entity;
use std::f64::consts::PI;

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
    let first = sketch.outline(&[(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)]);
    sketch.outline(&[(10.0, 0.0), (12.0, 0.0), (12.0, 2.0), (10.0, 2.0)]);
    sketch.outline(&[(20.0, 0.0), (23.0, 0.0), (23.0, 3.0), (20.0, 3.0)]);

    // One arrangement rebuilt twice rather than two of them, because that is
    // what a drag does — and because a position only means anything if it
    // means the same thing across a rebuild in place.
    let mut found = Arrangement::default();
    found.rebuild(&sketch);
    assert!(
        follows(&found, &[1.0, 4.0, 9.0]),
        "{:?} is not the order the squares were drawn in",
        areas(&found)
    );

    // The first square's near corner, dragged away from everything.
    sketch.set_point(first[0], DVec2::new(-10.0, -10.0));

    found.rebuild(&sketch);
    let after = areas(&found);
    assert!(
        (after[2] - 9.0).abs() < CLOSE,
        "{after:?} moved an untouched square out of third place"
    );
    assert!(
        (after[1] - 4.0).abs() < CLOSE,
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
    let Halved {
        mut sketch,
        circle,
        chord,
    } = Halved::new();
    let cap = Halved::cap();

    let found = Arrangement::of(&sketch);
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
    let base = sketch.add_point(DVec2::new(0.0, 1.0));
    let tip = sketch.add_point(DVec2::new(0.0, 1.5));
    let spur = sketch.add_segment(base, tip);

    let found = Arrangement::of(&sketch);
    assert_eq!(found.faces().len(), 2, "the spur enclosed something");
    for (name, want) in [(&above_by, cap), (&below_by, PI * 4.0 - cap)] {
        let still = found
            .face_named_by(name)
            .unwrap_or_else(|| panic!("{name:?} stopped naming anything"));
        assert!(
            (found.faces()[still].area() - want).abs() < CLOSE,
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
    let mut sketch = square();
    let middle = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.add_circle(middle, 1.0);

    // One arrangement rebuilt rather than two of them, because that is what an
    // edit does — and a position only means anything across a rebuild in place.
    let mut found = Arrangement::default();
    found.rebuild(&sketch);
    assert!(
        follows(&found, &[12.0, PI]),
        "{:?} is not the square and then the disc",
        areas(&found)
    );
    let disc = found.faces()[1].named().to_vec();
    let square_by = found.faces()[0].named().to_vec();

    sketch.outline(&[(20.0, 0.0), (22.0, 0.0), (22.0, 2.0), (20.0, 2.0)]);
    found.rebuild(&sketch);
    assert!(
        follows(&found, &[12.0, 4.0, PI]),
        "{:?} is not the two squares and then the disc",
        areas(&found)
    );

    // The position that named the disc now names the square drawn after it —
    // which is the silent failure the name exists to refuse.
    assert!(
        (found.faces()[1].area() - 4.0).abs() < CLOSE,
        "{:?} left the disc where it was, so this proves nothing",
        areas(&found)
    );
    assert_eq!(found.face_named_by(&disc), Some(2));

    // Drawing *across* the region is the one thing that does break the name,
    // and it says so rather than answering with whichever half covers most:
    // neither half is bounded by what the disc was bounded by.
    let left = sketch.add_point(DVec2::new(8.0, 0.5));
    let right = sketch.add_point(DVec2::new(12.0, 0.5));
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
    let hub = sketch.add_point(DVec2::new(4.0, 1.5));
    let bite = sketch.add_circle(hub, 1.0);
    found.rebuild(&sketch);

    // Half the circle falls each side of the edge it is centred on.
    let bitten = covering(&found, 12.0 - PI / 2.0);
    let bitten_by = found.faces()[bitten].named().to_vec();
    assert!(
        bitten_by.len() == 5 && square_by.iter().all(|bound| bitten_by.contains(bound)),
        "the bitten square is not the square's four edges and one more, so this proves nothing: \
         {bitten_by:?} against {square_by:?}"
    );
    assert_eq!(found.face_named_by(&square_by), None);

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
    assert_eq!(found.face_named_by(&square_by), Some(0));
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
    /// One drawing in the sweep, and what to call it when it fails.
    type Case = (&'static str, fn() -> Sketch);

    let drawings: [Case; 5] = [
        ("nested", nested),
        ("bowtie", bowtie),
        ("square", square),
        ("open", open),
        ("empty", Sketch::default),
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
            let fresh = Arrangement::of(&sketch);

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
