use super::*;
use silverpoint::{Along, Dimension, Plane};

/// A drawing of `sketch` on the ground, whose plane maps sketch `(x, y)` to
/// world `(x, 0, -y)` — so an anchor is read back by asking for x and −z.
fn on_ground(sketch: &Sketch) -> Drawing<'_> {
    Drawing::new(sketch, Plane::GROUND)
}

/// Where a relation drawn once stands, which every rule below but the Beside
/// family is.
fn sole(sketch: &Sketch, constraint: Constraint) -> DVec2 {
    only(sketch, constraint).at
}

/// The whole of what one such relation was given — where it stands and which
/// way it runs.
fn only(sketch: &Sketch, constraint: Constraint) -> Standing {
    let [first, second] = anchors(sketch, constraint);
    assert!(second.is_none(), "{constraint:?} is drawn more than once");
    first.expect("every constraint is drawn at least once")
}

/// Both anchors of a relation drawn against each of its referents.
fn both(sketch: &Sketch, constraint: Constraint) -> [DVec2; 2] {
    anchors(sketch, constraint).map(|it| it.expect("a relation drawn twice has two anchors").at)
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
                along: Along::Shortest,
                dimension: Dimension::new(0.0),
            },
        ),
        DVec2::new(3.0, 5.0),
    );
    near(
        sole(&sketch, Constraint::Horizontal { a: start, b: end }),
        DVec2::new(3.0, 0.0),
    );

    // The two that measure square to an edge span the perpendicular rather than
    // the geometry, so their marks sit midway along *that*. The first edge runs
    // along y = 0, so both feet drop straight down and the answers are the
    // halfway points of two vertical spans ten long.
    //
    // A standoff measures from the point it names: `over` is at (0, 10), its
    // foot is the origin, and the number goes at (0, 5). The foot is on the
    // edge's infinite line and not on the edge, which here is the same place —
    // and `across` below is where the two part.
    near(
        sole(
            &sketch,
            Constraint::Standoff {
                point: over,
                segment: first,
                dimension: Dimension::new(10.0),
            },
        ),
        DVec2::new(0.0, 5.0),
    );
    // A spacing measures from the *middle* of the second edge, at (3, 10),
    // whose foot is (3, 0) — so it lands on the middle of the gap rather than
    // over either end. A rule that took an endpoint instead would answer (0, 5).
    near(
        sole(
            &sketch,
            Constraint::Spacing {
                first,
                second,
                dimension: Dimension::new(10.0),
            },
        ),
        DVec2::new(3.0, 5.0),
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
                dimension: Dimension::new(3.0),
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

/// The world anchor is the sketch anchor put on the drawing's plane.
///
/// The one step between what the rules answer and what a projection can be
/// asked about, and worth a claim of its own because it is where a plane's axes
/// could be applied the wrong way round — which would put every mark in the
/// sketch somewhere plausible and wrong.
#[test]
fn the_world_anchor_is_the_sketch_anchor_on_the_drawings_plane() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(2.0, 0.0));
    let b = sketch.add_point(DVec2::new(8.0, 4.0));
    let of = sketch.add_constraint(Constraint::Distance {
        a,
        b,
        along: Along::Shortest,
        dimension: Dimension::new(0.0),
    });
    let ground = on_ground(&sketch);
    let placed = Placed {
        along: DVec2::X,
        of,
        at: sole(&sketch, sketch.constraint(of)),
        lane: 0,
    };
    assert_eq!(placed.at, DVec2::new(5.0, 2.0));
    // The ground plane maps sketch (x, y) to world (x, 0, −y), so a mark at
    // (5, 2) stands five along and two back — not two *up*, which is what
    // taking the plane's axes in the wrong order would give.
    assert_eq!(placed.world(ground), Vec3::new(5.0, 0.0, -2.0));
}

/// Marks wanting one place rise in a column, in the order they are held.
///
/// Two failures in one claim. Marks that share a place and are given the same
/// lane are drawn on top of each other, which is the defect the whole pass
/// exists for; marks that do *not* share a place and are given different lanes
/// float off the geometry they belong to. So the fixture has three at one
/// place, one a hair away and inside the tolerance, and one plainly elsewhere.
///
/// Fed by hand rather than through a sketch, because what this is about is the
/// pass itself: which anchor a relation gets is [`anchors`]'s and is asked
/// above.
#[test]
fn marks_wanting_one_place_rise_in_a_column_in_the_order_they_are_held() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(1.0, 0.0));
    // One real id, because `lanes` reads nothing but the coordinates — what it
    // is *about* is the caller's, and reusing one keeps that plain.
    let of = sketch.add_constraint(Constraint::Horizontal { a, b });

    // A hair inside the tolerance, which is the case this is for: a solve
    // leaves two points it made one agreeing to about this much, and a mark on
    // each has to know they are one place.
    let corner = DVec2::new(4.0, -2.0);
    let drifted = corner + DVec2::splat(SAME_PLACE * 0.4);
    let mut marks = [corner, DVec2::new(9.0, 9.0), drifted, corner].map(|at| Placed {
        of,
        at,
        along: DVec2::X,
        lane: 0,
    });
    lanes(&mut marks);

    // The three at the corner rise 0, 1, 2 in the order they were held, and the
    // one elsewhere is a stack of its own — a first lane, not a fourth.
    assert_eq!(marks.map(|placed| placed.lane), [0, 0, 1, 2]);
}

