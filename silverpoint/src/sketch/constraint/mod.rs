//! The relations a sketch can impose, and the residuals they contribute.
//!
//! Every constraint is written as one or more scalar equations that read zero
//! when satisfied. The solver drives those residuals to zero, so a constraint
//! is fully described by its residual and the partial derivatives of that
//! residual with respect to the parameters it touches.

use crate::arena::Id;
use crate::math::direction::Direction;
use crate::sketch::entity::Entity;
use crate::sketch::jacobian_row::JacobianRow;
use crate::sketch::{CircleId, PointId, Segment, SegmentId, Sketch};
use glam::DVec2;

/// Handle to a constraint in a [`Sketch`].
pub type ConstraintId = Id<Constraint>;

/// What a dimension states, and where its number sits.
///
/// The two halves of a dimension that are not the geometry it is about: what it
/// measures to, which the solver drives onto, and where the drawing puts the
/// figure, which the solver never reads.
///
/// The placement is here rather than beside the sketch because it is content: a
/// number dragged clear of the geometry stays clear of it across a save, an
/// undo and every solve in between, and the sketch is the one thing that
/// already promises all three — [`Snapshot`](crate::Snapshot) clones the arenas,
/// a restore puts back the generations, the file mirrors every relation, and a
/// removal takes the dimension along with what it was about. What is *not* here
/// is anything about appearance: what colour, what size, what face it is set in
/// all stay with whoever draws it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Dimension {
    pub value: f64,
    /// Where the number sits, read in the measurement's own frame: `+x` along
    /// the dimension line from the middle of what is measured, `+y` across it.
    ///
    /// Relative rather than a place on the sketch, so a label follows its
    /// geometry when that geometry is dragged, turned, or resolved somewhere
    /// else entirely — a dimension left behind by what it measures is not a
    /// dimension.
    ///
    /// A radius is the one exception and states its own frame: a circle has no
    /// orientation to be relative to, so its placement is an offset from the
    /// centre in the sketch's own coordinates, and the leader runs out through
    /// it. See [`Measurement`](crate::Measurement), which is where either
    /// reading becomes a position.
    pub placement: DVec2,
}

impl Dimension {
    /// A dimension stating `value`, with its number wherever the geometry puts
    /// it.
    ///
    /// Where a dimension starts before anyone has dragged it, and what a caller
    /// with no opinion about placement asks for: zero is the middle of what is
    /// measured, so the number lands on the geometry and whoever draws it is
    /// free to stand it off. Not a [`Default`], because the value is the whole
    /// point and there is no sensible one to leave out.
    pub fn new(value: f64) -> Self {
        Self {
            value,
            placement: DVec2::ZERO,
        }
    }
}

/// Which way a distance between two points is read.
///
/// The three readings a drawing offers for one pair, and the whole of what
/// tells them apart is which components of the difference survive the
/// projection below — so all three are one residual rather than three.
///
/// Named for the drawing's words, and the same words the relations beside them
/// wear: [`Constraint::Horizontal`] states that a pair shares a y, and
/// [`Along::Horizontal`] measures the x between them. Both are about the
/// sketch's own axes, and where those point in the world is the
/// [`Plane`](crate::Plane)'s to say.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Along {
    /// Straight between the two, whichever way that runs.
    Shortest,
    /// Along the sketch's x, so the reading is how far apart they are across.
    Horizontal,
    /// Along its y.
    Vertical,
}

