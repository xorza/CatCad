//! 2D sketching: the geometry being solved, and the constraints tying it
//! together.

pub(crate) mod constraint;
pub(crate) mod snapshot;
pub(crate) mod solver;

use crate::arena::{Arena, Id};
use crate::sketch::constraint::Constraint;
use crate::sketch::snapshot::Snapshot;
use glam::DVec2;

/// Handle to a point in a [`Sketch`].
pub type PointId = Id<Point>;

/// Handle to a segment in a [`Sketch`].
pub type SegmentId = Id<Segment>;

/// Handle to a circle in a [`Sketch`].
pub type CircleId = Id<Circle>;

/// What a handle to something the sketch no longer holds reports. Reaching one
/// means a caller kept a handle across a removal, which is a mistake in the
/// caller rather than anything the sketch can answer.
const REMOVED_POINT: &str = "this point is no longer in the sketch";
const REMOVED_SEGMENT: &str = "this segment is no longer in the sketch";
const REMOVED_CIRCLE: &str = "this circle is no longer in the sketch";

/// A point's position, and whether the solver may move it.
#[derive(Debug, Clone, Copy)]
pub struct Point {
    pub position: DVec2,
    /// The solver leaves it where it is. Anchors the sketch so a
    /// well-constrained system isn't still free to translate and rotate.
    pub fixed: bool,
}

/// A straight edge between two points. Carries no parameters of its own — it
/// is entirely defined by its endpoints.
#[derive(Debug, Clone, Copy)]
pub struct Segment {
    pub a: PointId,
    pub b: PointId,
}

/// A circle about a point. The radius is a solver parameter, so a
/// [`Constraint::Radius`] can pin it or a tangency can drive it.
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub center: PointId,
    pub radius: f64,
}

/// Which of a point's two coordinates a parameter names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Axis {
    X,
    Y,
}

impl Axis {
    /// The component this axis names.
    fn component(self, point: DVec2) -> f64 {
        match self {
            Axis::X => point.x,
            Axis::Y => point.y,
        }
    }

    /// Overwrite the component this axis names, leaving the other alone.
    fn set(self, point: &mut DVec2, value: f64) {
        match self {
            Axis::X => point.x = value,
            Axis::Y => point.y = value,
        }
    }
}

/// One slot of the solver's parameter vector, named.
///
/// [`Sketch::param_index`] and [`Sketch::param`] are inverses of each other,
/// and between them they are the whole statement of the layout. Everything
/// that reads a parameter, writes one, or asks whether it may move goes
/// through one of the two rather than doing the arithmetic again — so a layout
/// change is those two functions and the round-trip test over them.
///
/// All of it sits on [`Sketch`] rather than in a module of its own, though it
/// is about the vector a sketch flattens to rather than about a sketch. An impl
/// block belongs to its type's file, so a module would mean free functions:
/// `param::of_point(sketch, id)` in place of `sketch.point_param(id)`, at every
/// call in the solver. The layout being one type's business is worth less than
/// the crate's most-read code reading as it does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Param {
    Point(PointId, Axis),
    Radius(CircleId),
}

/// Points, segments, circles, and the constraints between them.
///
/// The solver's parameter vector is this sketch flattened: two entries per
/// point in insertion order, then one radius per circle. Handles index
/// straight into it, so nothing needs to be looked up by name — see `Param`,
/// which is where that layout is stated.
#[derive(Debug, Clone, Default)]
pub struct Sketch {
    points: Arena<Point>,
    segments: Arena<Segment>,
    circles: Arena<Circle>,
    constraints: Vec<Constraint>,
}

impl Sketch {
    /// Add a free point at `position`, which is also its starting guess. The
    /// solver converges on the solution nearest the guess, so place points
    /// roughly where they belong.
    pub fn add_point(&mut self, position: DVec2) -> PointId {
        self.points.insert(Point {
            position,
            fixed: false,
        })
    }

    /// Pin a point where it is. At least one fixed point is usually wanted:
    /// otherwise every sketch keeps three degrees of freedom for its own
    /// placement.
    pub fn fix(&mut self, point: PointId) {
        self.point_mut(point).fixed = true;
    }

    pub fn add_segment(&mut self, a: PointId, b: PointId) -> SegmentId {
        // Checked here rather than where the endpoints are next read: a
        // segment outliving a point is the caller's mistake, and it is worth
        // more at the line that made it than deep inside a solve.
        assert!(
            self.points.contains(a) && self.points.contains(b),
            "a segment needs two points the sketch still holds"
        );
        self.segments.insert(Segment { a, b })
    }

