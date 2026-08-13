//! The relations a sketch can impose, and the residuals they contribute.
//!
//! Every constraint is written as one or more scalar equations that read zero
//! when satisfied. The solver drives those residuals to zero, so a constraint
//! is fully described by its residual and the partial derivatives of that
//! residual with respect to the parameters it touches.

use crate::sketch::{CircleId, PointId, SegmentId, Sketch};
use glam::DVec2;

/// Below this length a direction can't be recovered from a difference of
/// points, so the derivative is taken along +x instead: any direction will do,
/// and the solver only needs somewhere to push.
const DEGENERATE: f64 = 1e-12;

/// A difference of two points, split into how far it ran and which way.
///
/// Every constraint measuring a distance needs both halves — the length is its
/// residual and the direction is its partials — and every one of them has to
/// answer for the pair being in the same place.
#[derive(Debug, Clone, Copy)]
struct Direction {
    /// Unit length, or `+x` where there was no direction left to recover.
    unit: DVec2,
    length: f64,
}

impl Direction {
    fn of(delta: DVec2) -> Self {
        let length = delta.length();
        let unit = if length < DEGENERATE {
            DVec2::X
        } else {
            delta / length
        };
        Self { unit, length }
    }
}

/// A relation between sketch entities. Distances and radii are signed values
/// in sketch units; angles are expressed structurally
/// ([`Self::Perpendicular`], [`Self::Parallel`]) rather than in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Constraint {
    /// Two points occupy the same position. Two equations — one per axis.
    Coincident { a: PointId, b: PointId },
    /// Fixed straight-line distance between two points.
    Distance {
        a: PointId,
        b: PointId,
        distance: f64,
    },
    /// The two points share a y coordinate.
    Horizontal { a: PointId, b: PointId },
    /// The two points share an x coordinate.
    Vertical { a: PointId, b: PointId },
    /// The segments point along the same line (their cross product vanishes).
    Parallel { first: SegmentId, second: SegmentId },
    /// The segments meet at a right angle (their dot product vanishes).
    Perpendicular { first: SegmentId, second: SegmentId },
    /// The point lies on the segment's infinite line — not necessarily
    /// between its endpoints.
    PointOnSegment { point: PointId, segment: SegmentId },
    /// The circle has exactly this radius.
    Radius { circle: CircleId, radius: f64 },
    /// The point lies on the circle's circumference.
    PointOnCircle { point: PointId, circle: CircleId },
}

impl Constraint {
    /// How many scalar equations this constraint contributes.
    pub fn equation_count(&self) -> usize {
        match self {
            Constraint::Coincident { .. } => 2,
            _ => 1,
        }
    }

    /// Residual of the given equation, with its partial derivatives written
    /// into `row` — one entry per sketch parameter, zeroed by the caller.
    pub(crate) fn evaluate(&self, sketch: &Sketch, equation: usize, row: &mut [f64]) -> f64 {
        debug_assert!(equation < self.equation_count());
        debug_assert_eq!(row.len(), sketch.param_count());
        match *self {
            Constraint::Coincident { a, b } => {
                let (ax, bx) = (sketch.point_param(a), sketch.point_param(b));
                row[ax + equation] = 1.0;
                row[bx + equation] = -1.0;
                let axis = |p: DVec2| if equation == 0 { p.x } else { p.y };
                axis(sketch.point(a)) - axis(sketch.point(b))
            }
            Constraint::Distance { a, b, distance } => {
                let apart = Direction::of(sketch.point(a) - sketch.point(b));
                let (ax, bx) = (sketch.point_param(a), sketch.point_param(b));
                row[ax] = apart.unit.x;
                row[ax + 1] = apart.unit.y;
                row[bx] = -apart.unit.x;
                row[bx + 1] = -apart.unit.y;
                apart.length - distance
            }
            Constraint::Horizontal { a, b } => {
                row[sketch.point_param(a) + 1] = 1.0;
                row[sketch.point_param(b) + 1] = -1.0;
                sketch.point(a).y - sketch.point(b).y
            }
            Constraint::Vertical { a, b } => {
                row[sketch.point_param(a)] = 1.0;
                row[sketch.point_param(b)] = -1.0;
                sketch.point(a).x - sketch.point(b).x
            }
            Constraint::Parallel { first, second } => {
                let (s1, s2) = (sketch.segment(first), sketch.segment(second));
                let d1 = sketch.point(s1.b) - sketch.point(s1.a);
                let d2 = sketch.point(s2.b) - sketch.point(s2.a);
                // `perp_dot(d1, d2)` is `dot(perp(d1), d2)`, so this is
                // [`Self::Perpendicular`] with one direction turned a quarter
                // circle — and each direction's partials are the *other* one
                // turned, the opposite way round, because the cross product
                // reverses when its arguments swap.
                sketch.write_segment_partials(row, s1, DVec2::new(d2.y, -d2.x));
                sketch.write_segment_partials(row, s2, DVec2::new(-d1.y, d1.x));
                d1.perp_dot(d2)
            }
            Constraint::Perpendicular { first, second } => {
                let (s1, s2) = (sketch.segment(first), sketch.segment(second));
                let d1 = sketch.point(s1.b) - sketch.point(s1.a);
                let d2 = sketch.point(s2.b) - sketch.point(s2.a);
                // The residual is `dot(d1, d2)`, whose partial in either
                // direction is simply the other direction.
                sketch.write_segment_partials(row, s1, d2);
                sketch.write_segment_partials(row, s2, d1);
                d1.dot(d2)
            }
            Constraint::PointOnSegment { point, segment } => {
                let s = sketch.segment(segment);
                let (pa, pb, p) = (sketch.point(s.a), sketch.point(s.b), sketch.point(point));
                let edge = pb - pa;
                let offset = p - pa;
                let (ia, ib, ip) = (
                    sketch.point_param(s.a),
                    sketch.point_param(s.b),
                    sketch.point_param(point),
                );
                row[ip] -= edge.y;
                row[ip + 1] += edge.x;
                row[ia] += edge.y - offset.y;
                row[ia + 1] += offset.x - edge.x;
                row[ib] += offset.y;
                row[ib + 1] -= offset.x;
                edge.perp_dot(offset)
            }
            Constraint::Radius { circle, radius } => {
                row[sketch.radius_param(circle)] = 1.0;
                sketch.circle(circle).radius - radius
            }
            Constraint::PointOnCircle { point, circle } => {
                let c = sketch.circle(circle);
                let out = Direction::of(sketch.point(point) - sketch.point(c.center));
                let (ip, ic) = (sketch.point_param(point), sketch.point_param(c.center));
                row[ip] += out.unit.x;
                row[ip + 1] += out.unit.y;
                row[ic] -= out.unit.x;
                row[ic + 1] -= out.unit.y;
                row[sketch.radius_param(circle)] -= 1.0;
                out.length - c.radius
            }
        }
    }
}

#[cfg(test)]
mod tests {
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
}