/// A place a whole sketch unit away is a different place.
///
/// The other side of the tolerance, and worth its own claim because the two
/// pull opposite ways: too tight and two marks the solver made one are drawn
/// on top of each other, too loose and marks belonging to different corners are
/// stacked into a column that points at neither.
#[test]
fn marks_a_whole_unit_apart_are_not_one_place() {
    assert!(same_place(DVec2::ZERO, DVec2::splat(SAME_PLACE * 0.5)));
    assert!(!same_place(DVec2::ZERO, DVec2::new(SAME_PLACE * 2.0, 0.0)));
    // Nothing anybody draws by hand lands this close, and everything a solve
    // converges to lands closer.
    assert!(!same_place(DVec2::ZERO, DVec2::new(0.001, 0.0)));
}

/// A dimension is set along the span it measures, and a symbol along the edge it
/// is about.
///
/// **What makes a number read as belonging to the line under it** rather than to
/// the drawing at large, which is what a draughtsman's sheet does. Hand-computed
/// off spans laid at angles no axis would give by accident: a 3-4-5 triangle, so
/// the directions come out in fifths and a mark set along the wrong one of the
/// two edges is unmistakable.
#[test]
fn a_mark_is_set_along_the_geometry_it_is_about() {
    let mut sketch = Sketch::default();
    let origin = sketch.add_point(DVec2::ZERO);
    let along = sketch.add_point(DVec2::new(4.0, 3.0));
    let across = sketch.add_point(DVec2::new(-3.0, 4.0));
    let rising = sketch.add_segment(origin, along);
    let falling = sketch.add_segment(origin, across);

    // A dimension takes the span between its own two points.
    near(
        only(
            &sketch,
            Constraint::Distance {
                a: origin,
                b: along,
                along: Along::Shortest,
                dimension: Dimension::new(5.0),
            },
        )
        .along,
        DVec2::new(0.8, 0.6),
    );
    // And the axis relations with it, which measure a line through a pair just
    // as much.
    near(
        only(
            &sketch,
            Constraint::Horizontal {
                a: origin,
                b: along,
            },
        )
        .along,
        DVec2::new(0.8, 0.6),
    );

    // A symbol about one edge runs along that edge — the other one, here, so a
    // mark that took whichever came first would read as the wrong claim.
    near(
        only(
            &sketch,
            Constraint::PointOnSegment {
                point: across,
                segment: rising,
            },
        )
        .along,
        DVec2::new(0.8, 0.6),
    );
    // Two marks, each along its own edge.
    let [first, second] = anchors(
        &sketch,
        Constraint::Parallel {
            first: rising,
            second: falling,
        },
    )
    .map(|it| it.expect("drawn against each").along);
    near(first, DVec2::new(0.8, 0.6));
    near(second, DVec2::new(-0.6, 0.8));

    // And a relation about a point alone has no span to take, so it runs the way
    // the sketch itself does.
    near(
        only(
            &sketch,
            Constraint::Coincident {
                a: origin,
                b: across,
            },
        )
        .along,
        DVec2::X,
    );
}

/// A span drawn back to front is set the same way round.
///
/// **The one thing that would otherwise follow the order a sketch happened to
/// name its points in.** A mark stands clear of its span square to the way it
/// runs, so a direction that turned over with the naming would put the dimension
/// of one segment above it and its mirror image's below — two drawings that are
/// the same drawing, read differently.
///
/// Settled in the sketch rather than against the projection, so this is asked of
/// the rule and not of a camera.
#[test]
fn a_span_drawn_back_to_front_is_set_the_same_way_round() {
    let mut sketch = Sketch::default();
    let low = sketch.add_point(DVec2::ZERO);
    let high = sketch.add_point(DVec2::new(4.0, 3.0));
    // Straight up, which is the case the tie-break decides: neither point is to
    // the right of the other.
    let over = sketch.add_point(DVec2::new(0.0, 5.0));

    let span = |a, b| {
        only(
            &sketch,
            Constraint::Distance {
                a,
                b,
                along: Along::Shortest,
                dimension: Dimension::new(1.0),
            },
        )
        .along
    };
    near(span(low, high), span(high, low));
    near(span(low, over), span(over, low));
    // And it is a direction rather than merely a consistent one: the leaning
    // span comes out in fifths, and the upright one points up.
    near(span(high, low), DVec2::new(0.8, 0.6));
    near(span(over, low), DVec2::Y);
}

/// An axis-aligned span is settled well clear of the cut.
///
/// **Where a drawing actually lives.** A solved sketch leaves residue in the
/// coordinate a span is supposed to have none of, so an upright dimension is
/// upright only to within a rounding — and a cut lying along the axes would let
/// that residue pick its direction, flickering it across as a drag wobbled the
/// sign. With the direction goes the lift square to it, so the mark would hop
/// from one side of the line it measures to the other.
///
/// Asked of a pair differing only in the sign of a residue far below anything a
/// hand or a solve puts there, which is the case that would have parted.
#[test]
fn an_upright_span_is_not_settled_by_the_residue_in_its_own_width() {
    const RESIDUE: f64 = 1e-15;
    let upright = [
        canonical(DVec2::new(RESIDUE, -4.0)),
        canonical(DVec2::new(-RESIDUE, -4.0)),
    ];
    near(upright[0], upright[1]);
    near(upright[0], DVec2::Y);

    // And the same across, where the residue is in y.
    let across = [
        canonical(DVec2::new(-4.0, RESIDUE)),
        canonical(DVec2::new(-4.0, -RESIDUE)),
    ];
    near(across[0], across[1]);
    near(across[0], DVec2::X);
}
