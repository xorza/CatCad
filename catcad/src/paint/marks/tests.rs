use super::*;
use silverpoint::Plane;

/// A drawing of `sketch` on the ground, whose plane maps sketch `(x, y)` to
/// world `(x, 0, -y)` — so an anchor is read back by asking for x and −z.
fn on_ground(sketch: &Sketch) -> Drawing<'_> {
    Drawing::new(sketch, Plane::GROUND)
}

/// The one anchor of a relation drawn once, which every rule below but the
/// Beside family is.
fn sole(sketch: &Sketch, constraint: Constraint) -> DVec2 {
    let [first, second] = anchors(sketch, constraint);
    assert_eq!(second, None, "{constraint:?} is drawn more than once");
    first.expect("every constraint is drawn at least once")
}

/// Both anchors of a relation drawn against each of its referents.
fn both(sketch: &Sketch, constraint: Constraint) -> [DVec2; 2] {
    anchors(sketch, constraint).map(|at| at.expect("a relation drawn twice has two anchors"))
}

fn near(got: DVec2, want: DVec2) {
    assert!(
        got.abs_diff_eq(want, 1e-12),
        "expected {want:?}, got {got:?}"
    );
}

/// A relation that is *located* stands where its geometry meets.
///
/// Hand-computed, and laid out so that averaging something would come out
/// *wrong* rather than merely arbitrary: the two segments cross at (4, 0), a
/// corner nine units from one of the midpoints and four from the other. Any
/// mark taken from the middles of what a relation names lands somewhere the
/// reader has no reason to look, which is a failure that reads as a placement
/// nobody got round to rather than as a bug.
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
        sole(
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
        sole(
            &sketch,
            Constraint::Coincident {
                a: corner,
                b: below,
            },
        ),
        DVec2::new(4.0, 0.0),
    );
    near(
        sole(
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
        sole(
            &sketch,
            Constraint::Tangent {
                segment: flat,
                circle: ring,
            },
        ),
        DVec2::new(7.0, 0.0),
    );
}

/// A relation between things that need not touch is drawn against each of
/// them, and a dimension once on the span it measures.
///
/// Two families in one fixture because they are told apart by exactly one
/// thing — whether the mark belongs to a referent or to the pair — and a rule
/// that confused them would put one `∥` between two edges and two lengths on
/// one span. The edges here are ten apart, so a mark on either is nowhere near
/// a mark that split the difference.
#[test]
fn a_relation_between_separate_things_is_drawn_against_each_and_a_dimension_once() {
    let mut sketch = Sketch::default();
    // From (0, 0) to (6, 0): middle (3, 0).
    let start = sketch.add_point(DVec2::ZERO);
    let end = sketch.add_point(DVec2::new(6.0, 0.0));
    let first = sketch.add_segment(start, end);
    // From (0, 10) to (6, 10): middle (3, 10), well away from the first.
    let over = sketch.add_point(DVec2::new(0.0, 10.0));
    let across = sketch.add_point(DVec2::new(6.0, 10.0));
    let second = sketch.add_segment(over, across);

    // One on each edge's own middle. Neither is at (3, 5), which is the middle
    // of the two middles and is on neither edge — one mark there is the
    // question this family exists to stop asking.
    for relation in [
        Constraint::Parallel { first, second },
        Constraint::EqualLength { first, second },
    ] {
        let [here, there] = both(&sketch, relation);
        near(here, DVec2::new(3.0, 0.0));
        near(there, DVec2::new(3.0, 10.0));
    }

    // A dimension stays put where it is: once, on the middle of what it spans.
    // A number belongs to the span it measures, so there is nothing to draw it
    // against twice.
    near(
        sole(
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
        sole(&sketch, Constraint::Horizontal { a: start, b: end }),
        DVec2::new(3.0, 0.0),
    );

    // A radius reads on the rim. At the centre it would read as belonging to
    // whatever else runs through the middle of the circle — which for a circle
    // drawn on a corner is every mark that corner carries.
    let hub = sketch.add_point(DVec2::new(2.0, 2.0));
    let ring = sketch.add_circle(hub, 3.0);
    near(
        sole(
            &sketch,
            Constraint::Radius {
                circle: ring,
                radius: 3.0,
            },
        ),
        DVec2::new(5.0, 2.0),
    );

    // Two circles matched against each other face one another, so the pair sits
    // in the gap between them: the left circle's mark is on its right rim and
    // the right circle's on its left. Different radii, so a rule that took one
    // circle's size for both would come out wrong on one of them.
    let far = sketch.add_point(DVec2::new(12.0, 2.0));
    let other = sketch.add_circle(far, 1.0);
    let [near_rim, far_rim] = both(
        &sketch,
        Constraint::EqualRadius {
            first: ring,
            second: other,
        },
    );
    near(near_rim, DVec2::new(5.0, 2.0));
    near(far_rim, DVec2::new(11.0, 2.0));
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
    near(sole(&sketch, square), DVec2::ZERO);

    // Now the other way about, with the same pair named in the same order.
    // The upright is lifted to y = 5..6, putting it five from the crossing,
    // and the flat one is brought in to x = 2..30, putting it two — so the
    // answer moves from one span to the other without either argument
    // moving.
    sketch.set_point(c, DVec2::new(0.0, 5.0));
    sketch.set_point(d, DVec2::new(0.0, 6.0));
    sketch.set_point(a, DVec2::new(2.0, 0.0));
    near(sole(&sketch, square), DVec2::new(2.0, 0.0));
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
        sole(
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
        sole(
            &sketch,
            Constraint::Tangent {
                segment: nothing,
                circle: ring,
            },
        ),
        DVec2::new(1.0, 2.0),
    );

    // Concentric circles give no bearing from one centre to the other, so both
    // marks take +x — the same bearing a lone radius takes. Each still on its
    // own rim, so the two do not land on one another even in the case where
    // there is nothing to tell their directions apart.
    let twin = sketch.add_circle(hub, 5.0);
    let [inner, outer] = both(
        &sketch,
        Constraint::EqualRadius {
            first: ring,
            second: twin,
        },
    );
    near(inner, DVec2::new(4.0, 2.0));
    near(outer, DVec2::new(6.0, 2.0));
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
    let sketched = sole(&sketch, constraint);
    assert_eq!(sketched, DVec2::new(5.0, 2.0));
    assert_eq!(
        at(ground, constraint),
        ground.plane().point(sketched).as_vec3()
    );
}
