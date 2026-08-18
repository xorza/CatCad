//! What a removal takes with it, and what a cleanup sweeps up.

use crate::sketch::constraint::Dimension;
use crate::sketch::*;

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
    let spanning = sketch.add_constraint(Constraint::apart(before, after, 2.0));

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
    sketch.add_constraint(Constraint::apart(measured, b, 3.0));

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
