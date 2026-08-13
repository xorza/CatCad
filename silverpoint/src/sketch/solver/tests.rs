use super::*;
use crate::sketch::constraint::Constraint;
use crate::sketch::solver::freedoms::Freedom;
use glam::DVec2;

/// Tight enough that a wrong answer can't hide behind it: the solver's own
/// tolerance is on the residual, and these check the geometry that follows.
const EPSILON: f64 = 1e-9;

/// Worked by hand against a matrix whose factor is exact in binary, so the
/// assertions can be equalities rather than tolerances.
///
/// Built by choosing `L` and multiplying out, so every entry of the factor is
/// non-zero — a zero below the diagonal would let an index slip in the
/// inner product go unnoticed.
///
/// ```text
/// L = [2 0 0]     L Lᵀ = A = [ 4 2  6]
///     [1 2 0]                [ 2 5  5]
///     [3 1 2]                [ 6 5 14]
/// ```
///
/// `L₀₀ = √4 = 2`, `L₁₀ = 2/2 = 1`, `L₁₁ = √(5−1) = 2`, `L₂₀ = 6/2 = 3`,
/// `L₂₁ = (5−3·1)/2 = 1`, `L₂₂ = √(14−9−1) = 2`. With `x = [1 2 3]`, `A x` is
/// `[26 27 58]`; forward substitution gives `y = [13 7 6]` and back
/// substitution returns `x`.
#[test]
fn cholesky_factors_and_solves_a_known_system() {
    let mut a = [4.0, 2.0, 6.0, 2.0, 5.0, 5.0, 6.0, 5.0, 14.0];
    let mut b = [26.0, 27.0, 58.0];
    assert!(solve_in_place(&mut a, 3, &mut b));
    assert_eq!(b, [1.0, 2.0, 3.0]);

    // The lower triangle is the factor itself, which pins the arithmetic
    // rather than only the answer it happened to produce.
    let lower = [a[0], a[3], a[4], a[6], a[7], a[8]];
    assert_eq!(lower, [2.0, 1.0, 2.0, 3.0, 1.0, 2.0]);

    // Indefinite: `L₁₁² = 1 − 2² = −3`, so there is no real factor and the
    // caller is told to damp harder rather than handed a wrong step.
    let mut indefinite = [1.0, 2.0, 2.0, 1.0];
    assert!(!solve_in_place(&mut indefinite, 2, &mut [1.0, 1.0]));

    // Singular sits on the same boundary: `L₁₁² = 1 − 1 = 0`.
    let mut singular = [1.0, 1.0, 1.0, 1.0];
    assert!(!solve_in_place(&mut singular, 2, &mut [1.0, 1.0]));

    // A diagonal system is the case with no off-diagonal work to do at all.
    let mut diagonal = [4.0, 0.0, 0.0, 0.25];
    let mut rhs = [2.0, 1.0];
    assert!(solve_in_place(&mut diagonal, 2, &mut rhs));
    assert_eq!(rhs, [0.5, 4.0]);
}

