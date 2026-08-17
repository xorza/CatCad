use super::*;
use crate::sketch::constraint::{Along, Dimension};

/// The iteration order is the parameter order, which is what lets a
/// handle index straight into the parameter vector.
#[test]
fn geometry_comes_back_in_insertion_order() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let c = sketch.add_point(DVec2::new(5.0, 6.0));
    sketch.fix(b);
    sketch.add_segment(a, b);
    sketch.add_segment(b, c);
    let circle = sketch.add_circle(c, 0.5);

    let points: Vec<_> = sketch.points().collect();
    assert_eq!(points.len(), 3);
    assert_eq!(points[0].1.position, DVec2::new(1.0, 2.0));
    assert_eq!(points[1].1.position, DVec2::new(3.0, 4.0));
    assert_eq!(points[2].1.position, DVec2::new(5.0, 6.0));
    assert_eq!([points[0].0, points[1].0, points[2].0], [a, b, c]);
    // The walk hands back the flag as well as the place — only the second of
    // the three was pinned.
    let fixed: Vec<bool> = points.iter().map(|&(_, point)| point.fixed).collect();
    assert_eq!(fixed, [false, true, false]);

    let segments: Vec<_> = sketch.segments().collect();
    assert_eq!(segments.len(), 2);
    assert_eq!((segments[0].1.a, segments[0].1.b), (a, b));
    assert_eq!((segments[1].1.a, segments[1].1.b), (b, c));
    // The handle each carries is the one that names it back.
    assert_eq!(sketch.segment(segments[1].0).a, b);

    let circles: Vec<_> = sketch.circles().collect();
    assert_eq!(circles.len(), 1);
    assert_eq!(circles[0].0, circle);
    assert_eq!(circles[0].1.center, c);
    assert_eq!(circles[0].1.radius, 0.5);

    // Solving rewrites positions through the same order, so the iterator
    // reports what the solver left behind rather than the initial guess.
    // Radii ride the same vector: three points fill 0..6, so the circle's
    // radius is parameter 6.
    let mut params = Vec::new();
    sketch.params().write(&mut params);
    assert_eq!(params, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.5]);
    params[2] = 30.0;
    params[6] = 0.75;
    sketch.set_params(&params);
    assert_eq!(
        sketch.points().nth(1).unwrap().1.position,
        DVec2::new(30.0, 4.0)
    );
    assert_eq!(sketch.circle(circle).radius, 0.75);

    // The same two values written one at a time, which is what a drag
    // does — and a fixed point takes a new guess like any other, since
    // being pinned is a statement to the solver and not to the caller.
    sketch.set_point(b, DVec2::new(-1.0, -2.0));
    sketch.set_radius(circle, 2.5);
    assert_eq!(sketch.point(b).position, DVec2::new(-1.0, -2.0));
    assert_eq!(sketch.circle(circle).radius, 2.5);
}

