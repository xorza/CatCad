//! The sketch read as the vector a solve works on.
//!
//! Two entries per point in position order, then one radius per circle. Handles
//! index straight in, so nothing has to be looked up by name — and a removal
//! leaves a hole rather than closing up, which is what keeps every surviving
//! handle indexing where it did.

use crate::sketch::{CircleId, PointId, Sketch};
use glam::DVec2;

/// Which of a point's two coordinates a parameter names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Axis {
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

/// One slot of the parameter vector, named.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Param {
    Point(PointId, Axis),
    Radius(CircleId),
}

impl Param {
    fn value(self, sketch: &Sketch) -> f64 {
        match self {
            Param::Point(id, axis) => axis.component(sketch.point(id).position),
            Param::Radius(id) => sketch.circle(id).radius,
        }
    }

    pub(super) fn set(self, sketch: &mut Sketch, value: f64) {
        match self {
            Param::Point(id, axis) => axis.set(&mut sketch.point_mut(id).position, value),
            Param::Radius(id) => sketch.circle_mut(id).radius = value,
        }
    }
}

/// A sketch flattened to the parameter vector a solve works on.
///
/// [`Params::index_of`] and [`Params::at`] are inverses of each other, and
/// between them they are the whole statement of the layout. Everything that
/// reads a parameter, writes one, or asks whether it may move works the index
/// out here rather than doing the arithmetic again — so a layout change is those
/// two functions and the round-trip test over them.
///
/// Reading is the half that lives here. Writing is
/// [`Sketch::set_params`](crate::Sketch), because a parameter is written
/// *through* the sketch it belongs to and this holds that sketch immutably; what
/// it writes it still names by asking [`Params::at`].
///
/// A borrow rather than anything owned, so it costs what naming the sketch costs
/// and there is nothing to keep in step with the sketch it describes. Built by
/// [`Sketch::params`], which is the only place one comes from.
#[derive(Debug, Clone, Copy)]
pub(super) struct Params<'a> {
    pub(super) sketch: &'a Sketch,
}