#[test]
fn distance_moves_a_point_along_its_own_direction() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: free,
        distance: 5.0,
    });

    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    // The residual's gradient points along b - a, so a point starting on the
    // +x axis can only travel along it: (1,0) becomes exactly (5,0).
    assert!((sketch.point(free) - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(sketch.point(anchor), DVec2::ZERO, "fixed point moved");
    // One equation against two free parameters: the point may still slide
    // around the circle of radius 5.
    assert_eq!(report.degrees_of_freedom, 1);
    assert_eq!(report.redundant_equations, 0);
}

#[test]
fn the_requested_distance_is_what_changes_the_answer() {
    let solve_for = |distance: f64| {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.0));
        sketch.fix(anchor);
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance,
        });
        assert!(Solver::default().solve(&mut sketch).converged);
        sketch.point(free).x
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
        sketch.add_constraint(Constraint::Distance {
            a: first,
            b: second,
            distance,
        });
    }

    let report = Solver::default().solve(&mut sketch);
    assert!(report.converged, "{report:?}");

    let (pa, pb, pc) = (sketch.point(a), sketch.point(b), sketch.point(c));
    assert!(((pb - pa).length() - 3.0).abs() < EPSILON);
    assert!(((pc - pa).length() - 4.0).abs() < EPSILON);
    assert!(((pc - pb).length() - 5.0).abs() < EPSILON);

    // Nothing asked for a right angle — 3² + 4² = 5² produces one, so the
    // corner at `a` squaring up is evidence the solve is genuine.
    assert!((pb - pa).dot(pc - pa).abs() < 1e-8, "{pa:?} {pb:?} {pc:?}");

    // Four free parameters against three independent distances: the triangle
    // can still rotate about the fixed corner.
    assert_eq!(report.degrees_of_freedom, 1);
    assert_eq!(report.redundant_equations, 0);
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
    sketch.add_constraint(Constraint::Distance {
        a: p0,
        b: p1,
        distance: 5.0,
    });
    sketch.add_constraint(Constraint::Perpendicular {
        first: bottom,
        second: right,
    });
    sketch.add_constraint(Constraint::Distance {
        a: p1,
        b: p2,
        distance: 3.0,
    });
    sketch.add_constraint(Constraint::Parallel {
        first: bottom,
        second: top,
    });
    sketch.add_constraint(Constraint::Vertical { a: p0, b: p3 });

    let report = Solver::default().solve(&mut sketch);
    assert!(report.converged, "{report:?}");

    // Six independent equations against six free parameters pin every corner
    // of a 5x3 rectangle with its lower-left at the fixed origin.
    assert!((sketch.point(p1) - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert!((sketch.point(p2) - DVec2::new(5.0, 3.0)).length() < EPSILON);
    assert!((sketch.point(p3) - DVec2::new(0.0, 3.0)).length() < EPSILON);
    assert_eq!(report.degrees_of_freedom, 0);
    assert_eq!(report.redundant_equations, 0);
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

    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    assert!((sketch.point(free) - sketch.point(anchor)).length() < EPSILON);
    // Two free parameters against two independent equations.
    assert_eq!(report.degrees_of_freedom, 0, "{report:?}");
    assert_eq!(report.redundant_equations, 0, "{report:?}");
}

/// Holding a point pins it where the caller put it and moves the rest of the
/// sketch to suit — which is the whole of what dragging is.
///
/// Without it the solver treats the held point as free and pulls it straight
/// back toward the constraint, so a drag would slip out from under the cursor.
/// The second half of this is that failure, measured.
#[test]
fn a_held_point_stays_put_and_the_rest_of_the_sketch_follows() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let held = sketch.add_point(DVec2::new(5.0, 0.0));
    let trailing = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(anchor);
    // A chain: anchor — held — trailing, each pair five apart. Holding the
    // middle one leaves the last with somewhere to go.
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: held,
        distance: 5.0,
    });
    sketch.add_constraint(Constraint::Distance {
        a: held,
        b: trailing,
        distance: 5.0,
    });
    Solver::default().solve(&mut sketch);

    // Drag the middle point straight up. It has to stay exactly there.
    let dragged = DVec2::new(3.0, 4.0);
    sketch.set_point(held, dragged);
    let report = Solver::default().solve_holding(&mut sketch, &[held]);

    assert!(report.converged, "{report:?}");
    assert_eq!(sketch.point(held), dragged, "the held point was moved");
    // 3-4-5 again: the anchor constraint is satisfied where the caller put it,
    // which is why this drag is possible at all.
    assert!((sketch.point(held).length() - 5.0).abs() < EPSILON);
    // And the trailing point followed, staying its own five away.
    let span = sketch.point(trailing) - sketch.point(held);
    assert!((span.length() - 5.0).abs() < EPSILON, "{span:?}");

    // The same drag without holding: the solver counts the dragged point as
    // free and satisfies the distance by moving it, so it does not stay.
    sketch.set_point(held, DVec2::new(3.0, 9.0));
    let mut solver = Solver::default();
    solver.solve(&mut sketch);
    assert_ne!(
        sketch.point(held),
        DVec2::new(3.0, 9.0),
        "a free point slides back onto the constraint"
    );

    // And once more through `edit_holding`, which is the call a drag makes.
    // The 3-4-5 the other way round, so the request is reachable — and a
    // reachable one has to be kept.
    let sent = DVec2::new(-3.0, 4.0);
    let report = solver.edit_holding(&mut sketch, &[held], |sketch| sketch.set_point(held, sent));
    assert!(report.converged, "{report:?}");
    assert_eq!(sketch.point(held), sent, "a reachable edit was undone");
    let span = sketch.point(trailing) - sketch.point(held);
    assert!((span.length() - 5.0).abs() < EPSILON, "{span:?}");
}

