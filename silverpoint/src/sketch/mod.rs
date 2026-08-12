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

    pub fn segment(&self, id: SegmentId) -> Segment {
        self.segments[id.idx()]
    }

    pub fn circle(&self, id: CircleId) -> Circle {
        self.circles[id.idx()]
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
