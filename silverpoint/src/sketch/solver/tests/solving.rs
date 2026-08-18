//! What a solve lands on: where the constraints put the geometry.

use crate::sketch::constraint::{Constraint, Dimension};
use crate::sketch::solver::tests::fixtures::{Apart, EPSILON};
use crate::sketch::solver::*;
use glam::DVec2;

#[test]
fn distance_moves_a_point_along_its_own_direction() {
    let Apart {
        mut sketch,
        anchor,
        free,
        ..
    } = Apart::new();

    let outcome = sketch.solved();

    assert!(outcome.converged(), "{:?}", outcome);
    // The residual's gradient points along b - a, so a point starting on the
    // +x axis can only travel along it: (1,0) becomes exactly (5,0).
    assert!((sketch.point(free).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(
        sketch.point(anchor).position,
        DVec2::ZERO,
        "fixed point moved"
    );
    // One equation against two free parameters: the point may still slide
    // around the circle of radius 5.
    assert_eq!(outcome.degrees_of_freedom(), 1);
    assert_eq!(outcome.redundant_constraints(), 0);
}

/// A removal leaves a sketch the solver can still work on, and gives back
/// exactly the freedom it took away.
///
/// The end-to-end check on the hole a removal leaves: the parameters a removed
/// point occupied stay in the vector, and what keeps the solver off them is
/// that they read as unfree. So a solve after a removal has to reach the same
/// geometry as one before it, and the degrees of freedom have to fall by the
/// two the point was carrying rather than by nothing or by everything.
#[test]
fn a_sketch_solves_the_same_once_geometry_and_constraints_are_removed() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let end = sketch.add_point(DVec2::new(1.0, 0.2));
    // Constrained by nothing, so the two freedoms it carries are its own and
    // go when it does.
    let spare = sketch.add_point(DVec2::new(4.0, 4.0));
    sketch.fix(anchor);
    let level = sketch.add_constraint(Constraint::Horizontal { a: anchor, b: end });
    sketch.add_constraint(Constraint::apart(anchor, end, 5.0));

    let mut solver = Solver::default();
    let mut outcome = Outcome::default();

    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.converged(), "{:?}", outcome);
    assert!((sketch.point(end).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    // Six parameters, two of them the anchor's and pinned: four free against a
    // rank of two, so the spare point's pair is what is left.
    assert_eq!(outcome.degrees_of_freedom(), 2);

    sketch.remove_point(spare);
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.converged(), "{:?}", outcome);
    // The vector is as wide as it was — the hole keeps its two positions — so
    // this is the same solve reaching the same place with nothing left over.
    assert_eq!(sketch.params().count(), 6);
    assert!((sketch.point(end).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(outcome.degrees_of_freedom(), 0);
    assert_eq!(outcome.redundant_constraints(), 0);

    // Taking the level away hands `end` back the freedom it was spending: it
    // keeps its distance from the anchor and may swing about it.
    sketch.remove_constraint(level);
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.converged(), "{:?}", outcome);
    assert_eq!(outcome.degrees_of_freedom(), 1);
    assert!((sketch.point(end).position.length() - 5.0).abs() < EPSILON);
}

#[test]
fn the_requested_distance_is_what_changes_the_answer() {
    let solve_for = |distance: f64| {
        let Apart {
            mut sketch, free, ..
        } = Apart::stating(distance);
        assert!(sketch.solved().converged());
        sketch.point(free).position.x
    };
    assert!((solve_for(5.0) - 5.0).abs() < EPSILON);
    assert!((solve_for(7.0) - 7.0).abs() < EPSILON);
    assert!(solve_for(5.0) != solve_for(7.0));
}

#[test]
fn three_distances_make_a_right_triangle() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.2, 0.1));
    let c = sketch.add_point(DVec2::new(0.1, 3.9));
    sketch.fix(a);
    for (first, second, distance) in [(a, b, 3.0), (a, c, 4.0), (b, c, 5.0)] {
        sketch.add_constraint(Constraint::apart(first, second, distance));
    }

    let outcome = sketch.solved();
    assert!(outcome.converged(), "{:?}", outcome);

    let (pa, pb, pc) = (
        sketch.point(a).position,
        sketch.point(b).position,
        sketch.point(c).position,
    );
    assert!(((pb - pa).length() - 3.0).abs() < EPSILON);
    assert!(((pc - pa).length() - 4.0).abs() < EPSILON);
    assert!(((pc - pb).length() - 5.0).abs() < EPSILON);

    // Nothing asked for a right angle — 3² + 4² = 5² produces one, so the
    // corner at `a` squaring up is evidence the solve is genuine.
    assert!((pb - pa).dot(pc - pa).abs() < 1e-8, "{pa:?} {pb:?} {pc:?}");

    // Four free parameters against three independent distances: the triangle
    // can still rotate about the fixed corner.
    assert_eq!(outcome.degrees_of_freedom(), 1);
    assert_eq!(outcome.redundant_constraints(), 0);
}