/// Holding a point of a fully-determined sketch asks for a motion its
/// constraints forbid, and the report says so rather than pretending — and the
/// same request made as an edit is refused whole rather than half-taken.
///
/// The compromise the first half measures is exactly what an edit must never
/// keep: it satisfies nothing, and it stands up only while the point that
/// caused it is held, so the next solve that holds something else lets go of it
/// and the sketch springs back to where it always had to be.
#[test]
fn holding_a_point_a_determined_sketch_cannot_move_reports_unsolved() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let pinned = sketch.add_point(DVec2::new(5.0, 0.0));
    sketch.fix(anchor);
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: pinned,
        distance: 5.0,
    });
    sketch.add_constraint(Constraint::Horizontal {
        a: anchor,
        b: pinned,
    });
    let at_rest = Solver::default().solve(&mut sketch);
    assert!(
        at_rest.converged && at_rest.degrees_of_freedom == 0,
        "{at_rest:?}"
    );

    // Somewhere the constraints cannot reach with that point held. The anchor
    // is fixed and this one is held, so every column is zeroed and nothing can
    // move at all: the distance is satisfied exactly where it was put — 3-4-5
    // — and the horizontal is out by the whole of its four.
    sketch.set_point(pinned, DVec2::new(3.0, 4.0));
    let report = Solver::default().solve_holding(&mut sketch, &[pinned]);
    assert!(!report.converged, "{report:?}");
    assert_eq!(report.max_residual, 4.0, "{report:?}");
    assert_eq!(sketch.point(pinned), DVec2::new(3.0, 4.0), "{report:?}");

    // Back to rest, and then the same request as an edit. Nothing of it
    // survives: not the geometry, and not the report either.
    let mut solver = Solver::default();
    solver.solve(&mut sketch);
    let was: Vec<DVec2> = sketch.points().map(|(_, at)| at).collect();
    let refused = solver.edit_holding(&mut sketch, &[pinned], |sketch| {
        sketch.set_point(pinned, DVec2::new(3.0, 4.0))
    });

    let now: Vec<DVec2> = sketch.points().map(|(_, at)| at).collect();
    assert_eq!(now, was, "a refused edit moved the sketch");
    assert!(refused.converged, "{refused:?}");
    assert_eq!(refused.degrees_of_freedom, 0, "{refused:?}");
    assert_eq!(refused.iterations, 0, "a refused edit kept an iteration");
}

/// Holding nothing is solving, exactly — the general entry point cannot drift
/// from the one every caller uses.
#[test]
fn holding_nothing_is_the_same_solve() {
    let build = || {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.5));
        sketch.fix(anchor);
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance: 5.0,
        });
        sketch
    };
    let mut plain = build();
    let mut empty_hold = build();
    assert_eq!(
        Solver::default().solve(&mut plain),
        Solver::default().solve_holding(&mut empty_hold, &[]),
    );
    let moved: Vec<DVec2> = plain.points().map(|(_, at)| at).collect();
    let same: Vec<DVec2> = empty_hold.points().map(|(_, at)| at).collect();
    assert_eq!(moved, same);
}

