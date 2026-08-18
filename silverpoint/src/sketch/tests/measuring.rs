//! What a sketch answers about its own geometry: what a dimension would
//! read, where two edges meet, and where a point drops onto one.

use crate::sketch::constraint::{Along, Dimension};
use crate::sketch::*;

/// A dimension offered takes the size the drawing already is, and one that
/// would measure nothing is not offered at all.
///
/// Both halves of what a bar asks of a sketch. The first is what makes a
/// dimension appear reading what it measured rather than demanding a number
/// nobody can type yet; the second is what keeps a horizontal distance off a
/// pair that is already level, where it would state a zero and have no span to
/// be drawn along.
///
/// A relation passes through untouched, which is what lets one call answer a
/// whole table of candidates rather than the caller sorting them into two kinds
/// first.
#[test]
fn a_dimension_is_fitted_to_what_the_drawing_measures_and_dropped_where_it_measures_nothing() {
    let mut sketch = Sketch::default();
    // 3-4-5 from the origin, and a third point level with the first.
    let origin = sketch.add_point(DVec2::ZERO);
    let corner = sketch.add_point(DVec2::new(3.0, 4.0));
    let level = sketch.add_point(DVec2::new(6.0, 0.0));
    let fitted = |a, b, along| {
        sketch.fitted(Constraint::Distance {
            a,
            b,
            along,
            // Nothing, deliberately: what comes back has to be the drawing's
            // answer rather than anything the caller wrote.
            dimension: Dimension::new(0.0),
        })
    };

    // The three readings of one pair are the three sides of that triangle.
    for (along, want) in [
        (Along::Shortest, 5.0),
        (Along::Horizontal, 3.0),
        (Along::Vertical, 4.0),
    ] {
        let offered = fitted(origin, corner, along).expect("the pair measures something");
        assert_eq!(offered.value(), Some(want), "{along:?}");
    }

    // Level with each other, so there is no vertical distance to state — and
    // the other two readings still are, which is what says the refusal is about
    // the reading rather than about the pair.
    assert_eq!(fitted(origin, level, Along::Vertical), None);
    assert_eq!(
        fitted(origin, level, Along::Horizontal).and_then(|offered| offered.value()),
        Some(6.0)
    );

    // A relation has no number to fit and nothing to be empty of, so it comes
    // back as itself — including over the level pair the vertical distance was
    // just refused for, which is exactly the case a drawing offers it in.
    let horizontal = Constraint::Horizontal {
        a: origin,
        b: level,
    };
    assert_eq!(sketch.fitted(horizontal), Some(horizontal));

    // A standoff is fitted off its own perpendicular rather than off the gap
    // between what it names: `corner` is four above the line through the origin
    // and `level`, whatever its x.
    let flat = sketch.add_segment(origin, level);
    let standoff = sketch.fitted(Constraint::Standoff {
        point: corner,
        segment: flat,
        dimension: Dimension::new(0.0),
    });
    assert_eq!(standoff.and_then(|offered| offered.value()), Some(4.0));
}

