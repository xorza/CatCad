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

    // Reaching past the public API on purpose: removal isn't exposed until
    // it can cascade, and this is the behaviour that has to be right first.
    sketch.points.remove(a);

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

    // A snapshot rides the same hole: it records the zero, writes the zero
    // back, and resurrects nothing. Undoing across a removal is the one thing
    // it must not attempt, and this is the half of that it can promise.
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

/// A snapshot is exact in both directions: it puts every parameter back the
/// `f64` it was, and it compares equal to one taken of the same geometry.
///
/// The comparison is the sharp half. It is what will tell an edit that changed
/// something from one that changed nothing — a drag the constraints refused, an
/// orbit that never touched the drawing — so an answer that were merely nearly
/// equal would record edits that never happened.
#[test]
fn a_snapshot_puts_every_parameter_back_and_says_whether_anything_moved() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(3.0, 4.0));
    let circle = sketch.add_circle(b, 0.5);
    sketch.fix(a);

    let mut was = Snapshot::default();
    sketch.snapshot_into(&mut was);
    assert_eq!(was.at, [1.0, 2.0, 3.0, 4.0, 0.5]);

    // A point, a radius, and a *pinned* point: between them everything a
    // snapshot holds. Being fixed is a statement to the solver, so a snapshot
    // records where the geometry is rather than what is allowed to move it.
    sketch.set_point(a, DVec2::new(-7.5, 0.25));
    sketch.set_point(b, DVec2::new(3.0, 4.5));
    sketch.set_radius(circle, 2.5);

    let mut now = Snapshot::default();
    sketch.snapshot_into(&mut now);
    assert_eq!(now.at, [-7.5, 0.25, 3.0, 4.5, 2.5]);
    assert_ne!(now, was, "a moved sketch snapshots as it stood before");

    sketch.restore(&was);
    assert_eq!(sketch.point(a), DVec2::new(1.0, 2.0));
    assert_eq!(sketch.point(b), DVec2::new(3.0, 4.0));
    assert_eq!(sketch.circle(circle).radius, 0.5);

    // Taken again into a buffer that already held one: refilled, not appended
    // to — appending would leave it twice the width, describing nothing.
    sketch.snapshot_into(&mut now);
    assert_eq!(now, was, "a restored sketch snapshots differently");

    // And it knows what it no longer describes. One more point and the sketch
    // is two parameters wider than either snapshot naming its geometry.
    sketch.add_point(DVec2::ZERO);
    assert!(!was.fits(&sketch));
}