/// A solver keeps the buffers a solve works in, so nothing one solve leaves
/// behind may be visible to the next.
///
/// Sizes are the sharp case. Solving a large sketch and then a small one
/// leaves every buffer longer than the small one needs, so a missed `clear` or
/// a length taken from the buffer instead of the sketch shows up here and
/// nowhere else — and the large one again afterwards covers the grow back.
#[test]
fn a_reused_solver_answers_exactly_as_a_fresh_one_would() {
    fn rectangle() -> Sketch {
        let mut sketch = Sketch::default();
        let corner = [
            sketch.add_point(DVec2::ZERO),
            sketch.add_point(DVec2::new(5.1, 0.2)),
            sketch.add_point(DVec2::new(4.9, 3.1)),
        ];
        sketch.fix(corner[0]);
        sketch.add_constraint(Constraint::Horizontal {
            a: corner[0],
            b: corner[1],
        });
        sketch.add_constraint(Constraint::Distance {
            a: corner[0],
            b: corner[1],
            distance: 5.0,
        });
        sketch.add_constraint(Constraint::Vertical {
            a: corner[1],
            b: corner[2],
        });
        sketch.add_constraint(Constraint::Distance {
            a: corner[1],
            b: corner[2],
            distance: 3.0,
        });
        sketch
    }
    // Four parameters against one equation, where the rectangle has six
    // against four.
    fn pair() -> Sketch {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.0));
        sketch.fix(anchor);
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance: 5.0,
        });
        sketch
    }

    // What each looks like to a solver that has never seen anything.
    let mut fresh_rectangle = rectangle();
    let rectangle_report = Solver::default().solve(&mut fresh_rectangle);
    let mut fresh_pair = pair();
    let pair_report = Solver::default().solve(&mut fresh_pair);

    // The same two through one solver, largest first.
    let mut solver = Solver::default();
    let mut large = rectangle();
    assert_eq!(solver.solve(&mut large), rectangle_report);
    let mut small = pair();
    assert_eq!(
        solver.solve(&mut small),
        pair_report,
        "a smaller sketch after a larger one"
    );
    let mut large_again = rectangle();
    assert_eq!(
        solver.solve(&mut large_again),
        rectangle_report,
        "and larger again after that"
    );

    // The same reports, and the same geometry behind them.
    for (reused, fresh) in [
        (&large, &fresh_rectangle),
        (&small, &fresh_pair),
        (&large_again, &fresh_rectangle),
    ] {
        let moved: Vec<DVec2> = reused.points().map(|(_, at)| at).collect();
        let expected: Vec<DVec2> = fresh.points().map(|(_, at)| at).collect();
        assert_eq!(moved, expected);
    }
}

#[test]
fn a_duplicate_constraint_is_reported_as_redundant() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    let distance = Constraint::Distance {
        a: anchor,
        b: free,
        distance: 5.0,
    };
    sketch.add_constraint(distance);
    sketch.add_constraint(distance);

    let report = Solver::default().solve(&mut sketch);

    // Consistent, so it still solves — but two equations share one row of
    // rank, and the extra one is reported rather than silently absorbed.
    assert!(report.converged, "{report:?}");
    assert!((sketch.point(free) - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(report.redundant_equations, 1);
    assert_eq!(report.degrees_of_freedom, 1);
}

#[test]
fn conflicting_distances_settle_at_the_least_squares_compromise() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    for distance in [1.0, 2.0] {
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance,
        });
    }

    let report = Solver::default().solve(&mut sketch);

    // Minimising (L-1)² + (L-2)² puts L at 1.5, leaving each equation half a
    // unit out. The solve reports failure rather than pretending.
    assert!(!report.converged, "{report:?}");
    assert!((sketch.point(free).length() - 1.5).abs() < 1e-8);
    assert!((report.max_residual - 0.5).abs() < 1e-8, "{report:?}");
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
        radius: 2.0,
    });
    sketch.add_constraint(Constraint::PointOnCircle { point: rim, circle });

    let start = sketch.point(rim);
    let report = Solver::default().solve(&mut sketch);
    assert!(report.converged, "{report:?}");

    assert!((sketch.circle(circle).radius - 2.0).abs() < EPSILON);
    assert!((sketch.point(rim).length() - 2.0).abs() < EPSILON);
    // The point is only ever pushed radially, so it lands on its own ray.
    assert!((sketch.point(rim).normalize() - start.normalize()).length() < EPSILON);
    // Three free parameters (the point, the radius) against two equations:
    // the point can still travel around the circle.
    assert_eq!(report.degrees_of_freedom, 1);
    assert_eq!(report.redundant_equations, 0);
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

    let report = Solver::default().solve(&mut sketch);
    assert!(report.converged, "{report:?}");

    // Both endpoints are fixed, so the line can't come to the point: the
    // point drops straight down onto y = 0, keeping its x.
    assert!((sketch.point(stray) - DVec2::new(2.0, 0.0)).length() < EPSILON);
    assert_eq!(sketch.point(b), DVec2::new(4.0, 0.0));
    assert_eq!(report.degrees_of_freedom, 1);
}