impl Along {
    /// What is left of a difference once it is read this way.
    ///
    /// The one place the three differ. A projected difference is still a
    /// difference, so the residual and the partials that follow are written
    /// once for all of them.
    fn project(self, delta: DVec2) -> DVec2 {
        match self {
            Along::Shortest => delta,
            Along::Horizontal => DVec2::new(delta.x, 0.0),
            Along::Vertical => DVec2::new(0.0, delta.y),
        }
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
    /// Fixed distance between two points, measured the way `along` says.
    Distance {
        a: PointId,
        b: PointId,
        along: Along,
        dimension: Dimension,
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
    /// The point stands this far off the segment's infinite line, square to it.
    ///
    /// [`Self::PointOnSegment`] with a number in place of the zero, and the
    /// same residual as [`Self::Tangent`] with a constant where that one has a
    /// radius. Says nothing about *which side* the point stands on, for
    /// tangency's reason: a distance has no sign, so the relation holds
    /// mirrored either way and the solve keeps whichever side it started from.
    Standoff {
        point: PointId,
        segment: SegmentId,
        dimension: Dimension,
    },
    /// The two edges stand this far apart, square to the first.
    ///
    /// The standoff of `second`'s middle from `first`'s infinite line, which
    /// for a parallel pair *is* the distance between the two lines. Only
    /// offered for a parallel pair, because that is the only arrangement in
    /// which a distance between two lines is one number at all — see
    /// [`Sketch::parallel`].
    ///
    /// It does not state parallelism itself, deliberately. The natural order is
    /// to make two edges parallel and then dimension the gap, and a relation
    /// that restated the parallel would report the sketch over-constrained for
    /// having been used the way it reads.
    ///
    /// Its own variant rather than a [`Self::Standoff`] on an endpoint borrowed
    /// from `second`, because it names both edges: removing a segment keeps its
    /// endpoints, so the borrowed form would leave a dimension alive on a vertex
    /// nothing draws, measuring a gap to an edge that has gone.
    Spacing {
        first: SegmentId,
        second: SegmentId,
        dimension: Dimension,
    },
    /// The circle has exactly this radius.
    Radius {
        circle: CircleId,
        dimension: Dimension,
    },
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
    /// none of the fourteen names another constraint, so the cascade is two
    /// levels deep and stops without being made to. That is a property of the
    /// list below rather than of the type — see [`Entity`] — and the sweep in
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
            | Constraint::EqualLength { first, second }
            | Constraint::Spacing { first, second, .. } => {
                [Some(Entity::Segment(first)), Some(Entity::Segment(second))]
            }
            Constraint::PointOnSegment { point, segment }
            | Constraint::Standoff { point, segment, .. } => {
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
    /// Read off the same list the sketch writes a dimension through, so which
    /// variants carry a magnitude is stated once rather than in two lists free
    /// to disagree.
    ///
    /// Taken by value because reaching that list wants a `&mut` of its own: a
    /// constraint is [`Copy`], so a caller holding one spends nothing, and one
    /// holding a reference makes the copy at its own call rather than here.
    pub fn value(mut self) -> Option<f64> {
        self.value_mut().copied()
    }

    /// The magnitude, to restate it at something else.
    ///
    /// Kept inside `sketch` where [`Self::value`] is public, because a caller
    /// outside changes one through the sketch that holds it — see
    /// [`Sketch::set_value`](crate::Sketch::set_value). Editing a constraint in
    /// a caller's own hand would leave the sketch it came from unsolved.
    pub(super) fn value_mut(&mut self) -> Option<&mut f64> {
        self.dimension_mut().map(|dimension| &mut dimension.value)
    }

    /// The whole dimension, to restate what it says or move where it says it.
    ///
    /// The one list of which variants carry a number, spelled out over all
    /// fourteen rather than falling through: a fifteenth carrying a value — an
    /// angle, most obviously — has to say here that it does, and one that
    /// quietly answered `None` would be a dimension the drawing showed as a
    /// symbol and refused to edit or to place.
    ///
    /// Inside `sketch` for [`Self::value_mut`]'s reason, and reached through
    /// the sketch by [`Sketch::set_value`](crate::Sketch::set_value) and
    /// [`Sketch::place`](crate::Sketch::place).
    ///
    /// **And no immutable twin beside it**, which is why [`Self::value`] reads
    /// this rather than a list of its own: a second match over the fourteen
    /// arms *is* a second list, and the two would be free to disagree about one
    /// variant without the compiler saying so.
    pub(super) fn dimension_mut(&mut self) -> Option<&mut Dimension> {
        match self {
            Constraint::Distance { dimension, .. }
            | Constraint::Standoff { dimension, .. }
            | Constraint::Spacing { dimension, .. }
            | Constraint::Radius { dimension, .. } => Some(dimension),
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
            Constraint::Distance {
                a,
                b,
                along,
                dimension,
            } => {
                let (pa, pb) = (sketch.point(a).position, sketch.point(b).position);
                // Projected before it is measured, which is the whole of what
                // the three readings differ by: what survives is still a
                // difference, so its length is the residual and its direction
                // the partials, exactly as an unprojected one would be. A
                // reading that drops an axis drops that axis's partials with
                // it, because the projected difference has nothing there.
                let apart = Direction::of(along.project(pa - pb));
                row.point(a, apart.unit);
                row.point(b, -apart.unit);
                apart.length - dimension.value
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
            Constraint::Standoff {
                point,
                segment,
                dimension,
            } => standoff(
                sketch,
                row,
                Gap {
                    at: sketch.point(point).position,
                    // The place is a point the sketch holds, so the whole of
                    // the gradient goes to it.
                    carried: &[(point, 1.0)],
                    segment,
                    distance: dimension.value,
                },
            ),
            Constraint::Spacing {
                first,
                second,
                dimension,
            } => {
                let edge = sketch.segment(second);
                let (a, b) = (sketch.point(edge.a).position, sketch.point(edge.b).position);
                standoff(
                    sketch,
                    row,
                    Gap {
                        at: a.midpoint(b),
                        // The place is the middle of an edge, which moves half
                        // as fast as either end — so each end takes half the
                        // gradient. Evenly, which is what makes a pull on this
                        // slide the edge rather than pivot it.
                        carried: &[(edge.a, 0.5), (edge.b, 0.5)],
                        segment: first,
                        distance: dimension.value,
                    },
                )
            }
            Constraint::Radius { circle, dimension } => {
                row.radius(circle, 1.0);
                sketch.circle(circle).radius - dimension.value
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
                let c = sketch.circle(circle);
                // **The radius is the one gap here that is itself a
                // parameter**, so it takes a column where a typed dimension
                // takes none. The residual subtracts it, which is the whole of
                // its gradient.
                row.radius(circle, -1.0);
                standoff(
                    sketch,
                    row,
                    Gap {
                        at: sketch.point(c.center).position,
                        // The place is the circle's centre, a point the sketch
                        // holds, so the whole of the gradient goes to it.
                        carried: &[(c.center, 1.0)],
                        segment,
                        distance: c.radius,
                    },
                )
            }
            Constraint::EqualRadius { first, second } => {
                row.radius(first, 1.0);
                row.radius(second, -1.0);
                sketch.circle(first).radius - sketch.circle(second).radius
            }
        }
    }
}

/// The gap a standoff residual is measured across, and what the place it is
/// measured from is made of.
///
/// The arguments [`standoff`] takes, gathered because they only mean anything
/// together: `at` is where the place currently is and `carried` is which
/// parameters it is a function of, so a caller handing over one without the
/// other would be asking for a gradient of the wrong thing.
#[derive(Debug)]
struct Gap<'a> {
    /// Where the place is now.
    at: DVec2,
    /// The points `at` is built from, each with how fast it moves when that
    /// point does — one at full weight for a point the sketch holds, two at a
    /// half each for the middle of an edge.
    carried: &'a [(PointId, f64)],
    /// The segment whose infinite line the place stands off.
    segment: SegmentId,
    /// How far off that line the place is asked to stand.
    ///
    /// A number a user typed, or a circle's radius — which is a parameter, so
    /// there the caller writes its column and this reads its value alone.
    distance: f64,
}

/// The residual and partials of "this place stands `distance` off that line".
///
/// Shared by [`Constraint::Standoff`], [`Constraint::Spacing`] and
/// [`Constraint::Tangent`], which differ only in where the place comes from — a
/// point the sketch holds, the middle of an edge, or a circle's centre — and so
/// only in [`Gap::carried`]. Written once, because the three differing in the
/// chain rule applied to one of three gradients is not three equations.
///
/// **Divided by the length rather than multiplied through**, which is what lets
/// one spelling serve all three: the other side of the equation is a number a
/// user typed or a radius they can drag, and an equation scaled by an edge's
/// length would converge differently for every length of edge. What that costs
/// is the guard below, against a segment whose ends have met.
///
/// A private free fn rather than a method: it asks the sketch for geometry and
/// answers a number, and there is no type here it is *of*.
fn standoff(sketch: &Sketch, row: &mut JacobianRow<'_>, standing: Gap<'_>) -> f64 {
    let Gap {
        at,
        carried,
        segment,
        distance,
    } = standing;
    let s = sketch.segment(segment);
    let edge = direction(sketch, s);
    let offset = at - sketch.point(s.a).position;
    let along = Direction::of(edge);
    // The line's unit normal, and how far off it the place stands. Signed,
    // which the residual then takes back out for [`Constraint::Tangent`]'s
    // reason: a distance has no sign, so the relation holds mirrored either way
    // and a solve settles onto whichever side it started from.
    let normal = along.unit.perp();
    let reach = normal.dot(offset);
    let side = if reach < 0.0 { -1.0 } else { 1.0 };
    for &(point, share) in carried {
        row.point(point, side * share * normal);
    }
    // The two ends only where there is a line for them to be a gradient of.
    // Both terms divide by the length, so a segment whose ends have met would
    // run away — and it has no line to stand off in the first place. What is
    // left is the place's own gradient, which still asks it to stand `distance`
    // from where the edge collapsed to; what is given up is any way for this
    // equation to push the ends apart, which is right, because a distance to a
    // point says nothing about how long an edge should be.
    if along.known() {
        let scale = side / along.length;
        // Each is [`Constraint::PointOnSegment`]'s own gradient, divided by the
        // length, plus the term that division adds: the length moves with
        // either end too, and how far off the line the place stands scales with
        // it.
        row.point(s.a, scale * ((offset - edge).perp() + reach * along.unit));
        row.point(s.b, scale * (-offset.perp() - reach * along.unit));
    }
    side * reach - distance
}

#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::sketch::PointId;
    use crate::sketch::constraint::{Along, Constraint, Dimension};

    impl Constraint {
        /// A distance of `value` between two points, read the shortest way.
        ///
        /// The reading every fixture that is not *about* the reading wants, and
        /// the one that spells out to five lines of `Along` and `Dimension`
        /// wherever it is written in full. Which way a distance is read is the
        /// whole subject of one test next door, and that one writes its own out
        /// — being the one place the choice is worth seeing at the call site.
        pub fn apart(a: PointId, b: PointId, value: f64) -> Self {
            Self::Distance {
                a,
                b,
                along: Along::Shortest,
                dimension: Dimension::new(value),
            }
        }
    }
}

#[cfg(test)]
mod tests;