/// Removing a point takes everything that was built on it — the segments it
/// ends, the circles it centres, and every constraint naming any of those —
/// and leaves the rest of the sketch untouched.
///
/// Both halves matter equally. What is left has to be a sketch that still
/// solves, so nothing may survive holding a handle to what went; and a removal
/// that swept up more than it had to would be one nobody could predict.
#[test]
fn removing_a_point_takes_what_was_built_on_it_and_nothing_else() {
    let mut sketch = Sketch::default();
    let doomed = sketch.add_point(DVec2::new(1.0, 1.0));
    let [before, after, near, far] = [
        sketch.add_point(DVec2::new(0.0, 0.0)),
        sketch.add_point(DVec2::new(2.0, 0.0)),
        sketch.add_point(DVec2::new(0.0, 3.0)),
        sketch.add_point(DVec2::new(2.0, 3.0)),
    ];
    // Two edges meeting at the doomed point, one from either end, so the walk
    // has to look at both of a segment's endpoints and not just the first.
    let leading = sketch.add_segment(before, doomed);
    let trailing = sketch.add_segment(doomed, after);
    let aside = sketch.add_segment(near, far);
    let hole = sketch.add_circle(doomed, 0.5);
    let elsewhere = sketch.add_circle(near, 0.25);

    let by_point = sketch.add_constraint(Constraint::Horizontal {
        a: before,
        b: doomed,
    });
    let by_segment = sketch.add_constraint(Constraint::Parallel {
        first: leading,
        second: aside,
    });
    let by_circle = sketch.add_constraint(Constraint::Radius {
        circle: hole,
        dimension: Dimension::new(0.5),
    });
    // Named by both routes at once — the point going and the segment going —
    // so the cascade reaches it twice and has to take that as calmly as once.
    let twice_over = sketch.add_constraint(Constraint::PointOnSegment {
        point: doomed,
        segment: trailing,
    });
    let survivor = sketch.add_constraint(Constraint::Vertical { a: near, b: far });
    let spanning = sketch.add_constraint(Constraint::Distance {
        a: before,
        b: after,
        along: Along::Shortest,
        dimension: Dimension::new(2.0),
    });

    sketch.remove_point(doomed);

    assert!(!sketch.holds(doomed));
    for point in [before, after, near, far] {
        assert!(sketch.holds(point), "a bystanding point went");
    }

    assert!(!sketch.holds(leading));
    assert!(!sketch.holds(trailing));
    assert!(sketch.holds(aside));

    assert!(!sketch.holds(hole));
    assert!(sketch.holds(elsewhere));

    for constraint in [by_point, by_segment, by_circle, twice_over] {
        assert!(
            !sketch.holds(constraint),
            "a constraint over what went survived it"
        );
    }
    // The two naming nothing that went, and no others: the count is what says
    // the sweep stopped where it should have.
    assert_eq!(sketch.constraints().count(), 2);
    assert!(sketch.holds(survivor));
    assert!(sketch.holds(spanning));

    // Idempotent, which is what lets the cascade reach one thing by two routes:
    // asking again for what has already gone changes nothing.
    sketch.remove_point(doomed);
    sketch.remove_segment(leading);
    sketch.remove_circle(hole);
    assert_eq!(sketch.points().count(), 4);
    assert_eq!(sketch.segments().count(), 1);
    assert_eq!(sketch.circles().count(), 1);
    assert_eq!(sketch.constraints().count(), 2);
}

/// An edge and a circle are drawn *over* points rather than owning them, so
/// removing either leaves the points behind — and takes only the constraints
/// that named the thing removed.
#[test]
fn removing_an_edge_or_a_circle_leaves_the_points_it_was_drawn_over() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.0, 0.0));
    let edge = sketch.add_segment(a, b);
    let circle = sketch.add_circle(a, 1.0);
    let on_edge = sketch.add_constraint(Constraint::PointOnSegment {
        point: b,
        segment: edge,
    });
    let on_circle = sketch.add_constraint(Constraint::PointOnCircle { point: b, circle });
    let over_points = sketch.add_constraint(Constraint::Horizontal { a, b });

    sketch.remove_segment(edge);
    assert!(!sketch.holds(edge));
    assert!(!sketch.holds(on_edge));
    assert!(sketch.holds(a) && sketch.holds(b));
    assert!(sketch.holds(on_circle));

    sketch.remove_circle(circle);
    assert!(!sketch.holds(circle));
    assert!(!sketch.holds(on_circle));
    assert!(sketch.holds(a), "the centre went with its circle");

    // The constraint over two bare points outlived both, and goes only when it
    // is asked for by name — which is the one removal that cascades to nothing.
    assert_eq!(sketch.constraints().count(), 1);
    assert!(
        sketch.holds(over_points),
        "a constraint is a thing the sketch holds, like the geometry it is about"
    );
    sketch.remove_constraint(over_points);
    assert!(!sketch.holds(over_points));
    assert_eq!(sketch.constraints().count(), 0);
    assert!(sketch.holds(a) && sketch.holds(b));
}

/// A constraint over geometry the sketch no longer holds is the caller's
/// mistake, caught where it is made rather than deep inside the next solve —
/// which is where the handle would otherwise be read.
#[test]
#[should_panic = "a constraint needs geometry the sketch still holds"]
fn a_constraint_over_geometry_that_has_gone_is_refused() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.remove_point(b);
    sketch.add_constraint(Constraint::Horizontal { a, b });
}

