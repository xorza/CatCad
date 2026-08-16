use super::*;
use silverpoint::Plane;

/// A drawing of `sketch` on the ground, whose plane maps sketch `(x, y)` to
/// world `(x, 0, -y)` — so an anchor is read back by asking for x and −z.
fn on_ground(sketch: &Sketch) -> Drawing<'_> {
    Drawing::new(sketch, Plane::GROUND)
}

/// The anchor as the sketch sees it, which is what every rule below is
/// written in.
fn anchored(sketch: &Sketch, constraint: Constraint) -> DVec2 {
    anchor(sketch, constraint)
}

fn near(got: DVec2, want: DVec2) {
    assert!(
        got.abs_diff_eq(want, 1e-12),
        "expected {want:?}, got {got:?}"
    );
}

/// A relation that is *located* stands where its geometry meets.
///
/// The family the old rule got worst, and the arithmetic is hand-computed
/// so that a rule which merely averaged something would fail rather than
/// come out plausible. The two segments here cross at (4, 0) — a corner
/// nine units from one midpoint and four from the other — so a mark at any
/// average of middles lands somewhere the reader has no reason to look.
#[test]
fn a_relation_that_is_located_stands_where_its_geometry_meets() {
    let mut sketch = Sketch::default();
    // Along x from (4, 0) to (12, 0): middle (8, 0).
    let corner = sketch.add_point(DVec2::new(4.0, 0.0));
    let along = sketch.add_point(DVec2::new(12.0, 0.0));
    // Up y from (4, -6) to (4, 2): middle (4, -2).
    let below = sketch.add_point(DVec2::new(4.0, -6.0));
    let above = sketch.add_point(DVec2::new(4.0, 2.0));
    let flat = sketch.add_segment(corner, along);
    let upright = sketch.add_segment(below, above);
    near(
        anchored(
            &sketch,
            Constraint::Perpendicular {
                first: flat,
                second: upright,
            },
        ),
        DVec2::new(4.0, 0.0),
    );

    // A coincidence is its point, and one on a segment is that point too —
    // neither is an average of anything.
    near(
        anchored(
            &sketch,
            Constraint::Coincident {
                a: corner,
                b: below,
            },
        ),
        DVec2::new(4.0, 0.0),
    );
    near(
        anchored(
            &sketch,
            Constraint::PointOnSegment {
                point: above,
                segment: flat,
            },
        ),
        DVec2::new(4.0, 2.0),
    );

    // A tangency touches where the perpendicular from the centre lands: the
    // circle at (7, 5) drops onto the flat segment at (7, 0), which is
    // neither the centre nor the segment's middle.
    let hub = sketch.add_point(DVec2::new(7.0, 5.0));
    let ring = sketch.add_circle(hub, 5.0);
    near(
        anchored(
            &sketch,
            Constraint::Tangent {
                segment: flat,
                circle: ring,
            },
        ),
        DVec2::new(7.0, 0.0),
    );
}

/// A relation between things that need not touch stands beside one of them,
/// and a dimension stands on the span it measures.
///
/// Two families in one fixture because they are told apart by exactly one
/// thing — whether the mark belongs to a referent or to the pair — and a
/// rule that confused them would put a `∥` between two edges and a length
/// on top of one.
#[test]
fn a_relation_between_separate_things_stands_beside_one_and_a_dimension_on_its_span() {
    let mut sketch = Sketch::default();
    // From (0, 0) to (6, 0): middle (3, 0).
    let start = sketch.add_point(DVec2::ZERO);
    let end = sketch.add_point(DVec2::new(6.0, 0.0));
    let first = sketch.add_segment(start, end);
    // From (0, 10) to (6, 10): middle (3, 10), well away from the first.
    let over = sketch.add_point(DVec2::new(0.0, 10.0));
    let across = sketch.add_point(DVec2::new(6.0, 10.0));
    let second = sketch.add_segment(over, across);

    // Beside the first edge — on it — rather than at (3, 5), which is the
    // middle of the two middles and is on neither.
    for relation in [
        Constraint::Parallel { first, second },
        Constraint::EqualLength { first, second },
    ] {
        near(anchored(&sketch, relation), DVec2::new(3.0, 0.0));
    }

    // A dimension is the one family the old rule already had right, and it
    // stays right: the middle of what it spans.
    near(
        anchored(
            &sketch,
            Constraint::Distance {
                a: start,
                b: across,
                distance: 0.0,
            },
        ),
        DVec2::new(3.0, 5.0),
    );
    near(
        anchored(&sketch, Constraint::Horizontal { a: start, b: end }),
        DVec2::new(3.0, 0.0),
    );

    // A radius reads on the rim. At the centre it would read as belonging
    // to whatever else runs through the middle of the circle — which for a
    // circle drawn on a corner is every mark that corner carries.
    let hub = sketch.add_point(DVec2::new(2.0, 2.0));
    let ring = sketch.add_circle(hub, 3.0);
    near(
        anchored(
            &sketch,
            Constraint::Radius {
                circle: ring,
                radius: 3.0,
            },
        ),
        DVec2::new(5.0, 2.0),
    );

    // And two circles matched against each other face one another, so the
    // pair sits in the gap between them: the left circle's mark is on its
    // right rim and the right circle's on its left.
    let far = sketch.add_point(DVec2::new(12.0, 2.0));
    let other = sketch.add_circle(far, 1.0);
    near(
        anchored(
            &sketch,
            Constraint::EqualRadius {
                first: ring,
                second: other,
            },
        ),
        DVec2::new(5.0, 2.0),
    );
    near(
        anchored(
            &sketch,
            Constraint::EqualRadius {
                first: other,
                second: ring,
            },
        ),
        DVec2::new(11.0, 2.0),
    );
}

