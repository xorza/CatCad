//! The relations a sketch can impose, and the residuals they contribute.
//!
//! Every constraint is written as one or more scalar equations that read zero
//! when satisfied. The solver drives those residuals to zero, so a constraint
//! is fully described by its residual and the partial derivatives of that
//! residual with respect to the parameters it touches.

use crate::arena::Id;
use crate::math::approx::NO_DIRECTION;
use crate::sketch::entity::Entity;
use crate::sketch::jacobian_row::JacobianRow;
use crate::sketch::{CircleId, PointId, Segment, SegmentId, Sketch};
use glam::DVec2;

/// Handle to a constraint in a [`Sketch`].
pub type ConstraintId = Id<Constraint>;

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
        let unit = if length < NO_DIRECTION {
            DVec2::X
        } else {
            delta / length
        };
        Self { unit, length }
    }
}

/// The way a segment runs, tail to head — what every constraint reading a
/// direction is about, and the difference two of them take their partials from.
fn direction(sketch: &Sketch, segment: Segment) -> DVec2 {
    sketch.point(segment.b).position - sketch.point(segment.a).position
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
    /// The segments are the same length, whatever that length is.
    ///
    /// A relation rather than a dimension: it states that two things match
    /// without saying what either measures, so the pair is still free to grow
    /// together. Stating a [`Self::Distance`] on each would say more than was
    /// asked and leave nothing to drag.
    EqualLength { first: SegmentId, second: SegmentId },
    /// The point lies on the segment's infinite line — not necessarily
    /// between its endpoints.
    PointOnSegment { point: PointId, segment: SegmentId },
    /// The circle has exactly this radius.
    Radius { circle: CircleId, radius: f64 },
    /// The point lies on the circle's circumference.
    PointOnCircle { point: PointId, circle: CircleId },
    /// The segment's infinite line touches the circle exactly once: the
    /// centre stands off the line by the radius.
    ///
    /// Says nothing about *which side* the centre stands on. Tangency is a
    /// distance and a distance has no sign, so the relation holds mirrored
    /// either way and the solve keeps whichever side it started from — see the
    /// residual.
    Tangent {
        segment: SegmentId,
        circle: CircleId,
    },
    /// The circles are the same size, whatever that size is — [`Self::EqualLength`]
    /// for radii, and a relation rather than a dimension for the same reason.
    EqualRadius { first: CircleId, second: CircleId },
}

impl Constraint {
    /// The geometry this constraint is about.
    ///
    /// The one place that knows what each variant holds, which is what a
    /// removal cascade walks: geometry taken out of a sketch has to take the
    /// constraints naming it along, or the next solve reads a handle to
    /// something that is no longer there. A variant added to the enum above is
    /// a variant this has to answer for, and nothing else has to be taught
    /// about it.
    ///
    /// Answers in [`Entity`], which is wider than what any variant here yields:
    /// none of the twelve names another constraint, so the cascade is two levels
    /// deep and stops without being made to. That is a property of the list
    /// below rather than of the type — see [`Entity`] — and the sweep in
    /// `every_constraint_names_the_geometry_it_is_about` is what holds it.
    ///
    /// No constraint names more than two things, so the pair is built on the
    /// stack and flattened.
    pub fn referents(&self) -> impl Iterator<Item = Entity> {
        let named = match *self {
            Constraint::Coincident { a, b }
            | Constraint::Distance { a, b, .. }
            | Constraint::Horizontal { a, b }
            | Constraint::Vertical { a, b } => [Some(Entity::Point(a)), Some(Entity::Point(b))],
            Constraint::Parallel { first, second }
            | Constraint::Perpendicular { first, second }
            | Constraint::EqualLength { first, second } => {
                [Some(Entity::Segment(first)), Some(Entity::Segment(second))]
            }
            Constraint::PointOnSegment { point, segment } => {
                [Some(Entity::Point(point)), Some(Entity::Segment(segment))]
            }
            Constraint::Radius { circle, .. } => [Some(Entity::Circle(circle)), None],
            Constraint::PointOnCircle { point, circle } => {
                [Some(Entity::Point(point)), Some(Entity::Circle(circle))]
            }
            Constraint::Tangent { segment, circle } => {
                [Some(Entity::Segment(segment)), Some(Entity::Circle(circle))]
            }
            Constraint::EqualRadius { first, second } => {
                [Some(Entity::Circle(first)), Some(Entity::Circle(second))]
            }
        };
        named.into_iter().flatten()
    }