#[test]
fn a_rectangle_is_fully_constrained() {
    let mut sketch = Sketch::default();
    let p0 = sketch.add_point(DVec2::ZERO);
    let p1 = sketch.add_point(DVec2::new(5.1, 0.2));
    let p2 = sketch.add_point(DVec2::new(4.9, 3.1));
    let p3 = sketch.add_point(DVec2::new(0.1, 2.9));
    sketch.fix(p0);
    let bottom = sketch.add_segment(p0, p1);
    let right = sketch.add_segment(p1, p2);
    let top = sketch.add_segment(p2, p3);

    sketch.add_constraint(Constraint::Horizontal { a: p0, b: p1 });
    sketch.add_constraint(Constraint::apart(p0, p1, 5.0));
    sketch.add_constraint(Constraint::Perpendicular {
        first: bottom,
        second: right,
    });
    sketch.add_constraint(Constraint::apart(p1, p2, 3.0));
    sketch.add_constraint(Constraint::Parallel {
        first: bottom,
        second: top,
    });
    sketch.add_constraint(Constraint::Vertical { a: p0, b: p3 });

    let outcome = sketch.solved();
    assert!(outcome.converged(), "{:?}", outcome);

    // Six independent equations against six free parameters pin every corner
    // of a 5x3 rectangle with its lower-left at the fixed origin.
    assert!((sketch.point(p1).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert!((sketch.point(p2).position - DVec2::new(5.0, 3.0)).length() < EPSILON);
    assert!((sketch.point(p3).position - DVec2::new(0.0, 3.0)).length() < EPSILON);
    assert_eq!(outcome.degrees_of_freedom(), 0);
    assert_eq!(outcome.redundant_constraints(), 0);
}

/// A coincidence pins both axes, which is the whole of what makes it worth two
/// equations rather than one.
///
/// It is the only constraint the assembler expands — into a `Vertical` and a
/// `Horizontal` — so this is where that expansion has to prove itself. The
/// report is the sharp end: an expansion that dropped an equation would leave
/// a degree of freedom, and one that emitted the same axis twice would leave a
/// degree of freedom *and* a redundancy, while the point below still slid onto
/// the anchor either way.
#[test]
fn a_coincidence_pins_both_axes_and_counts_as_two_equations() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::new(2.0, -1.0));
    // Off in both axes, so neither is satisfied to begin with.
    let free = sketch.add_point(DVec2::new(-3.5, 4.25));
    sketch.fix(anchor);
    sketch.add_constraint(Constraint::Coincident { a: anchor, b: free });

    let outcome = sketch.solved();

    assert!(outcome.converged(), "{:?}", outcome);
    assert!((sketch.point(free).position - sketch.point(anchor).position).length() < EPSILON);
    // Two free parameters against two independent equations.
    assert_eq!(outcome.degrees_of_freedom(), 0, "{:?}", outcome);
    assert_eq!(outcome.redundant_constraints(), 0, "{:?}", outcome);
}

#[test]
fn a_circle_solves_its_radius_and_the_point_on_it_together() {
    let mut sketch = Sketch::default();
    let center = sketch.add_point(DVec2::ZERO);
    let rim = sketch.add_point(DVec2::new(3.0, 0.5));
    sketch.fix(center);
    let circle = sketch.add_circle(center, 1.0);
    sketch.add_constraint(Constraint::Radius {
        circle,
        dimension: Dimension::new(2.0),
    });
    sketch.add_constraint(Constraint::PointOnCircle { point: rim, circle });

    let start = sketch.point(rim).position;
    let outcome = sketch.solved();
    assert!(outcome.converged(), "{:?}", outcome);

    assert!((sketch.circle(circle).radius - 2.0).abs() < EPSILON);
    assert!((sketch.point(rim).position.length() - 2.0).abs() < EPSILON);
    // The point is only ever pushed radially, so it lands on its own ray.
    assert!((sketch.point(rim).position.normalize() - start.normalize()).length() < EPSILON);
    // Three free parameters (the point, the radius) against two equations:
    // the point can still travel around the circle.
    assert_eq!(outcome.degrees_of_freedom(), 1);
    assert_eq!(outcome.redundant_constraints(), 0);
}

#[test]
fn point_on_segment_slides_onto_the_line_without_moving_it() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(4.0, 0.0));
    let stray = sketch.add_point(DVec2::new(2.0, 1.5));
    sketch.fix(a);
    sketch.fix(b);
    let segment = sketch.add_segment(a, b);
    sketch.add_constraint(Constraint::PointOnSegment {
        point: stray,
        segment,
    });

    let outcome = sketch.solved();
    assert!(outcome.converged(), "{:?}", outcome);

    // Both endpoints are fixed, so the line can't come to the point: the
    // point drops straight down onto y = 0, keeping its x.
    assert!((sketch.point(stray).position - DVec2::new(2.0, 0.0)).length() < EPSILON);
    assert_eq!(sketch.point(b).position, DVec2::new(4.0, 0.0));
    assert_eq!(outcome.degrees_of_freedom(), 1);
}