/// Two edges run together, or cross somewhere, or do neither — and one whose
/// ends have met does neither whatever it is asked against.
///
/// Both questions on one fixture, because they are one reading: how near
/// parallel a pair is decides whether a distance between them means anything
/// *and* whether saying where they meet does, so a test that asked them apart
/// would let the two drift to different answers at the boundary between them.
///
/// The degenerate half is the sharp one. An edge whose ends have met has only
/// the direction a fallback handed it — reading two of those as parallel to each
/// other would be reading agreement into made-up data, and dividing by the sweep
/// they make would answer a NaN.
#[test]
fn two_edges_run_together_or_cross_and_a_collapsed_one_does_neither() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let c = sketch.add_point(DVec2::new(1.0, 0.0));
    let d = sketch.add_point(DVec2::new(4.0, 4.0));
    let e = sketch.add_point(DVec2::ZERO);
    let f = sketch.add_point(DVec2::new(4.0, 3.0));
    let first = sketch.add_segment(a, b);
    let beside = sketch.add_segment(c, d);
    let across = sketch.add_segment(e, f);

    assert!(sketch.parallel(first, beside));
    // Either way round, and an edge with itself, because parallelism is a
    // property of a pair of directions and not of which was named first.
    assert!(sketch.parallel(beside, first));
    assert!(sketch.parallel(first, first));
    // 3-4 against 4-3 is a long way from parallel, and near enough in *length*
    // that a test against the bare cross product rather than the sine would be
    // measuring how big the sketch is.
    assert!(!sketch.parallel(first, across));

    // Where they cross is the same reading answered the other way about, so a
    // pair that runs together has nowhere to meet — including an edge against
    // itself, which runs together everywhere and meets nowhere in particular.
    assert_eq!(sketch.crossing(first, beside), None);
    assert_eq!(sketch.crossing(first, first), None);

    // And a pair at an angle meets where it meets. `first` runs (0,0)→(3,4) and
    // this one (0,4)→(3,0), so they cross at half of each: (1.5, 2).
    let over = sketch.add_point(DVec2::new(0.0, 4.0));
    let under = sketch.add_point(DVec2::new(3.0, 0.0));
    let slanting = sketch.add_segment(over, under);
    assert_eq!(sketch.crossing(first, slanting), Some(DVec2::new(1.5, 2.0)));
    // Either way round names the same place, which is the whole of what a
    // crossing is: the pair's, not the first-named edge's.
    assert_eq!(sketch.crossing(slanting, first), Some(DVec2::new(1.5, 2.0)));

    // Past both their ends and still a crossing, because it is the *lines* that
    // are asked. `across` runs (0,0)→(4,3) and this one is well off the end of
    // it — a crossing bounded to the edges would answer nothing here, and what
    // a mark about the angle between them needs is somewhere to stand.
    let far = sketch.add_point(DVec2::new(8.0, 0.0));
    let farther = sketch.add_point(DVec2::new(8.0, 2.0));
    let beyond = sketch.add_segment(far, farther);
    assert_eq!(sketch.crossing(across, beyond), Some(DVec2::new(8.0, 6.0)));

    // An edge whose ends have met has no direction of its own, so it is
    // parallel to nothing and crosses nothing — not even another that has also
    // collapsed, which is the pair that would divide by nothing at all.
    let here = sketch.add_point(DVec2::new(2.0, 2.0));
    let there = sketch.add_point(DVec2::new(2.0, 2.0));
    let collapsed = sketch.add_segment(here, there);
    let also = sketch.add_segment(there, here);
    assert!(!sketch.parallel(collapsed, first));
    assert!(!sketch.parallel(first, collapsed));
    assert!(!sketch.parallel(collapsed, also));
    assert_eq!(sketch.crossing(collapsed, first), None);
    assert_eq!(sketch.crossing(first, collapsed), None);
    assert_eq!(sketch.crossing(collapsed, also), None);
}

