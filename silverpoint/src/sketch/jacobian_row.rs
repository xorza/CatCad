//! One equation's row of the Jacobian, and the partials written into it.

use crate::sketch::params::Params;
use crate::sketch::{CircleId, PointId, Segment};
use glam::DVec2;

/// One row of the Jacobian: a cell per parameter of the sketch it was sized
/// against, and the only way to reach one.
///
/// Every partial is *added* to the cell it names, never assigned. A constraint
/// is free to name one entity twice, and then both writes land on the same
/// cells: assigning would let the second silently replace the first, where the
/// sum is the derivative the chain rule actually asks for. `Perpendicular` on
/// one segment has to come out at twice the gradient, `Parallel` on one segment
/// at none, and a point constrained against itself at none — all three fall out
/// of adding and none of them out of assigning. Nothing here assigns, which is
/// what keeps that from being a rule anyone has to remember.
///
/// The caller hands over a zeroed row, so where nothing collides an addition is
/// what an assignment would have written anyway.
///
/// **Every column written is written down**, in `touched`, because the caller
/// reading the row out has no other way to find the few of them: a row is dense
/// and nearly empty, and scanning it costs the width of the sketch per equation
/// to rediscover what this was just told. Duplicates and no order are the
/// reader's to settle, which keeps every write below to one push.
#[derive(Debug)]
pub(super) struct JacobianRow<'a> {
    params: Params<'a>,
    cells: &'a mut [f64],
    touched: &'a mut Vec<u32>,
}

impl<'a> JacobianRow<'a> {
    /// Read `cells` as the row of the sketch `params` describes, which it must
    /// already be as wide as, writing every column named into `touched`.
    pub(super) fn new(params: Params<'a>, cells: &'a mut [f64], touched: &'a mut Vec<u32>) -> Self {
        debug_assert_eq!(cells.len(), params.count());
        debug_assert!(touched.is_empty(), "a row of another equation");
        Self {
            params,
            cells,
            touched,
        }
    }

    /// Add `gradient` to a point's two cells.
    pub(super) fn point(&mut self, point: PointId, gradient: DVec2) {
        let [x, y] = self.params.of_point(point);
        self.cells[x] += gradient.x;
        self.cells[y] += gradient.y;
        self.touched.push(x as u32);
        self.touched.push(y as u32);
    }

    /// Add `gradient` to a segment's endpoints, as the partials of a residual
    /// that reads the segment's direction.
    ///
    /// The head gains it and the tail loses it, because the direction is
    /// `head - tail` and moving either end moves it by the same amount in
    /// opposite senses.
    pub(super) fn segment(&mut self, segment: Segment, gradient: DVec2) {
        self.point(segment.b, gradient);
        self.point(segment.a, -gradient);
    }

    /// Add `partial` to a circle's radius cell.
    pub(super) fn radius(&mut self, circle: CircleId, partial: f64) {
        let at = self.params.of_radius(circle);
        self.cells[at] += partial;
        self.touched.push(at as u32);
    }
}
