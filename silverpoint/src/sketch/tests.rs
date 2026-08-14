use super::*;

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
    assert_eq!(points[0], (a, DVec2::new(1.0, 2.0)));
    assert_eq!(points[1], (b, DVec2::new(3.0, 4.0)));
    assert_eq!(points[2], (c, DVec2::new(5.0, 6.0)));
    // The handle the iterator hands back is the one `is_fixed` answers
    // for — only the second point was pinned.
    let fixed: Vec<bool> = points.iter().map(|&(id, _)| sketch.is_fixed(id)).collect();
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
    sketch.write_params(&mut params);
    assert_eq!(params, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.5]);
    params[2] = 30.0;
    params[6] = 0.75;
    sketch.set_params(&params);
    assert_eq!(sketch.points().nth(1).unwrap().1, DVec2::new(30.0, 4.0));
    assert_eq!(sketch.circle(circle).radius, 0.75);

    // The same two values written one at a time, which is what a drag
    // does — and a fixed point takes a new guess like any other, since
    // being pinned is a statement to the solver and not to the caller.
    sketch.set_point(b, DVec2::new(-1.0, -2.0));
    sketch.set_radius(circle, 2.5);
    assert_eq!(sketch.point(b), DVec2::new(-1.0, -2.0));
    assert_eq!(sketch.circle(circle).radius, 2.5);
}

/// The layout, against hand-counted indices: three points fill 0..6 two
/// apiece, then one radius each at 6 and 7.
#[test]
fn every_parameter_index_names_something_and_names_it_back() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let c = sketch.add_point(DVec2::new(5.0, 6.0));
    let inner = sketch.add_circle(a, 0.5);
    let outer = sketch.add_circle(c, 1.5);
    sketch.fix(b);

    assert_eq!(sketch.param_count(), 8);
    assert_eq!(sketch.point_param(a), 0);
    assert_eq!(sketch.point_param(b), 2);
    assert_eq!(sketch.point_param(c), 4);
    assert_eq!(sketch.radius_param(inner), 6);
    assert_eq!(sketch.radius_param(outer), 7);

    // The round trip is what keeps the forward map and the reverse lookup
    // in step: break either and some index stops coming back as itself.
    for index in 0..sketch.param_count() {
        let param = sketch.param(index).expect("nothing has been removed");
        assert_eq!(sketch.param_index(param), index, "{index}");
    }
    assert_eq!(sketch.param(0), Some(Param::Point(a, Axis::X)));
    assert_eq!(sketch.param(3), Some(Param::Point(b, Axis::Y)));
    assert_eq!(sketch.param(6), Some(Param::Radius(inner)));
    assert_eq!(sketch.param(7), Some(Param::Radius(outer)));

    // Only b is pinned, so only its two coordinates are held. Radii move
    // whatever the points do.
    let free: Vec<bool> = (0..sketch.param_count())
        .map(|index| sketch.param_is_free(index))
        .collect();
    assert_eq!(free, [true, true, false, false, true, true, true, true]);
}

/// A removal leaves the vector the width it was, with a hole where the
/// point used to be — which is what keeps every surviving handle indexing
/// where it did.
#[test]
fn a_removed_points_parameters_stay_put_and_never_move() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let circle = sketch.add_circle(b, 0.5);
    assert_eq!(sketch.param_count(), 5);

    // Nothing is built on it — the circle is centred on `b` — so the cascade
    // has nothing to take and this is the bare removal.
    sketch.remove_point(a);

    assert_eq!(sketch.param_count(), 5);
    assert_eq!(sketch.param(0), None);
    assert_eq!(sketch.param(1), None);
    assert_eq!(sketch.param(2), Some(Param::Point(b, Axis::X)));
    assert_eq!(sketch.param(4), Some(Param::Radius(circle)));
    assert_eq!(sketch.point_param(b), 2);
    assert_eq!(sketch.radius_param(circle), 4);

    // The hole is unfree, which is the whole of what the solver needs: it
    // already pins a parameter it may not move and zeroes that column.
    let free: Vec<bool> = (0..5).map(|index| sketch.param_is_free(index)).collect();
    assert_eq!(free, [false, false, true, true, true]);

    // It reads zero and refuses to be written, so a step landing on it
    // changes nothing.
    let mut params = Vec::new();
    sketch.write_params(&mut params);
    assert_eq!(params, [0.0, 0.0, 3.0, 4.0, 0.5]);
    sketch.set_params(&[9.0; 5]);
    params.clear();
    sketch.write_params(&mut params);
    assert_eq!(params, [0.0, 0.0, 9.0, 9.0, 9.0]);

    // A snapshot rides the same hole: taken with the position already freed, it
    // puts back a sketch that still has the hole in it rather than one that has
    // quietly closed up around it.
    let mut over_the_hole = Snapshot::default();
    sketch.snapshot_into(&mut over_the_hole);
    sketch.set_point(b, DVec2::new(1.0, 1.0));
    sketch.restore(&over_the_hole);
    assert_eq!(sketch.point(b), DVec2::new(9.0, 9.0));
    assert_eq!(sketch.points().count(), 1);

    // The freed position is filled again rather than the vector widening,
    // and the handle to what was there is refused, not answered.
    let c = sketch.add_point(DVec2::new(5.0, 6.0));
    assert_eq!(sketch.param_count(), 5);
    assert_eq!(sketch.point_param(c), 0);
    assert_ne!(c, a);
    assert_eq!(sketch.points().count(), 2);
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
        radius: 0.5,
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
        distance: 2.0,
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
            !sketch.contains_constraint(constraint),
            "a constraint over what went survived it"
        );
    }
    // The two naming nothing that went, and no others: the count is what says
    // the sweep stopped where it should have.
    assert_eq!(sketch.constraints().count(), 2);
    assert!(sketch.contains_constraint(survivor));
    assert!(sketch.contains_constraint(spanning));

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
    assert!(!sketch.contains_constraint(on_edge));
    assert!(sketch.holds(a) && sketch.holds(b));
    assert!(sketch.contains_constraint(on_circle));

    sketch.remove_circle(circle);
    assert!(!sketch.holds(circle));
    assert!(!sketch.contains_constraint(on_circle));
    assert!(sketch.holds(a), "the centre went with its circle");

    // The constraint over two bare points outlived both, and goes only when it
    // is asked for by name — which is the one removal that cascades to nothing.
    assert_eq!(sketch.constraints().count(), 1);
    sketch.remove_constraint(over_points);
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
    assert_eq!(sketch.point(a), DVec2::new(1.0, 2.0));
    assert_eq!(sketch.point(b), DVec2::new(3.0, 4.0));
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
    assert_eq!(sketch.param_count(), 7);
    assert!(!was.fits(&sketch));
    sketch.snapshot_into(&mut now);
    assert_ne!(now, was, "an added point snapshots as if it were not there");

    sketch.restore(&was);
    assert_eq!(sketch.param_count(), 5);
    assert_eq!(sketch.points().count(), 2);
    assert_eq!(sketch.add_point(DVec2::new(6.0, 7.0)), c);

    // And put back again, which is the redo half: a snapshot of the wider
    // sketch restores into the narrower one it was taken from.
    sketch.restore(&now);
    assert_eq!(sketch.param_count(), 7);
    assert_eq!(sketch.point(c), DVec2::new(6.0, 7.0));
}