    /// Add a circle whose `radius` is a starting guess, free to move unless a
    /// constraint fixes it.
    pub fn add_circle(&mut self, center: PointId, radius: f64) -> CircleId {
        assert!(
            self.points.contains(center),
            "a circle needs a centre the sketch still holds"
        );
        self.circles.insert(Circle { center, radius })
    }

    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    pub fn point(&self, id: PointId) -> DVec2 {
        self.points.get(id).expect(REMOVED_POINT).position
    }

    /// Move a point.
    ///
    /// A starting guess like the one it was added with, not a constraint: the
    /// next solve is free to move it again unless it is fixed, or held for the
    /// length of a drag by [`Solver::edit_holding`](crate::Solver).
    pub fn set_point(&mut self, id: PointId, position: DVec2) {
        self.point_mut(id).position = position;
    }

    fn point_mut(&mut self, id: PointId) -> &mut Point {
        self.points.get_mut(id).expect(REMOVED_POINT)
    }

    /// Every point in insertion order, each with the handle needed to ask
    /// [`Self::is_fixed`] about it.
    pub fn points(&self) -> impl Iterator<Item = (PointId, DVec2)> {
        self.points.iter().map(|(id, point)| (id, point.position))
    }

    pub fn segment(&self, id: SegmentId) -> Segment {
        *self.segments.get(id).expect(REMOVED_SEGMENT)
    }

    /// Every segment in insertion order, each with the handle that names it.
    pub fn segments(&self) -> impl Iterator<Item = (SegmentId, Segment)> {
        self.segments.iter().map(|(id, segment)| (id, *segment))
    }

    pub fn circle(&self, id: CircleId) -> Circle {
        *self.circles.get(id).expect(REMOVED_CIRCLE)
    }

    fn circle_mut(&mut self, id: CircleId) -> &mut Circle {
        self.circles.get_mut(id).expect(REMOVED_CIRCLE)
    }

    /// Resize a circle.
    ///
    /// A starting guess like the one it was added with, exactly as
    /// [`Sketch::set_point`] is: a radius is a solver parameter, so the next
    /// solve is free to move it again unless a constraint holds it.
    pub fn set_radius(&mut self, id: CircleId, radius: f64) {
        self.circle_mut(id).radius = radius;
    }

    /// Every circle in insertion order, each with the handle that names it.
    pub fn circles(&self) -> impl Iterator<Item = (CircleId, Circle)> {
        self.circles.iter().map(|(id, circle)| (id, *circle))
    }