#[test]
fn an_empty_sketch_is_solved_and_fully_determined() {
    let mut sketch = Sketch::default();
    let report = Solver::default().solve(&mut sketch);
    assert_eq!(
        report,
        SolveReport {
            converged: true,
            iterations: 0,
            max_residual: 0.0,
            degrees_of_freedom: 0,
            redundant_equations: 0,
        }
    );
}

/// Which geometry the constraints pin down, and which they leave something to
/// decide.
///
/// The pair that matters is the last two, and what matters is that they agree.
/// A point riding a horizontal line and a point riding a circle are equally
/// constrained — each has one way to go — and both read `Partly`, though the
/// first keeps a coordinate still and the second changes both as it travels.
/// Counting coordinates rather than directions would separate them, and would
/// then have to call a point on a diagonal free.
#[test]
fn freedoms_name_which_geometry_the_constraints_leave_undecided() {
    let mut freedoms = Freedoms::default();
    let mut solver = Solver::default();

    // Six independent equations against six free parameters: every corner has
    // exactly one place it can be, and the anchor never had a choice.
    let mut rectangle = Sketch::default();
    let corner = [
        rectangle.add_point(DVec2::ZERO),
        rectangle.add_point(DVec2::new(5.1, 0.2)),
        rectangle.add_point(DVec2::new(4.9, 3.1)),
    ];
    rectangle.fix(corner[0]);
    rectangle.add_constraint(Constraint::Horizontal {
        a: corner[0],
        b: corner[1],
    });
    rectangle.add_constraint(Constraint::Distance {
        a: corner[0],
        b: corner[1],
        distance: 5.0,
    });
    rectangle.add_constraint(Constraint::Vertical {
        a: corner[1],
        b: corner[2],
    });
    rectangle.add_constraint(Constraint::Distance {
        a: corner[1],
        b: corner[2],
        distance: 3.0,
    });
    assert!(solver.solve(&mut rectangle).converged);
    solver.freedoms(&rectangle, &mut freedoms);
    for (index, point) in corner.iter().enumerate() {
        assert_eq!(
            freedoms.point(*point),
            Freedom::Determined,
            "corner {index} was left something to decide"
        );
    }

    // The same rectangle with its width released: two of its corners can now
    // travel, and the third still cannot.
    let mut stretchy = Sketch::default();
    let loose = [
        stretchy.add_point(DVec2::ZERO),
        stretchy.add_point(DVec2::new(5.0, 0.0)),
    ];
    stretchy.fix(loose[0]);
    stretchy.add_constraint(Constraint::Horizontal {
        a: loose[0],
        b: loose[1],
    });
    assert!(solver.solve(&mut stretchy).converged);
    solver.freedoms(&stretchy, &mut freedoms);
    assert_eq!(freedoms.point(loose[0]), Freedom::Determined, "the anchor");
    // Its y is the anchor's and its x is anyone's guess.
    assert_eq!(freedoms.point(loose[1]), Freedom::Partly);

    // The same one freedom, spent on a curve instead of a line. Both of its
    // coordinates change as it goes round, and it is no freer for that — a
    // cursor that leaves the circle is still asking for the impossible.
    let mut orbit = Sketch::default();
    let hub = orbit.add_point(DVec2::ZERO);
    let rider = orbit.add_point(DVec2::new(2.0, 0.5));
    orbit.fix(hub);
    let ring = orbit.add_circle(hub, 2.0);
    orbit.add_constraint(Constraint::Radius {
        circle: ring,
        radius: 2.0,
    });
    orbit.add_constraint(Constraint::PointOnCircle {
        point: rider,
        circle: ring,
    });
    let report = solver.solve(&mut orbit);
    assert!(report.converged, "{report:?}");
    assert_eq!(report.degrees_of_freedom, 1, "the same one freedom");
    solver.freedoms(&orbit, &mut freedoms);
    assert_eq!(freedoms.point(rider), Freedom::Partly);
    // And the radius the constraint named is decided, unlike the rider on it.
    assert_eq!(freedoms.radius(ring), Freedom::Determined);

    // A circle nothing sized keeps its rim to be dragged.
    let mut loose_ring = Sketch::default();
    let centre = loose_ring.add_point(DVec2::ZERO);
    loose_ring.fix(centre);
    let free_ring = loose_ring.add_circle(centre, 1.0);
    assert!(solver.solve(&mut loose_ring).converged);
    solver.freedoms(&loose_ring, &mut freedoms);
    assert_eq!(freedoms.radius(free_ring), Freedom::Free);
    assert_eq!(freedoms.point(centre), Freedom::Determined);
}

