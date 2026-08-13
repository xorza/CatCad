use super::*;
use crate::sketch::Sketch;

/// Four points in general position (no two sharing a coordinate, none
/// axis-aligned), one circle, and segments over them — so no partial
/// derivative is accidentally zero and a sign error can't hide.
#[derive(Debug)]
struct Fixture {
    sketch: Sketch,
    point: [PointId; 4],
    segment: [SegmentId; 2],
    circle: CircleId,
}

impl Fixture {
    fn new() -> Self {
        let mut sketch = Sketch::default();
        let point = [
            sketch.add_point(DVec2::new(-1.3, 0.7)),
            sketch.add_point(DVec2::new(2.1, 1.9)),
            sketch.add_point(DVec2::new(0.4, -2.2)),
            sketch.add_point(DVec2::new(3.3, -0.6)),
        ];
        let segment = [
            sketch.add_segment(point[0], point[1]),
            sketch.add_segment(point[2], point[3]),
        ];
        let circle = sketch.add_circle(point[3], 1.7);
        Self {
            sketch,
            point,
            segment,
            circle,
        }
    }
}

fn analytic(sketch: &Sketch, constraint: Constraint, equation: usize) -> Vec<f64> {
    let mut row = vec![0.0; sketch.param_count()];
    constraint.evaluate(sketch, equation, &mut row);
    row
}

/// Central differences of the residual, which the analytic partials must
/// agree with. This is the check that keeps a hand-derived Jacobian
/// honest: an error in either one shows up as a mismatch.
fn numeric(sketch: &Sketch, constraint: Constraint, equation: usize) -> Vec<f64> {
    const H: f64 = 1e-6;
    let base = sketch.params();
    let mut scratch = sketch.clone();
    let mut discard = vec![0.0; base.len()];
    let mut row = Vec::with_capacity(base.len());
    for i in 0..base.len() {
        let mut params = base.clone();
        params[i] = base[i] + H;
        scratch.set_params(&params);
        discard.fill(0.0);
        let high = constraint.evaluate(&scratch, equation, &mut discard);
        params[i] = base[i] - H;
        scratch.set_params(&params);
        discard.fill(0.0);
        let low = constraint.evaluate(&scratch, equation, &mut discard);
        row.push((high - low) / (2.0 * H));
    }
    row
}

/// The fallback the fixture below can't reach: its points are in general
/// position, so no residual there ever measures two points in one place.
#[test]
fn a_difference_too_short_to_point_anywhere_falls_back_to_x() {
    // 3-4-5, and both components of the unit vector are the correctly
    // rounded quotient, so they compare equal to the literals.
    let apart = Direction::of(DVec2::new(3.0, -4.0));
    assert_eq!(apart.length, 5.0);
    assert_eq!(apart.unit, DVec2::new(0.6, -0.8));

    // Far under the threshold, dividing would be a ratio of two roundings.
    // The solver is handed +x to push along instead, and a length that
    // still reports how far apart the two really were.
    let together = Direction::of(DVec2::new(1e-20, -1e-20));
    assert_eq!(together.unit, DVec2::X);
    assert!(together.length > 0.0 && together.length < DEGENERATE);

    // Exactly nowhere is the case that would divide by zero.
    let same = Direction::of(DVec2::ZERO);
    assert_eq!(same.unit, DVec2::X);
    assert_eq!(same.length, 0.0);
}

#[test]
fn analytic_partials_match_central_differences() {
    let Fixture {
        sketch,
        point,
        segment,
        circle,
    } = Fixture::new();
    let [p0, p1, p2, p3] = point;
    let [s0, s1] = segment;
    let cases = [
        Constraint::Coincident { a: p0, b: p2 },
        Constraint::Distance {
            a: p0,
            b: p1,
            distance: 2.5,
        },
        Constraint::Horizontal { a: p1, b: p3 },
        Constraint::Vertical { a: p0, b: p2 },
        Constraint::Parallel {
            first: s0,
            second: s1,
        },
        Constraint::Perpendicular {
            first: s0,
            second: s1,
        },
        Constraint::PointOnSegment {
            point: p2,
            segment: s0,
        },
        Constraint::Radius {
            circle,
            radius: 0.9,
        },
        Constraint::PointOnCircle { point: p0, circle },
        // Naming one entity twice, where both writes land on the same
        // parameters. Central differences don't care that the two halves
        // collide, so they say what the sum has to be: nothing for a point
        // measured against itself, nothing for a segment parallel to
        // itself, and twice the direction for one perpendicular to itself,
        // whose residual is the squared length.
        Constraint::Coincident { a: p0, b: p0 },
        Constraint::Distance {
            a: p0,
            b: p0,
            distance: 2.5,
        },
        Constraint::Horizontal { a: p1, b: p1 },
        Constraint::Vertical { a: p1, b: p1 },
        Constraint::Parallel {
            first: s0,
            second: s0,
        },
        Constraint::Perpendicular {
            first: s0,
            second: s0,
        },
        // The point is the segment's own tail, so two of the three
        // gradients meet in one place.
        Constraint::PointOnSegment {
            point: p0,
            segment: s0,
        },
        // The point is the circle's own centre.
        Constraint::PointOnCircle { point: p3, circle },
    ];
    for constraint in cases {
        for equation in 0..constraint.equation_count() {
            let a = analytic(&sketch, constraint, equation);
            let n = numeric(&sketch, constraint, equation);
            for (i, (got, want)) in a.iter().zip(&n).enumerate() {
                assert!(
                    (got - want).abs() < 1e-6,
                    "{constraint:?} eq {equation} param {i}: analytic {got} vs numeric {want}"
                );
            }
        }
    }
}

#[test]
fn residuals_read_zero_exactly_when_satisfied() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::new(1.0, 2.0));
    let b = sketch.add_point(DVec2::new(4.0, 6.0));
    let c = sketch.add_point(DVec2::new(1.0, 6.0));
    let segment = sketch.add_segment(a, b);
    let mut row = vec![0.0; sketch.param_count()];

    // 3-4-5: the distance from (1,2) to (4,6) is exactly 5.
    let distance = Constraint::Distance {
        a,
        b,
        distance: 5.0,
    };
    assert_eq!(distance.evaluate(&sketch, 0, &mut row), 0.0);
    row.fill(0.0);

    // Asking for 4 instead leaves a residual of exactly +1.
    let short = Constraint::Distance {
        a,
        b,
        distance: 4.0,
    };
    assert_eq!(short.evaluate(&sketch, 0, &mut row), 1.0);
    row.fill(0.0);

    // c shares b's y, so Horizontal is satisfied and Vertical is not:
    // c.x - b.x = 1 - 4 = -3.
    let horizontal = Constraint::Horizontal { a: c, b };
    assert_eq!(horizontal.evaluate(&sketch, 0, &mut row), 0.0);
    row.fill(0.0);
    let vertical = Constraint::Vertical { a: c, b };
    assert_eq!(vertical.evaluate(&sketch, 0, &mut row), -3.0);
    row.fill(0.0);

    // c is off the a-b line: cross((3,4), (0,4)) = 3*4 - 4*0 = 12.
    let on_line = Constraint::PointOnSegment { point: c, segment };
    assert_eq!(on_line.evaluate(&sketch, 0, &mut row), 12.0);
}