/// A crossing past the end of both segments is brought back onto the nearer
/// one.
///
/// Two segments that meet a long way out are still perpendicular, and a
/// mark saying so out there is attached to nothing. Both spans are asked in
/// turn, so what decides the answer is which is *nearer* rather than which
/// was named first — a rule that always took `first` would pass the half
/// below and fail the half above.
#[test]
fn a_crossing_past_both_segments_is_brought_back_onto_the_nearer_one() {
    let mut sketch = Sketch::default();
    // Along y = 0, from x = 20 to x = 30, so its nearest point to the
    // origin is its own end at (20, 0) — twenty away.
    let a = sketch.add_point(DVec2::new(20.0, 0.0));
    let b = sketch.add_point(DVec2::new(30.0, 0.0));
    let flat = sketch.add_segment(a, b);
    // Up x = 0, from y = −1 to y = 1. The infinite lines cross at the
    // origin, which is *on* this one, so this is the answer.
    let c = sketch.add_point(DVec2::new(0.0, -1.0));
    let d = sketch.add_point(DVec2::new(0.0, 1.0));
    let upright = sketch.add_segment(c, d);
    let square = Constraint::Perpendicular {
        first: flat,
        second: upright,
    };
    near(anchored(&sketch, square), DVec2::ZERO);

    // Now the other way about, with the same pair named in the same order.
    // The upright is lifted to y = 5..6, putting it five from the crossing,
    // and the flat one is brought in to x = 2..30, putting it two — so the
    // answer moves from one span to the other without either argument
    // moving.
    sketch.set_point(c, DVec2::new(0.0, 5.0));
    sketch.set_point(d, DVec2::new(0.0, 6.0));
    sketch.set_point(a, DVec2::new(2.0, 0.0));
    near(anchored(&sketch, square), DVec2::new(2.0, 0.0));
}

/// Degenerate geometry falls back rather than answering a NaN.
///
/// Every one of these is reachable, because a sketch is drawn before it is
/// solved: two segments start parallel, a segment starts as a point, two
/// circles start concentric. What must not happen is a mark at a NaN — it
/// would not be drawn *and* would not be picked, so the constraint would
/// silently stop being deletable.
#[test]
fn degenerate_geometry_falls_back_rather_than_answering_a_nan() {
    let mut sketch = Sketch::default();
    // Two parallel segments, so there is no crossing to stand in.
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(4.0, 0.0));
    let c = sketch.add_point(DVec2::new(0.0, 8.0));
    let d = sketch.add_point(DVec2::new(4.0, 8.0));
    let one = sketch.add_segment(a, b);
    let other = sketch.add_segment(c, d);
    // Between the two middles, (2, 0) and (2, 8).
    near(
        anchored(
            &sketch,
            Constraint::Perpendicular {
                first: one,
                second: other,
            },
        ),
        DVec2::new(2.0, 4.0),
    );

    // A segment with no length has no line to drop a foot onto, so a
    // tangency on it falls back to the circle's centre.
    let stuck = sketch.add_point(DVec2::new(9.0, 9.0));
    let also = sketch.add_point(DVec2::new(9.0, 9.0));
    let nothing = sketch.add_segment(stuck, also);
    let hub = sketch.add_point(DVec2::new(1.0, 2.0));
    let ring = sketch.add_circle(hub, 3.0);
    near(
        anchored(
            &sketch,
            Constraint::Tangent {
                segment: nothing,
                circle: ring,
            },
        ),
        DVec2::new(1.0, 2.0),
    );

    // Concentric circles give no bearing from one centre to the other, so
    // the mark takes +x — the same bearing a lone radius takes.
    let twin = sketch.add_circle(hub, 5.0);
    near(
        anchored(
            &sketch,
            Constraint::EqualRadius {
                first: ring,
                second: twin,
            },
        ),
        DVec2::new(4.0, 2.0),
    );
}

/// The world answer is the sketch answer put on the drawing's plane.
///
/// The one thing [`at`] does beyond [`anchor`], and worth a claim of its
/// own because it is where a plane's axes could be applied the wrong way
/// round — which would put every mark in the sketch somewhere plausible and
/// wrong.
#[test]
fn the_world_anchor_is_the_sketch_anchor_on_the_drawings_plane() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(2.0, 0.0));
    let b = sketch.add_point(DVec2::new(8.0, 4.0));
    let ground = on_ground(&sketch);
    let constraint = Constraint::Distance {
        a,
        b,
        distance: 0.0,
    };
    let sketched = anchored(&sketch, constraint);
    assert_eq!(sketched, DVec2::new(5.0, 2.0));
    assert_eq!(
        at(ground, constraint),
        ground.plane().point(sketched).as_vec3()
    );
}