/// A tangency drives the circle until its rim just touches the line, and it
/// settles on whichever side it started from.
///
/// The geometry behind the residual, which is written multiplied through by the
/// segment's length and so proves nothing by reading zero. What it has to mean
/// is that the centre stands off the line by exactly the radius — measured here
/// the honest way, as a perpendicular distance, with the line held still so
/// there is one answer to check.
#[test]
fn a_tangency_stands_the_centre_off_the_line_by_the_radius() {
    // A horizontal line along y = 0, pinned, so the circle is what moves.
    let solve_from = |start: DVec2, radius: f64| {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::ZERO);
        let b = sketch.add_point(DVec2::new(4.0, 0.0));
        sketch.fix(a);
        sketch.fix(b);
        let edge = sketch.add_segment(a, b);
        let centre = sketch.add_point(start);
        let circle = sketch.add_circle(centre, radius);
        sketch.add_constraint(Constraint::Radius {
            circle,
            dimension: Dimension::new(radius),
        });
        sketch.add_constraint(Constraint::Tangent {
            segment: edge,
            circle,
        });
        let outcome = sketch.solved();
        assert!(outcome.converged(), "{outcome:?}");
        // The line is the x axis, so the perpendicular distance is |y|.
        Touching {
            centre: sketch.point(centre).position,
            radius: sketch.circle(circle).radius,
        }
    };

    // Starting above, it settles above — the radius exactly, not the negative
    // of it. The x coordinate is free to be anything and stays where it began,
    // because a tangency says nothing about where along the line it touches.
    let above = solve_from(DVec2::new(1.5, 2.6), 1.2);
    assert!((above.centre.y - 1.2).abs() < EPSILON, "{above:?}");
    assert!((above.centre.x - 1.5).abs() < EPSILON, "{above:?}");
    assert!((above.radius - 1.2).abs() < EPSILON);

    // Starting below, it settles below rather than being dragged through the
    // line to the mirror answer. That is the sign the residual takes out.
    let below = solve_from(DVec2::new(1.5, -2.6), 1.2);
    assert!((below.centre.y + 1.2).abs() < EPSILON, "{below:?}");

    // And the radius is what decides how far off: a different one is a
    // different answer, so the constraint is reading the radius rather than
    // some fixed gap.
    let nearer = solve_from(DVec2::new(1.5, 2.6), 0.4);
    assert!((nearer.centre.y - 0.4).abs() < EPSILON, "{nearer:?}");
    assert!(nearer.centre.y != above.centre.y);
}

/// Where a tangency left the circle: its centre, and how big it ended up.
#[derive(Debug)]
struct Touching {
    centre: DVec2,
    radius: f64,
}

/// Equality makes two things match without saying what either measures, which
/// is what separates it from stating a dimension on each.
#[test]
fn equality_matches_two_lengths_and_two_radii_without_fixing_either() {
    // Two edges, one dimensioned and one not. The equality carries the number
    // across, and the pair keeps the freedom a second dimension would spend.
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.0, 0.0));
    let c = sketch.add_point(DVec2::new(0.0, 4.0));
    let d = sketch.add_point(DVec2::new(0.9, 4.8));
    sketch.fix(a);
    sketch.fix(c);
    let measured = sketch.add_segment(a, b);
    let matched = sketch.add_segment(c, d);
    sketch.add_constraint(Constraint::apart(a, b, 2.5));
    sketch.add_constraint(Constraint::EqualLength {
        first: measured,
        second: matched,
    });
    let outcome = sketch.solved();
    assert!(outcome.converged(), "{outcome:?}");

    let span = |from: PointId, to: PointId| {
        (sketch.point(to).position - sketch.point(from).position).length()
    };
    assert!((span(a, b) - 2.5).abs() < EPSILON, "{}", span(a, b));
    assert!((span(c, d) - 2.5).abs() < EPSILON, "{}", span(c, d));
    // Two equations spent — the distance and the equality — against the four
    // freedoms `b` and `d` had, so the pair can still swing.
    assert_eq!(outcome.degrees_of_freedom(), 2, "{outcome:?}");

    // The same for radii, and the same freedom left: neither circle is told
    // how big, only that they agree.
    let mut sketch = Sketch::default();
    let hub = sketch.add_point(DVec2::ZERO);
    let far = sketch.add_point(DVec2::new(5.0, 0.0));
    sketch.fix(hub);
    sketch.fix(far);
    let first = sketch.add_circle(hub, 2.0);
    let second = sketch.add_circle(far, 0.75);
    sketch.add_constraint(Constraint::EqualRadius { first, second });
    let outcome = sketch.solved();
    assert!(outcome.converged(), "{outcome:?}");

    let (one, two) = (sketch.circle(first).radius, sketch.circle(second).radius);
    assert!((one - two).abs() < EPSILON, "{one} against {two}");
    // Between the two radii, one equation leaves one — so they are matched and
    // still free to grow together, which a pair of `Radius` constraints would
    // have spent.
    assert_eq!(outcome.degrees_of_freedom(), 1, "{outcome:?}");
}
