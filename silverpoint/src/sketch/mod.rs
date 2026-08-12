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

/// Points, segments, circles, and the constraints between them.
///
/// The solver's parameter vector is this sketch flattened: two entries per
/// point in insertion order, then one radius per circle. Handles index
/// straight into it, so nothing needs to be looked up by name.
#[derive(Debug, Clone, Default)]
pub struct Sketch {
    points: Vec<DVec2>,
    /// Per point: the solver leaves it where it is. Anchors the sketch so a
    /// well-constrained system isn't still free to translate and rotate.
    fixed: Vec<bool>,
    segments: Vec<Segment>,
    circles: Vec<Circle>,
    constraints: Vec<Constraint>,
}

impl Sketch {
    /// Add a free point at `position`, which is also its starting guess. The
    /// solver converges on the solution nearest the guess, so place points
    /// roughly where they belong.
    pub fn add_point(&mut self, position: DVec2) -> PointId {
        self.points.push(position);
        self.fixed.push(false);
        PointId((self.points.len() - 1) as u32)
    }

    /// Pin a point where it is. At least one fixed point is usually wanted:
    /// otherwise every sketch keeps three degrees of freedom for its own
    /// placement.
    pub fn fix(&mut self, point: PointId) {
        self.fixed[point.idx()] = true;
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
        self.points[id.idx()]
    }

    /// Every point in insertion order, each with the handle needed to ask
    /// [`Self::is_fixed`] about it.
    pub fn points(&self) -> impl Iterator<Item = (PointId, DVec2)> {
        self.points
            .iter()
            .enumerate()
            .map(|(index, position)| (PointId(index as u32), *position))
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
        self.fixed[id.idx()]
    }

    pub fn constraints(&self) -> &[Constraint] {
        &self.constraints
    }

    /// Size of the solver's parameter vector.
    pub fn param_count(&self) -> usize {
        self.points.len() * 2 + self.circles.len()
    }

    /// Index of a point's x parameter; its y follows immediately.
    pub(crate) fn point_param(&self, id: PointId) -> usize {
        id.idx() * 2
    }

    pub(crate) fn radius_param(&self, id: CircleId) -> usize {
        self.points.len() * 2 + id.idx()
    }

    /// Whether the solver may move this parameter. Radii always move; point
    /// coordinates move unless the point is fixed.
    pub(crate) fn param_is_free(&self, param: usize) -> bool {
        let point_params = self.points.len() * 2;
        param >= point_params || !self.fixed[param / 2]
    }

    pub(crate) fn params(&self) -> Vec<f64> {
        let mut params = Vec::with_capacity(self.param_count());
        params.extend(self.points.iter().flat_map(|p| [p.x, p.y]));
        params.extend(self.circles.iter().map(|c| c.radius));
        params
    }

    pub(crate) fn set_params(&mut self, params: &[f64]) {
        debug_assert_eq!(params.len(), self.param_count());
        for (point, values) in self.points.iter_mut().zip(params.chunks_exact(2)) {
            *point = DVec2::new(values[0], values[1]);
        }
        let radii = &params[self.points.len() * 2..];
        for (circle, radius) in self.circles.iter_mut().zip(radii) {
            circle.radius = *radius;
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
        sketch.add_circle(c, 0.5);

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
        let mut params = sketch.params();
        params[2] = 30.0;
        sketch.set_params(&params);
        assert_eq!(sketch.points().nth(1).unwrap().1, DVec2::new(30.0, 4.0));
    }
}
