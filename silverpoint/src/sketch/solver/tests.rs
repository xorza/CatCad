use super::*;
use crate::sketch::constraint::Constraint;
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