impl Params<'_> {
    /// Size of the vector.
    ///
    /// Counts positions rather than what is in them: a removed point keeps its
    /// two entries, so every surviving handle keeps indexing where it did.
    pub(super) fn count(self) -> usize {
        self.radius_base() + self.sketch.circles.slot_count()
    }

    /// Where the radii start, which is the boundary the whole layout turns on.
    fn radius_base(self) -> usize {
        self.sketch.points.slot_count() * 2
    }

    /// Where `param` sits. Inverse of [`Params::at`].
    fn index_of(self, param: Param) -> usize {
        match param {
            Param::Point(id, Axis::X) => id.slot() * 2,
            Param::Point(id, Axis::Y) => id.slot() * 2 + 1,
            Param::Radius(id) => self.radius_base() + id.slot(),
        }
    }

    /// What the parameter at `index` names, or `None` where a removal left a
    /// hole. Inverse of [`Params::index_of`].
    ///
    /// An index alone can't name anything: rebuilding a handle needs the
    /// generation of the position, which only the store knows — and a freed
    /// position has no handle to give.
    pub(super) fn at(self, index: usize) -> Option<Param> {
        debug_assert!(
            index < self.count(),
            "parameter {index} is past the {} this sketch has",
            self.count()
        );
        if index < self.radius_base() {
            let axis = if index.is_multiple_of(2) {
                Axis::X
            } else {
                Axis::Y
            };
            Some(Param::Point(
                self.sketch.points.id_at_slot(index / 2)?,
                axis,
            ))
        } else {
            let slot = index - self.radius_base();
            Some(Param::Radius(self.sketch.circles.id_at_slot(slot)?))
        }
    }

    /// Where a point's two parameters sit, x first.
    ///
    /// Both through [`Params::index_of`], so the layout states where each axis
    /// lives rather than a caller deriving one from the other.
    pub(super) fn of_point(self, id: PointId) -> [usize; 2] {
        [
            self.index_of(Param::Point(id, Axis::X)),
            self.index_of(Param::Point(id, Axis::Y)),
        ]
    }

    pub(super) fn of_radius(self, id: CircleId) -> usize {
        self.index_of(Param::Radius(id))
    }

    /// What one parameter currently reads, and nought for a position a removal
    /// left empty — which is what [`Params::write`] puts there too.
    ///
    /// Beside `write` rather than instead of it: a run wants the whole vector
    /// and takes it in one walk, where a caller checking whether a drag moved
    /// what it had hold of wants two or three of them and should not walk a
    /// sketch to find out.
    pub(super) fn value_at(self, index: usize) -> f64 {
        self.at(index).map_or(0.0, |param| param.value(self.sketch))
    }

    /// Whether the solver may move this parameter. Radii always move; point
    /// coordinates move unless the point is fixed; a hole left by a removal
    /// never moves, which is what keeps the solver off it without the solver
    /// having to know holes exist.
    pub(super) fn is_free(self, index: usize) -> bool {
        match self.at(index) {
            Some(Param::Point(id, _)) => !self.sketch.point(id).fixed,
            Some(Param::Radius(_)) => true,
            None => false,
        }
    }

    /// Append every parameter's current value, in index order.
    ///
    /// With [`Sketch::set_params`], the pair a solve works through: the vector
    /// is the whole of what a step can have touched, so saving it and putting it
    /// back is how a step is undone.
    ///
    /// Appends rather than replacing, so the caller owns the buffer — which is
    /// what lets a solver kept across a drag keep one instead of being handed a
    /// fresh one every time.
    ///
    /// Reads zero at a hole, which is a value nothing will move: its column is
    /// zeroed and its step is pinned to zero, so the number is never used.
    /// Through [`Params::value_at`], so that rule is stated once — the two have
    /// to agree about what a hole reads as, and the cheapest way to agree is to
    /// be the same code.
    pub(super) fn write(self, out: &mut Vec<f64>) {
        let count = self.count();
        out.reserve_exact(count);
        out.extend((0..count).map(|index| self.value_at(index)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(sketch.params().count(), 8);
        assert_eq!(sketch.params().of_point(a), [0, 1]);
        assert_eq!(sketch.params().of_point(b), [2, 3]);
        assert_eq!(sketch.params().of_point(c), [4, 5]);
        assert_eq!(sketch.params().of_radius(inner), 6);
        assert_eq!(sketch.params().of_radius(outer), 7);

        // The round trip is what keeps the forward map and the reverse lookup
        // in step: break either and some index stops coming back as itself.
        for index in 0..sketch.params().count() {
            let param = sketch.params().at(index).expect("nothing has been removed");
            assert_eq!(sketch.params().index_of(param), index, "{index}");
        }
        assert_eq!(sketch.params().at(0), Some(Param::Point(a, Axis::X)));
        assert_eq!(sketch.params().at(3), Some(Param::Point(b, Axis::Y)));
        assert_eq!(sketch.params().at(6), Some(Param::Radius(inner)));
        assert_eq!(sketch.params().at(7), Some(Param::Radius(outer)));

        // Only b is pinned, so only its two coordinates are held. Radii move
        // whatever the points do.
        let free: Vec<bool> = (0..sketch.params().count())
            .map(|index| sketch.params().is_free(index))
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
        assert_eq!(sketch.params().count(), 5);

        // Nothing is built on it — the circle is centred on `b` — so the cascade
        // has nothing to take and this is the bare removal.
        sketch.remove_point(a);

        assert_eq!(sketch.params().count(), 5);
        assert_eq!(sketch.params().at(0), None);
        assert_eq!(sketch.params().at(1), None);
        assert_eq!(sketch.params().at(2), Some(Param::Point(b, Axis::X)));
        assert_eq!(sketch.params().at(4), Some(Param::Radius(circle)));
        assert_eq!(sketch.params().of_point(b), [2, 3]);
        assert_eq!(sketch.params().of_radius(circle), 4);

        // The hole is unfree, which is the whole of what the solver needs: it
        // already pins a parameter it may not move and zeroes that column.
        let free: Vec<bool> = (0..5).map(|index| sketch.params().is_free(index)).collect();
        assert_eq!(free, [false, false, true, true, true]);

        // It reads zero and refuses to be written, so a step landing on it
        // changes nothing.
        let mut params = Vec::new();
        sketch.params().write(&mut params);
        assert_eq!(params, [0.0, 0.0, 3.0, 4.0, 0.5]);
        sketch.set_params(&[9.0; 5]);
        params.clear();
        sketch.params().write(&mut params);
        assert_eq!(params, [0.0, 0.0, 9.0, 9.0, 9.0]);

        // The freed position is filled again rather than the vector widening,
        // and the handle to what was there is refused, not answered.
        let c = sketch.add_point(DVec2::new(5.0, 6.0));
        assert_eq!(sketch.params().count(), 5);
        assert_eq!(sketch.params().of_point(c), [0, 1]);
        assert_ne!(c, a);
        assert_eq!(sketch.points().count(), 2);
    }
}
