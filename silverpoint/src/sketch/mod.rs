//! 2D sketching: the geometry being solved, and the constraints tying it
//! together.

pub(crate) mod constraint;
pub(crate) mod solver;

use crate::arena::{Arena, Id};
use crate::sketch::constraint::Constraint;
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
    /// length of a drag by [`Solver::solve_holding`](crate::Solver).
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
        self.radius_base() + self.circles.slot_count()
    }

    /// Where the radii start, which is the boundary the whole layout turns on.
    fn radius_base(&self) -> usize {
        self.points.slot_count() * 2
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
    /// Appends rather than replacing, so the caller owns the buffer — which is
    /// what lets the solver keep one across solves instead of being handed a
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

    pub(crate) fn set_params(&mut self, params: &[f64]) {
        debug_assert_eq!(params.len(), self.param_count());
        for (index, &value) in params.iter().enumerate() {
            if let Some(param) = self.param(index) {
                self.set_param_value(param, value);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The iteration order is the parameter order, which is what lets a
    /// handle index straight into the parameter vector.
    #[test]
    fn geometry_comes_back_in_insertion_order() {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::new(1.0, 2.0));
        let b = sketch.add_point(DVec2::new(3.0, 4.0));
        let c = sketch.add_point(DVec2::new(5.0, 6.0));
        sketch.fix(b);
        sketch.add_segment(a, b);
        sketch.add_segment(b, c);
        let circle = sketch.add_circle(c, 0.5);

        let points: Vec<_> = sketch.points().collect();
        assert_eq!(points.len(), 3);
        assert_eq!(points[0], (a, DVec2::new(1.0, 2.0)));
        assert_eq!(points[1], (b, DVec2::new(3.0, 4.0)));
        assert_eq!(points[2], (c, DVec2::new(5.0, 6.0)));
        // The handle the iterator hands back is the one `is_fixed` answers
        // for — only the second point was pinned.
        let fixed: Vec<bool> = points.iter().map(|&(id, _)| sketch.is_fixed(id)).collect();
        assert_eq!(fixed, [false, true, false]);

        let segments: Vec<_> = sketch.segments().collect();
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].1.a, segments[0].1.b), (a, b));
        assert_eq!((segments[1].1.a, segments[1].1.b), (b, c));
        // The handle each carries is the one that names it back.
        assert_eq!(sketch.segment(segments[1].0).a, b);

        let circles: Vec<_> = sketch.circles().collect();
        assert_eq!(circles.len(), 1);
        assert_eq!(circles[0].0, circle);
        assert_eq!(circles[0].1.center, c);
        assert_eq!(circles[0].1.radius, 0.5);

        // Solving rewrites positions through the same order, so the iterator
        // reports what the solver left behind rather than the initial guess.
        // Radii ride the same vector: three points fill 0..6, so the circle's
        // radius is parameter 6.
        let mut params = Vec::new();
        sketch.write_params(&mut params);
        assert_eq!(params, [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 0.5]);
        params[2] = 30.0;
        params[6] = 0.75;
        sketch.set_params(&params);
        assert_eq!(sketch.points().nth(1).unwrap().1, DVec2::new(30.0, 4.0));
        assert_eq!(sketch.circle(circle).radius, 0.75);
    }

    /// The layout, against hand-counted indices: three points fill 0..6 two
    /// apiece, then one radius each at 6 and 7.
    #[test]
    fn every_parameter_index_names_something_and_names_it_back() {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::new(1.0, 2.0));
        let b = sketch.add_point(DVec2::new(3.0, 4.0));
        let c = sketch.add_point(DVec2::new(5.0, 6.0));
        let inner = sketch.add_circle(a, 0.5);
        let outer = sketch.add_circle(c, 1.5);
        sketch.fix(b);

        assert_eq!(sketch.param_count(), 8);
        assert_eq!(sketch.point_param(a), 0);
        assert_eq!(sketch.point_param(b), 2);
        assert_eq!(sketch.point_param(c), 4);
        assert_eq!(sketch.radius_param(inner), 6);
        assert_eq!(sketch.radius_param(outer), 7);

        // The round trip is what keeps the forward map and the reverse lookup
        // in step: break either and some index stops coming back as itself.
        for index in 0..sketch.param_count() {
            let param = sketch.param(index).expect("nothing has been removed");
            assert_eq!(sketch.param_index(param), index, "{index}");
        }
        assert_eq!(sketch.param(0), Some(Param::Point(a, Axis::X)));
        assert_eq!(sketch.param(3), Some(Param::Point(b, Axis::Y)));
        assert_eq!(sketch.param(6), Some(Param::Radius(inner)));
        assert_eq!(sketch.param(7), Some(Param::Radius(outer)));

        // Only b is pinned, so only its two coordinates are held. Radii move
        // whatever the points do.
        let free: Vec<bool> = (0..sketch.param_count())
            .map(|index| sketch.param_is_free(index))
            .collect();
        assert_eq!(free, [true, true, false, false, true, true, true, true]);
    }

    /// A removal leaves the vector the width it was, with a hole where the
    /// point used to be — which is what keeps every surviving handle indexing
    /// where it did.
    #[test]
    fn a_removed_points_parameters_stay_put_and_never_move() {
        let mut sketch = Sketch::default();
        let a = sketch.add_point(DVec2::new(1.0, 2.0));
        let b = sketch.add_point(DVec2::new(3.0, 4.0));
        let circle = sketch.add_circle(b, 0.5);
        assert_eq!(sketch.param_count(), 5);

        // Reaching past the public API on purpose: removal isn't exposed until
        // it can cascade, and this is the behaviour that has to be right first.
        sketch.points.remove(a);

        assert_eq!(sketch.param_count(), 5);
        assert_eq!(sketch.param(0), None);
        assert_eq!(sketch.param(1), None);
        assert_eq!(sketch.param(2), Some(Param::Point(b, Axis::X)));
        assert_eq!(sketch.param(4), Some(Param::Radius(circle)));
        assert_eq!(sketch.point_param(b), 2);
        assert_eq!(sketch.radius_param(circle), 4);

        // The hole is unfree, which is the whole of what the solver needs: it
        // already pins a parameter it may not move and zeroes that column.
        let free: Vec<bool> = (0..5).map(|index| sketch.param_is_free(index)).collect();
        assert_eq!(free, [false, false, true, true, true]);

        // It reads zero and refuses to be written, so a step landing on it
        // changes nothing.
        let mut params = Vec::new();
        sketch.write_params(&mut params);
        assert_eq!(params, [0.0, 0.0, 3.0, 4.0, 0.5]);
        sketch.set_params(&[9.0; 5]);
        params.clear();
        sketch.write_params(&mut params);
        assert_eq!(params, [0.0, 0.0, 9.0, 9.0, 9.0]);

        // The freed position is filled again rather than the vector widening,
        // and the handle to what was there is refused, not answered.
        let c = sketch.add_point(DVec2::new(5.0, 6.0));
        assert_eq!(sketch.param_count(), 5);
        assert_eq!(sketch.point_param(c), 0);
        assert_ne!(c, a);
        assert_eq!(sketch.points().count(), 2);
    }
}
