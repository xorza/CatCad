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
                // One axis per equation, which is also the gradient along it.
                let axis = if equation == 0 { DVec2::X } else { DVec2::Y };
                sketch.write_point_partials(row, a, axis);
                sketch.write_point_partials(row, b, -axis);
                axis.dot(sketch.point(a) - sketch.point(b))
            }
            Constraint::Distance { a, b, distance } => {
                let apart = Direction::of(sketch.point(a) - sketch.point(b));
                sketch.write_point_partials(row, a, apart.unit);
                sketch.write_point_partials(row, b, -apart.unit);
                apart.length - distance
            }
            Constraint::Horizontal { a, b } => {
                sketch.write_point_partials(row, a, DVec2::Y);
                sketch.write_point_partials(row, b, -DVec2::Y);
                sketch.point(a).y - sketch.point(b).y
            }
            Constraint::Vertical { a, b } => {
                sketch.write_point_partials(row, a, DVec2::X);
                sketch.write_point_partials(row, b, -DVec2::X);
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
                // The tail moves both `edge` and `offset`, which is why its
                // gradient carries a term from each and the other two don't.
                sketch.write_point_partials(row, point, DVec2::new(-edge.y, edge.x));
                sketch.write_point_partials(
                    row,
                    s.a,
                    DVec2::new(edge.y - offset.y, offset.x - edge.x),
                );
                sketch.write_point_partials(row, s.b, DVec2::new(offset.y, -offset.x));
                edge.perp_dot(offset)
            }
            Constraint::Radius { circle, radius } => {
                row[sketch.radius_param(circle)] += 1.0;
                sketch.circle(circle).radius - radius
            }
            Constraint::PointOnCircle { point, circle } => {
                let c = sketch.circle(circle);
                let out = Direction::of(sketch.point(point) - sketch.point(c.center));
                sketch.write_point_partials(row, point, out.unit);
                sketch.write_point_partials(row, c.center, -out.unit);
                row[sketch.radius_param(circle)] -= 1.0;
                out.length - c.radius
            }
        }
    }
}

#[cfg(test)]
mod tests;