/// A snapshot is exact in both directions: it puts the sketch back exactly as
/// it stood, and it compares equal to one taken of the same sketch.
///
/// The comparison is the sharp half. It is what will tell an edit that changed
/// something from one that changed nothing — a drag the constraints refused, an
/// orbit that never touched the drawing — so an answer that were merely nearly
/// equal would record edits that never happened.
#[test]
fn a_snapshot_puts_a_sketch_back_and_says_whether_anything_changed() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let circle = sketch.add_circle(b, 0.5);
    sketch.fix(a);

    let mut was = Snapshot::default();
    sketch.snapshot_into(&mut was);

    // A point, a radius, and a *pinned* point: between them everything moving
    // the geometry can reach. Being fixed is a statement to the solver rather
    // than a position, and a pinned point still travels when it is set.
    sketch.set_point(a, DVec2::new(-7.5, 0.25));
    sketch.set_point(b, DVec2::new(3.0, 4.5));
    sketch.set_radius(circle, 2.5);

    let mut now = Snapshot::default();
    sketch.snapshot_into(&mut now);
    assert_ne!(now, was, "a moved sketch snapshots as it stood before");

    sketch.restore(&was);
    assert_eq!(sketch.point(a).position, DVec2::new(1.0, 2.0));
    assert_eq!(sketch.point(b).position, DVec2::new(3.0, 4.0));
    assert_eq!(sketch.circle(circle).radius, 0.5);

    // Taken again into a buffer that already held one: refilled, not added to.
    sketch.snapshot_into(&mut now);
    assert_eq!(now, was, "a restored sketch snapshots differently");

    // Adding geometry is a change like any other, and one a snapshot can put
    // back — which is what a history needs to take a creation back. The sketch
    // is two parameters wider while the point is there and exactly as wide as
    // it was once it has gone, down to the handle the next point is minted
    // with: `c` is added twice and comes out the same both times.
    let c = sketch.add_point(DVec2::new(6.0, 7.0));
    assert_eq!(sketch.params().count(), 7);
    sketch.snapshot_into(&mut now);
    assert_ne!(now, was, "an added point snapshots as if it were not there");

    sketch.restore(&was);
    assert_eq!(sketch.params().count(), 5);
    assert_eq!(sketch.points().count(), 2);
    assert_eq!(sketch.add_point(DVec2::new(6.0, 7.0)), c);

    // And put back again, which is the redo half: a snapshot of the wider
    // sketch restores into the narrower one it was taken from.
    sketch.restore(&now);
    assert_eq!(sketch.params().count(), 7);
    assert_eq!(sketch.point(c).position, DVec2::new(6.0, 7.0));

    // A hole rides a snapshot like anything else: a position freed before one
    // is taken is still free after it is put back, rather than quietly closing
    // up around what went.
    sketch.remove_point(a);
    sketch.snapshot_into(&mut now);
    sketch.set_point(b, DVec2::ZERO);
    sketch.restore(&now);
    assert_eq!(sketch.point(b).position, DVec2::new(3.0, 4.0));
    assert_eq!(sketch.points().count(), 2);
    assert_eq!(sketch.params().count(), 7);

    // One taken out and another put in its place. Every tally the sketch keeps
    // is unchanged — the arena hands the freed position straight back — and it
    // is not the sketch that was recorded: the replacement carries a later
    // generation, which is the only thing that says so.
    sketch.remove_point(c);
    let replacement = sketch.add_point(DVec2::new(6.0, 7.0));
    assert_eq!(sketch.params().count(), 7, "the tally is unchanged");
    assert_eq!(sketch.points().count(), 2, "and so is the count");
    assert_ne!(replacement, c);
    // And the record says so. Every tally agrees, so the generation the
    // replacement carries is the only thing that can tell them apart — and a
    // snapshot carries it.
    let mut after = Snapshot::default();
    sketch.snapshot_into(&mut after);
    assert_ne!(after, now, "a replaced point read as the one it replaced");
}