    pub fn is_fixed(&self, id: PointId) -> bool {
        self.points.get(id).expect(REMOVED_POINT).fixed
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Size of the solver's parameter vector.
    ///
    /// Counts positions rather than what is in them: a removed point keeps its
    /// two entries, so every surviving handle keeps indexing where it did.
    pub fn param_count(&self) -> usize {
        self.radius_base() + self.circle_slot_count()
    }

    /// Positions the points occupy, holes from removals included — the length
    /// of anything keyed by point rather than by surviving point.
    pub(crate) fn point_slot_count(&self) -> usize {
        self.points.slot_count()
    }

    pub(crate) fn circle_slot_count(&self) -> usize {
        self.circles.slot_count()
    }

    /// Where the radii start, which is the boundary the whole layout turns on.
    fn radius_base(&self) -> usize {
        self.point_slot_count() * 2
    }

    /// Where `param` sits in the parameter vector. Inverse of [`Self::param`].
    fn param_index(&self, param: Param) -> usize {
        match param {
            Param::Point(id, Axis::X) => id.slot() * 2,
            Param::Point(id, Axis::Y) => id.slot() * 2 + 1,
            Param::Radius(id) => self.radius_base() + id.slot(),
        }
    }

    /// What the parameter at `index` names, or `None` where a removal left a
    /// hole. Inverse of [`Self::param_index`].
    ///
    /// An index alone can't name anything: rebuilding a handle needs the
    /// generation of the position, which only the store knows — and a freed
    /// position has no handle to give.
    fn param(&self, index: usize) -> Option<Param> {
        debug_assert!(
            index < self.param_count(),
            "parameter {index} is past the {} this sketch has",
            self.param_count()
        );
        if index < self.radius_base() {
            let axis = if index.is_multiple_of(2) {
                Axis::X
            } else {
                Axis::Y
            };
            Some(Param::Point(self.points.id_at_slot(index / 2)?, axis))
        } else {
            let slot = index - self.radius_base();
            Some(Param::Radius(self.circles.id_at_slot(slot)?))
        }
    }

    /// Index of a point's x parameter; its y follows immediately.
    pub(crate) fn point_param(&self, id: PointId) -> usize {
        self.param_index(Param::Point(id, Axis::X))
    }

    pub(crate) fn radius_param(&self, id: CircleId) -> usize {
        self.param_index(Param::Radius(id))
    }

    /// Add `gradient` to a point's two parameters.
    ///
    /// Added, never assigned. A constraint is free to name one entity twice,
    /// and then both writes land on these same two slots: assigning would let
    /// the second silently replace the first, where the sum is the derivative
    /// the chain rule actually asks for. `Perpendicular` on one segment has to
    /// come out at twice the gradient, `Parallel` on one segment at none, and
    /// a point constrained against itself at none — all three fall out of
    /// adding and none of them out of assigning.
    ///
    /// The caller zeroes the row, so with no collision this is what assigning
    /// would have written anyway.
    pub(crate) fn write_point_partials(&self, row: &mut [f64], point: PointId, gradient: DVec2) {
        let index = self.point_param(point);
        row[index] += gradient.x;
        row[index + 1] += gradient.y;
    }

    /// Add `gradient` to a segment's endpoints, as the partials of a residual
    /// that reads the segment's direction.
    ///
    /// The head gains it and the tail loses it, because the direction is
    /// `head - tail` and moving either end moves it by the same amount in
    /// opposite senses.
    pub(crate) fn write_segment_partials(
        &self,
        row: &mut [f64],
        segment: Segment,
        gradient: DVec2,
    ) {
        self.write_point_partials(row, segment.b, gradient);
        self.write_point_partials(row, segment.a, -gradient);
    }

    /// Whether the solver may move this parameter. Radii always move; point
    /// coordinates move unless the point is fixed; a hole left by a removal
    /// never moves, which is what keeps the solver off it without the solver
    /// having to know holes exist.
    pub(crate) fn param_is_free(&self, index: usize) -> bool {
        match self.param(index) {
            Some(Param::Point(id, _)) => !self.is_fixed(id),
            Some(Param::Radius(_)) => true,
            None => false,
        }
    }

    fn param_value(&self, param: Param) -> f64 {
        match param {
            Param::Point(id, axis) => axis.component(self.point(id)),
            Param::Radius(id) => self.circle(id).radius,
        }
    }

    fn set_param_value(&mut self, param: Param, value: f64) {
        match param {
            Param::Point(id, axis) => axis.set(&mut self.point_mut(id).position, value),
            Param::Radius(id) => self.circle_mut(id).radius = value,
        }
    }

    /// Append every parameter's current value, in index order.
    ///
    /// With [`Sketch::set_params`], the pair the solver works through: the
    /// parameter vector is the whole of what a step can have touched, so
    /// saving it and putting it back is how a step is undone.
    ///
    /// Appends rather than replacing, so the caller owns the buffer — which is
    /// what lets a solver kept across a drag keep one instead of being handed a
    /// fresh one every time.
    ///
    /// Reads zero at a hole, which is a value nothing will move: its column is
    /// zeroed and its step is pinned to zero, so the number is never used.
    pub(crate) fn write_params(&self, out: &mut Vec<f64>) {
        out.reserve_exact(self.param_count());
        out.extend((0..self.param_count()).map(|index| {
            self.param(index)
                .map_or(0.0, |param| self.param_value(param))
        }));
    }

    /// Put every parameter back, in the order [`Sketch::write_params`]
    /// wrote them.
    pub(crate) fn set_params(&mut self, params: &[f64]) {
        debug_assert_eq!(params.len(), self.param_count());
        for (index, &value) in params.iter().enumerate() {
            if let Some(param) = self.param(index) {
                self.set_param_value(param, value);
            }
        }
    }

    /// Take down where the geometry stands, so it can be put back later.
    ///
    /// Fills `into` rather than returning it, so a caller keeping one across a
    /// drag refills the same buffer sixty times a second instead of being
    /// handed a new one. Whatever was in it is gone, not appended to.
    pub fn snapshot_into(&self, into: &mut Snapshot) {
        into.at.clear();
        self.write_params(&mut into.at);
    }

    /// Put the geometry back where `snapshot` says it stood.
    ///
    /// Exact, parameter for parameter, which is what makes this an undo rather
    /// than an approximation of one: a sketch restored and a sketch never
    /// touched compare equal.
    ///
    /// Positions and radii only. Everything else a snapshot cannot reach —
    /// which points exist, what is pinned, what the constraints say — has to be
    /// the same as when it was taken, and only its width is checked.
    pub fn restore(&mut self, snapshot: &Snapshot) {
        debug_assert!(
            snapshot.fits(self),
            "this snapshot was taken of a sketch with different geometry in it"
        );
        self.set_params(&snapshot.at);
    }
}

#[cfg(test)]
mod tests;