/// A number put somewhere comes back there, and nothing else moves when it
/// does.
///
/// The round trip is the whole of it: a drag has a place on the sketch and the
/// drawing has a placement, and what is stored has to read back as what was
/// asked for. Two statements of that frame — one to write and one to read —
/// would agree until the day one of them changed, which is why both live on
/// [`Frame`](crate::Frame).
///
/// The rest is what a placement must *not* be. It is not a measurement and not
/// a position: restating one moves no point, so a drawing already solved is
/// still solved afterwards.
#[test]
fn a_number_put_somewhere_comes_back_there_and_moves_no_geometry() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let id = sketch.add_constraint(Constraint::apart(a, b, 5.0));
    let label = |sketch: &Sketch| {
        Measurement::of(sketch, sketch.constraint(id))
            .expect("a distance is a dimension")
            .label
    };
    // On the geometry to begin with, which is where a dimension nobody has
    // dragged sits.
    assert_eq!(label(&sketch), DVec2::new(1.5, 2.0));

    // Anywhere at all, including somewhere no frame lines up with: the round
    // trip is about the arithmetic rather than about the place.
    let put = DVec2::new(-2.5, 7.25);
    sketch.place(id, put);
    assert!(
        label(&sketch).abs_diff_eq(put, 1e-12),
        "{:?}",
        label(&sketch)
    );

    // The number it states is untouched, and so is the geometry.
    assert_eq!(sketch.constraint(id).value(), Some(5.0));
    assert_eq!(sketch.point(a).position, DVec2::ZERO);
    assert_eq!(sketch.point(b).position, DVec2::new(3.0, 4.0));

    // And it is stored *relative*, which is what the frame buys: slide the
    // geometry and the number goes with it rather than staying behind.
    let slid = DVec2::new(10.0, -3.0);
    sketch.set_point(a, slid);
    sketch.set_point(b, DVec2::new(3.0, 4.0) + slid);
    assert!(
        label(&sketch).abs_diff_eq(put + slid, 1e-12),
        "{:?}",
        label(&sketch)
    );

    // And it survives being put back, which is the whole reason it lives on the
    // constraint rather than beside the sketch.
    let where_it_was = label(&sketch);
    let mut snapshot = Snapshot::default();
    sketch.snapshot_into(&mut snapshot);
    sketch.place(id, DVec2::ZERO);
    sketch.restore(&snapshot);
    // The same label, to the bit — a restore puts a sketch back rather than
    // near where it was, and a placement is part of what it puts back.
    assert_eq!(label(&sketch), where_it_was);
}

/// **A point drops onto an edge two ways, and the difference is the edge's own
/// ends.**
///
/// [`Sketch::foot_on`] answers on the line the edge *runs* along, which is what
/// [`Constraint::PointOnSegment`] states and all it states: a point held to an
/// edge slides past the end of it without the relation being any less true, so
/// whatever places one has to place it there or the placing and the residual
/// would mean different things. [`Sketch::nearest_on`] answers on the edge
/// itself, which is what anything set *beside* one wants — a mark out where the
/// line would have reached is a mark attached to nothing.
///
/// The two agree wherever the foot lands between the ends, so the case that
/// tells them apart is the one past an end, and it is checked at both.
#[test]
fn a_point_drops_onto_an_edges_line_or_onto_the_edge_itself() {
    let mut sketch = Sketch::default();
    // Four along the x axis from the origin, so the foot of anything is its own
    // x and the ends are at 0 and 4.
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(4.0, 0.0));
    let edge = sketch.add_segment(a, b);

    // Square onto the middle: both answers are the foot, and it is the point's
    // own x with the y dropped.
    let middle = DVec2::new(1.5, 9.0);
    assert_eq!(sketch.foot_on(edge, middle), Some(DVec2::new(1.5, 0.0)));
    assert_eq!(sketch.nearest_on(edge, middle), Some(DVec2::new(1.5, 0.0)));

    // Past the far end: the line runs on and the edge stops. Six along is two
    // past `b`, so one answers (6, 0) and the other (4, 0).
    let beyond = DVec2::new(6.0, 3.0);
    assert_eq!(sketch.foot_on(edge, beyond), Some(DVec2::new(6.0, 0.0)));
    assert_eq!(sketch.nearest_on(edge, beyond), Some(DVec2::new(4.0, 0.0)));

    // And past the near end, which is the same claim with the sign turned over
    // — a clamp written one-sided would pass the case above and fail this.
    let before = DVec2::new(-2.5, -1.0);
    assert_eq!(sketch.foot_on(edge, before), Some(DVec2::new(-2.5, 0.0)));
    assert_eq!(sketch.nearest_on(edge, before), Some(DVec2::new(0.0, 0.0)));

    // An edge whose ends have met has no line to drop onto and no edge to land
    // on, so neither answers — where dividing by the length it does not have
    // would answer a NaN.
    let c = sketch.add_point(DVec2::new(7.0, 7.0));
    let d = sketch.add_point(DVec2::new(7.0, 7.0));
    let collapsed = sketch.add_segment(c, d);
    assert_eq!(sketch.foot_on(collapsed, middle), None);
    assert_eq!(sketch.nearest_on(collapsed, middle), None);
}