/// A cleanup takes out geometry that duplicates something and carries nothing,
/// and leaves everything else exactly where it was.
///
/// Both halves of the rule get a case apiece here, because either one alone
/// would be wrong in a way the other hides: a sweep that only checked for
/// duplicates would eat the corner of a polyline, and one that only checked
/// for references would never remove anything at all.
#[test]
fn a_cleanup_removes_spare_geometry_and_keeps_everything_depended_on() {
    let mut sketch = Sketch::default();

    // The drawing proper: an edge, and a circle on its far end.
    let a = sketch.add_point(DVec2::new(0.0, 0.0));
    let b = sketch.add_point(DVec2::new(3.0, 0.0));
    sketch.add_segment(a, b);
    let ring = sketch.add_circle(b, 1.0);

    // A spare marker exactly on `a`, tied to nothing.
    let stray = sketch.add_point(DVec2::new(0.0, 0.0));
    // Another on `b`, tied to it the way a join is — which counts as being in
    // the same place however the coordinates read, so this one is put slightly
    // off to prove the coincidence is what carries it.
    let joined = sketch.add_point(DVec2::new(3.0 + 1e-6, 0.0));
    sketch.add_constraint(Constraint::Coincident { a: joined, b });
    // And one on `a` that a *dimension* is written against. Duplicated like the
    // first, but something is said about it, so it stays.
    let measured = sketch.add_point(DVec2::new(0.0, 0.0));
    sketch.add_constraint(Constraint::Distance {
        a: measured,
        b,
        along: Along::Shortest,
        dimension: Dimension::new(3.0),
    });

    // A second edge drawn over the first through its own points, and a third
    // written the other way round — both duplicates of it.
    let a2 = sketch.add_point(DVec2::new(0.0, 0.0));
    let b2 = sketch.add_point(DVec2::new(3.0, 0.0));
    let over = sketch.add_segment(a2, b2);
    let backwards = sketch.add_segment(b2, a2);
    // And a duplicate circle, same centre and same radius.
    let twin = sketch.add_circle(b2, 1.0);

    let removed = sketch.remove_duplicates();

    // Both spare edges go, and the circle on top of the other one.
    assert_eq!(removed.segments, 2, "{removed:?}");
    assert_eq!(removed.circles, 1, "{removed:?}");
    assert!(
        !sketch.holds(over) && !sketch.holds(backwards),
        "one survived"
    );
    assert!(!sketch.holds(twin));
    assert!(sketch.segments().count() == 1 && sketch.circles().count() == 1);
    assert!(sketch.holds(ring), "the circle nothing duplicated went");

    // `stray` and `joined` go; `a2`/`b2` go too, freed by the edges above them
    // being taken out in the same pass. `measured` stays — a distance is said
    // about it — and so do `a` and `b`, which the surviving edge names.
    assert_eq!(removed.points, 4, "{removed:?}");
    assert!(!sketch.holds(stray), "a spare marker on a point survived");
    assert!(
        !sketch.holds(joined),
        "a point tied only by a join survived"
    );
    assert!(
        !sketch.holds(a2) && !sketch.holds(b2),
        "an orphaned end stayed"
    );
    assert!(sketch.holds(measured), "a dimensioned point was removed");
    assert!(sketch.holds(a) && sketch.holds(b));

    // The coincidence went with the point it was about; the distance did not.
    assert_eq!(sketch.constraints().count(), 1);
    assert!(matches!(
        sketch.constraints().next().expect("the distance stays").1,
        Constraint::Distance { .. }
    ));

    // And nothing moved. A cleanup deletes; it never repositions.
    assert_eq!(sketch.point(a).position, DVec2::new(0.0, 0.0));
    assert_eq!(sketch.point(b).position, DVec2::new(3.0, 0.0));
    assert_eq!(sketch.circle(ring).radius, 1.0);

    // Run again and it finds nothing: what is left duplicates nothing, which
    // is what makes the command safe to press twice.
    assert!(sketch.remove_duplicates().is_empty());
}

/// Two identical spares leave one behind rather than taking each other out.
///
/// The trap in phrasing the rule as "remove anything that duplicates
/// something": each of a pair duplicates the other, so both qualify and the
/// geometry vanishes. What actually happens is that the first becomes a keeper
/// and the rest measure against it.
#[test]
fn a_pile_of_identical_spares_leaves_exactly_one() {
    let mut sketch = Sketch::default();
    for _ in 0..4 {
        sketch.add_point(DVec2::new(1.0, 1.0));
    }
    // One somewhere else, so the survivor is not merely "the only point left".
    let elsewhere = sketch.add_point(DVec2::new(9.0, 9.0));

    let removed = sketch.remove_duplicates();
    assert_eq!(removed.points, 3, "{removed:?}");
    assert_eq!(sketch.points().count(), 2);
    assert!(sketch.holds(elsewhere));
    assert_eq!(
        sketch
            .points()
            .filter(|(_, point)| point.position == DVec2::new(1.0, 1.0))
            .count(),
        1,
        "the pile did not collapse to one"
    );
}

