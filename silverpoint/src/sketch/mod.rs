//! 2D sketching: the geometry being solved, and the constraints tying it
//! together.

pub(crate) mod constraint;
pub(crate) mod solver;

use crate::sketch::constraint::Constraint;
use glam::DVec2;

/// Handle to a point in a [`Sketch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PointId(u32);

impl PointId {
    fn idx(self) -> usize {
        self.0 as usize
    }
}

/// Handle to a segment in a [`Sketch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SegmentId(u32);

impl SegmentId {
    fn idx(self) -> usize {
        self.0 as usize
    }
}

/// Handle to a circle in a [`Sketch`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CircleId(u32);

impl CircleId {
    fn idx(self) -> usize {
        self.0 as usize
    }
}

/// A point's position, and whether the solver may move it.
#[derive(Debug, Clone, Copy)]
struct Point {
    position: DVec2,
    /// The solver leaves it where it is. Anchors the sketch so a
    /// well-constrained system isn't still free to translate and rotate.
    fixed: bool,
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
/// straight into it, so nothing needs to be looked up by name — see [`Param`],
/// which is where that layout is stated.
#[derive(Debug, Clone, Default)]
pub struct Sketch {
    points: Vec<Point>,
    segments: Vec<Segment>,
    circles: Vec<Circle>,
    constraints: Vec<Constraint>,
}

impl Sketch {
    /// Add a free point at `position`, which is also its starting guess. The
    /// solver converges on the solution nearest the guess, so place points
    /// roughly where they belong.
    pub fn add_point(&mut self, position: DVec2) -> PointId {
        self.points.push(Point {
            position,
            fixed: false,
        });
        PointId((self.points.len() - 1) as u32)
    }

    /// Pin a point where it is. At least one fixed point is usually wanted:
    /// otherwise every sketch keeps three degrees of freedom for its own
    /// placement.
    pub fn fix(&mut self, point: PointId) {
        self.points[point.idx()].fixed = true;
    }

    pub fn add_segment(&mut self, a: PointId, b: PointId) -> SegmentId {
        self.segments.push(Segment { a, b });
        SegmentId((self.segments.len() - 1) as u32)
    }

    /// Add a circle whose `radius` is a starting guess, free to move unless a
    /// constraint fixes it.
    pub fn add_circle(&mut self, center: PointId, radius: f64) -> CircleId {
        self.circles.push(Circle { center, radius });
        CircleId((self.circles.len() - 1) as u32)
    }

    pub fn add_constraint(&mut self, constraint: Constraint) {
        self.constraints.push(constraint);
    }

    pub fn point(&self, id: PointId) -> DVec2 {
        self.points[id.idx()].position
    }

    /// Every point in insertion order, each with the handle needed to ask
    /// [`Self::is_fixed`] about it.
    pub fn points(&self) -> impl Iterator<Item = (PointId, DVec2)> {
        self.points
            .iter()
            .enumerate()
            .map(|(index, point)| (PointId(index as u32), point.position))
    }

    pub fn segment(&self, id: SegmentId) -> Segment {
        self.segments[id.idx()]
    }

    /// Every segment in insertion order. Each carries the handles of its own
    /// endpoints, so nothing here needs a [`SegmentId`] to be usable.
    pub fn segments(&self) -> &[Segment] {
        &self.segments
    }

    pub fn circle(&self, id: CircleId) -> Circle {
        self.circles[id.idx()]
    }

    /// Every circle in insertion order, each carrying its centre handle.
    pub fn circles(&self) -> &[Circle] {
        &self.circles
    }

    pub fn is_fixed(&self, id: PointId) -> bool {
        self.points[id.idx()].fixed
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Size of the solver's parameter vector.
    pub fn param_count(&self) -> usize {
        self.radius_base() + self.circles.len()
    }

    /// Where the radii start, which is the boundary the whole layout turns on.
    fn radius_base(&self) -> usize {
        self.points.len() * 2
    }

    /// Where `param` sits in the parameter vector. Inverse of [`Self::param`].
    fn param_index(&self, param: Param) -> usize {
        match param {
            Param::Point(id, Axis::X) => id.idx() * 2,
            Param::Point(id, Axis::Y) => id.idx() * 2 + 1,
            Param::Radius(id) => self.radius_base() + id.idx(),
        }
    }

    /// What the parameter at `index` names. Inverse of [`Self::param_index`].
    fn param(&self, index: usize) -> Param {
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
            Param::Point(PointId((index / 2) as u32), axis)
        } else {
            Param::Radius(CircleId((index - self.radius_base()) as u32))
        }
    }

    /// Index of a point's x parameter; its y follows immediately.
    pub(crate) fn point_param(&self, id: PointId) -> usize {
        self.param_index(Param::Point(id, Axis::X))
    }

    pub(crate) fn radius_param(&self, id: CircleId) -> usize {
        self.param_index(Param::Radius(id))
    }

    /// Whether the solver may move this parameter. Radii always move; point
    /// coordinates move unless the point is fixed.
    pub(crate) fn param_is_free(&self, index: usize) -> bool {
        match self.param(index) {
            Param::Point(id, _) => !self.is_fixed(id),
            Param::Radius(_) => true,
        }
    }

    fn param_value(&self, param: Param) -> f64 {
        match param {
            Param::Point(id, axis) => axis.component(self.points[id.idx()].position),
            Param::Radius(id) => self.circles[id.idx()].radius,
        }
    }

    fn set_param_value(&mut self, param: Param, value: f64) {
        match param {
            Param::Point(id, axis) => axis.set(&mut self.points[id.idx()].position, value),
            Param::Radius(id) => self.circles[id.idx()].radius = value,
        }
    }

    pub(crate) fn params(&self) -> Vec<f64> {
        (0..self.param_count())
            .map(|index| self.param_value(self.param(index)))
            .collect()
    }

    pub(crate) fn set_params(&mut self, params: &[f64]) {
        debug_assert_eq!(params.len(), self.param_count());
        for (index, &value) in params.iter().enumerate() {
            let param = self.param(index);
            self.set_param_value(param, value);
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

        let segments = sketch.segments();
        assert_eq!(segments.len(), 2);
        assert_eq!((segments[0].a, segments[0].b), (a, b));
        assert_eq!((segments[1].a, segments[1].b), (b, c));

        let circles = sketch.circles();
        assert_eq!(circles.len(), 1);
        assert_eq!(circles[0].center, c);
        assert_eq!(circles[0].radius, 0.5);

        // Solving rewrites positions through the same order, so the iterator
        // reports what the solver left behind rather than the initial guess.
        // Radii ride the same vector: three points fill 0..6, so the circle's
        // radius is parameter 6.
        let mut params = sketch.params();
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
            assert_eq!(sketch.param_index(sketch.param(index)), index, "{index}");
        }
        assert_eq!(sketch.param(0), Param::Point(a, Axis::X));
        assert_eq!(sketch.param(3), Param::Point(b, Axis::Y));
        assert_eq!(sketch.param(6), Param::Radius(inner));
        assert_eq!(sketch.param(7), Param::Radius(outer));

        // Only b is pinned, so only its two coordinates are held. Radii move
        // whatever the points do.
        let free: Vec<bool> = (0..sketch.param_count())
            .map(|index| sketch.param_is_free(index))
            .collect();
        assert_eq!(free, [true, true, false, false, true, true, true, true]);
    }
}