    /// The number this relation states, where it states one.
    ///
    /// A *dimension* is exactly this: a constraint carrying a magnitude, which
    /// is what a drawing shows as a number and lets a user retype. The other
    /// ten state a relation that has no magnitude — parallel is parallel, and
    /// there is nothing to type.
    ///
    /// Read off the same list `value_mut` matches on, so which variants carry
    /// a magnitude is stated once rather than in two lists free to disagree.
    pub fn value(&self) -> Option<f64> {
        let mut copy = *self;
        copy.value_mut().copied()
    }

    /// The magnitude, to restate it at something else.
    ///
    /// Kept inside `sketch` where [`Self::value`] is public, because a caller
    /// outside changes one through the sketch that holds it — see
    /// [`Sketch::set_value`](crate::Sketch::set_value). Editing a constraint in
    /// a caller's own hand would leave the sketch it came from unsolved.
    ///
    /// The one list of which variants carry a magnitude, spelled out over all
    /// twelve rather than falling through: a thirteenth carrying a value — an angle,
    /// most obviously — has to say here that it does, and one that quietly
    /// answered `None` would be a dimension the drawing showed as a symbol and
    /// refused to edit.
    pub(super) fn value_mut(&mut self) -> Option<&mut f64> {
        match self {
            Constraint::Distance { distance, .. } => Some(distance),
            Constraint::Radius { radius, .. } => Some(radius),
            Constraint::Coincident { .. }
            | Constraint::Horizontal { .. }
            | Constraint::Vertical { .. }
            | Constraint::Parallel { .. }
            | Constraint::Perpendicular { .. }
            | Constraint::EqualLength { .. }
            | Constraint::PointOnSegment { .. }
            | Constraint::PointOnCircle { .. }
            | Constraint::Tangent { .. }
            | Constraint::EqualRadius { .. } => None,
        }
    }

    /// Whether this constraint is about `entity`.
    pub(super) fn names(&self, entity: Entity) -> bool {
        self.referents().any(|referent| referent == entity)
    }

    /// The single-equation constraints this one is assembled from.
    ///
    /// Every variant but [`Self::Coincident`] is already one equation and
    /// yields itself. A coincidence is exactly a [`Self::Vertical`] and a
    /// [`Self::Horizontal`] over the same pair — the same residuals and the
    /// same partials — so it is expanded here, at the moment of assembly,
    /// rather than being carried as an equation index through everything that
    /// touches a constraint.
    ///
    /// Expanded here and nowhere earlier: the sketch still holds the
    /// coincidence the caller added, so [`Sketch::constraints`] can still say
    /// that these two equations are one relation.
    pub(super) fn equations(&self) -> impl Iterator<Item = Self> {
        let expanded = match *self {
            Constraint::Coincident { a, b } => [
                Some(Constraint::Vertical { a, b }),
                Some(Constraint::Horizontal { a, b }),
            ],
            one => [Some(one), None],
        };
        expanded.into_iter().flatten()
    }

