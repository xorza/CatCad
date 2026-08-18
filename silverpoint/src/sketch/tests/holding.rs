//! What a sketch holds, in what order, and how a snapshot puts it back.

use crate::sketch::*;

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