/// The freedoms have to agree with the count they break down, entity by entity.
///
/// Holding a point and asking again is the other way to learn the same thing:
/// pinning a coordinate that was already decided costs the sketch nothing,
/// where pinning one it was free to choose spends a degree of freedom. That is
/// a different route through the solver — the count comes from `free_params`
/// against the rank, the labels from the reduced elimination — so where the two
/// agree across sketches of every shape, both are doing what they claim.
#[test]
fn the_freedoms_agree_with_what_holding_each_point_costs() {
    let mut freedoms = Freedoms::default();
    let mut solver = Solver::default();

    for (name, mut sketch) in [
        ("a determined rectangle", determined_rectangle()),
        ("a point on its own circle", point_on_a_circle()),
        ("a duplicated constraint", duplicated_distance()),
        ("conflicting distances", conflicting_distances()),
    ] {
        solver.solve(&mut sketch);
        let at_rest = solver.edit_holding(&mut sketch, &[], |_| {});
        solver.freedoms(&sketch, &mut freedoms);

        for (id, _) in sketch.points().collect::<Vec<_>>() {
            let held = solver.edit_holding(&mut sketch, &[id], |_| {});
            let spent = at_rest.degrees_of_freedom - held.degrees_of_freedom;
            let expected = match spent {
                0 => Freedom::Determined,
                1 => Freedom::Partly,
                _ => Freedom::Free,
            };
            assert_eq!(
                freedoms.point(id),
                expected,
                "{name}: holding this point spent {spent} of the sketch's freedoms"
            );
        }
    }
}

fn determined_rectangle() -> Sketch {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(5.1, 0.2));
    sketch.fix(a);
    sketch.add_constraint(Constraint::Horizontal { a, b });
    sketch.add_constraint(Constraint::Distance {
        a,
        b,
        distance: 5.0,
    });
    sketch
}

fn point_on_a_circle() -> Sketch {
    let mut sketch = Sketch::default();
    let hub = sketch.add_point(DVec2::ZERO);
    let rider = sketch.add_point(DVec2::new(2.0, 0.5));
    sketch.fix(hub);
    let ring = sketch.add_circle(hub, 2.0);
    sketch.add_constraint(Constraint::Radius {
        circle: ring,
        radius: 2.0,
    });
    sketch.add_constraint(Constraint::PointOnCircle {
        point: rider,
        circle: ring,
    });
    sketch
}

fn duplicated_distance() -> Sketch {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    let distance = Constraint::Distance {
        a: anchor,
        b: free,
        distance: 5.0,
    };
    sketch.add_constraint(distance);
    sketch.add_constraint(distance);
    sketch
}

fn conflicting_distances() -> Sketch {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    for distance in [1.0, 2.0] {
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance,
        });
    }
    sketch
}