    /// Residual of this equation, with its partial derivatives added to `row`.
    ///
    /// Two views of the same sketch, because the two halves want different
    /// things of it: `sketch` is the geometry the residual measures, and `row`
    /// is where the derivatives of that measurement go.
    ///
    /// Only ever reached through [`Self::equations`], so every arm below is a
    /// single scalar equation and none of them needs to be told which.
    pub(super) fn evaluate(&self, sketch: &Sketch, row: &mut JacobianRow<'_>) -> f64 {
        match *self {
            Constraint::Coincident { .. } => {
                unreachable!("`equations` expands a coincidence into its two axes")
            }
            Constraint::Distance { a, b, distance } => {
                let (pa, pb) = (sketch.point(a).position, sketch.point(b).position);
                let apart = Direction::of(pa - pb);
                row.point(a, apart.unit);
                row.point(b, -apart.unit);
                apart.length - distance
            }
            Constraint::Horizontal { a, b } => {
                row.point(a, DVec2::Y);
                row.point(b, -DVec2::Y);
                sketch.point(a).position.y - sketch.point(b).position.y
            }
            Constraint::Vertical { a, b } => {
                row.point(a, DVec2::X);
                row.point(b, -DVec2::X);
                sketch.point(a).position.x - sketch.point(b).position.x
            }
            Constraint::Parallel { first, second } => {
                let (s1, s2) = (sketch.segment(first), sketch.segment(second));
                let (d1, d2) = (direction(sketch, s1), direction(sketch, s2));
                // `perp_dot(d1, d2)` is `dot(perp(d1), d2)`, so this is
                // [`Self::Perpendicular`] with one direction turned a quarter
                // circle — and each direction's partials are the *other* one
                // turned, the opposite way round, because the cross product
                // reverses when its arguments swap.
                row.segment(s1, -d2.perp());
                row.segment(s2, d1.perp());
                d1.perp_dot(d2)
            }
            Constraint::Perpendicular { first, second } => {
                let (s1, s2) = (sketch.segment(first), sketch.segment(second));
                let (d1, d2) = (direction(sketch, s1), direction(sketch, s2));
                // The residual is `dot(d1, d2)`, whose partial in either
                // direction is simply the other direction.
                row.segment(s1, d2);
                row.segment(s2, d1);
                d1.dot(d2)
            }
            Constraint::EqualLength { first, second } => {
                let (s1, s2) = (sketch.segment(first), sketch.segment(second));
                let one = Direction::of(direction(sketch, s1));
                let two = Direction::of(direction(sketch, s2));
                // A length grows fastest along its own direction, so each
                // segment's partials are its own unit vector — and the second
                // enters the residual negated, so its gradient does too.
                row.segment(s1, one.unit);
                row.segment(s2, -two.unit);
                one.length - two.length
            }
            Constraint::PointOnSegment { point, segment } => {
                let s = sketch.segment(segment);
                let edge = direction(sketch, s);
                let offset = sketch.point(point).position - sketch.point(s.a).position;
                // The tail moves both `edge` and `offset`, which is why its
                // gradient carries a term from each — the difference of the
                // two, turned — and the other two don't.
                row.point(point, edge.perp());
                row.point(s.a, (offset - edge).perp());
                row.point(s.b, -offset.perp());
                edge.perp_dot(offset)
            }
            Constraint::Radius { circle, radius } => {
                row.radius(circle, 1.0);
                sketch.circle(circle).radius - radius
            }
            Constraint::PointOnCircle { point, circle } => {
                let c = sketch.circle(circle);
                let at = sketch.point(point).position;
                let out = Direction::of(at - sketch.point(c.center).position);
                row.point(point, out.unit);
                row.point(c.center, -out.unit);
                row.radius(circle, -1.0);
                out.length - c.radius
            }
            Constraint::Tangent { segment, circle } => {
                let s = sketch.segment(segment);
                let c = sketch.circle(circle);
                let edge = direction(sketch, s);
                let offset = sketch.point(c.center).position - sketch.point(s.a).position;
                let along = Direction::of(edge);
                // How far off the line the centre stands, *times* how long the
                // edge is — which is [`Self::PointOnSegment`]'s residual
                // exactly, and why the three geometry gradients below are its
                // three. Multiplied through rather than divided by the length,
                // because dividing wants a guard against a segment whose ends
                // have met and multiplying does not; the radius is scaled to
                // match, so the whole equation is one length larger and reads
                // zero in the same places.
                let reach = edge.perp_dot(offset);
                // The cross product carries which side of the line the centre
                // is on, and tangency does not care: it is a distance, and the
                // relation holds mirrored either way. Taking the sign out here
                // is what lets a solve settle onto the nearer of the two rather
                // than being dragged across the line to the other.
                let side = if reach < 0.0 { -1.0 } else { 1.0 };
                // The first term of each is [`Self::PointOnSegment`]'s own
                // gradient turned by the side; the second is the radius, which
                // grows and shrinks the equation with the segment's length.
                row.point(c.center, side * edge.perp());
                row.point(s.a, side * (offset - edge).perp() + c.radius * along.unit);
                row.point(s.b, -side * offset.perp() - c.radius * along.unit);
                row.radius(circle, -along.length);
                side * reach - c.radius * along.length
            }
            Constraint::EqualRadius { first, second } => {
                row.radius(first, 1.0);
                row.radius(second, -1.0);
                sketch.circle(first).radius - sketch.circle(second).radius
            }
        }
    }
}

#[cfg(test)]
mod tests;