/// Nearness is a disc of `TOUCHING`, and a duplicate that something is
/// said about is kept whichever of the pair it is.
#[test]
fn the_cleanup_measures_nearness_and_spares_what_is_spoken_for() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    // Just inside the tolerance, and just outside it.
    let near = sketch.add_point(DVec2::new(TOUCHING * 0.5, 0.0));
    let far = sketch.add_point(DVec2::new(TOUCHING * 2.0, 0.0));

    let removed = sketch.remove_duplicates();
    assert_eq!(removed.points, 1, "{removed:?}");
    assert!(!sketch.holds(near), "a point within the tolerance stayed");
    assert!(sketch.holds(far), "a point outside it was removed");
    assert!(sketch.holds(anchor));

    // A duplicate segment that a relation names is kept, and the plain one
    // beside it still goes — so the reference is what saved it, not its order.
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(1.0, 0.0));
    let kept = sketch.add_segment(a, b);
    let spoken_for = sketch.add_segment(a, b);
    let plain = sketch.add_segment(a, b);
    sketch.add_constraint(Constraint::Parallel {
        first: spoken_for,
        second: kept,
    });

    let removed = sketch.remove_duplicates();
    assert_eq!(removed.segments, 1, "{removed:?}");
    assert!(!sketch.holds(plain), "the unspoken-for duplicate stayed");
    assert!(
        sketch.holds(spoken_for) && sketch.holds(kept),
        "a duplicate a relation names was removed, taking the relation with it"
    );
    assert_eq!(sketch.constraints().count(), 1, "the parallel was dropped");
}

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
    let at = |sketch: &mut Sketch, x, y| sketch.add_point(DVec2::new(x, y));
    let (a, b) = (at(&mut sketch, 0.0, 0.0), at(&mut sketch, 3.0, 4.0));
    let (c, d) = (at(&mut sketch, 1.0, 0.0), at(&mut sketch, 4.0, 4.0));
    let (e, f) = (at(&mut sketch, 0.0, 0.0), at(&mut sketch, 4.0, 3.0));
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
    let (over, under) = (at(&mut sketch, 0.0, 4.0), at(&mut sketch, 3.0, 0.0));
    let slanting = sketch.add_segment(over, under);
    assert_eq!(sketch.crossing(first, slanting), Some(DVec2::new(1.5, 2.0)));
    // Either way round names the same place, which is the whole of what a
    // crossing is: the pair's, not the first-named edge's.
    assert_eq!(sketch.crossing(slanting, first), Some(DVec2::new(1.5, 2.0)));

    // Past both their ends and still a crossing, because it is the *lines* that
    // are asked. `across` runs (0,0)→(4,3) and this one is well off the end of
    // it — a crossing bounded to the edges would answer nothing here, and what
    // a mark about the angle between them needs is somewhere to stand.
    let (far, farther) = (at(&mut sketch, 8.0, 0.0), at(&mut sketch, 8.0, 2.0));
    let beyond = sketch.add_segment(far, farther);
    assert_eq!(sketch.crossing(across, beyond), Some(DVec2::new(8.0, 6.0)));

    // An edge whose ends have met has no direction of its own, so it is
    // parallel to nothing and crosses nothing — not even another that has also
    // collapsed, which is the pair that would divide by nothing at all.
    let (here, there) = (at(&mut sketch, 2.0, 2.0), at(&mut sketch, 2.0, 2.0));
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
    let id = sketch.add_constraint(Constraint::Distance {
        a,
        b,
        along: Along::Shortest,
        dimension: Dimension::new(5.0),
    });
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
    let at = |sketch: &mut Sketch, x, y| sketch.add_point(DVec2::new(x, y));
    // Four along the x axis from the origin, so the foot of anything is its own
    // x and the ends are at 0 and 4.
    let (a, b) = (at(&mut sketch, 0.0, 0.0), at(&mut sketch, 4.0, 0.0));
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
    let (c, d) = (at(&mut sketch, 7.0, 7.0), at(&mut sketch, 7.0, 7.0));
    let collapsed = sketch.add_segment(c, d);
    assert_eq!(sketch.foot_on(collapsed, middle), None);
    assert_eq!(sketch.nearest_on(collapsed, middle), None);
}
