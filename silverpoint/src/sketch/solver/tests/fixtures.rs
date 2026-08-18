//! The drawings every test below is asked of, and the tolerances they are
//! read to.

use crate::sketch::constraint::{Constraint, ConstraintId, Dimension};
use crate::sketch::solver::*;
use glam::DVec2;

/// Tight enough that a wrong answer can't hide behind it: the solver's own
/// tolerance is on the residual, and these check the geometry that follows.
pub(super) const EPSILON: f64 = 1e-9;

/// What a *drag* lands within, where [`EPSILON`] is what a solve lands within.
///
/// Looser, and for a stated reason. A solve is run until its residuals vanish;
/// a drag is the settled answer to a tug of war between the cursor and the
/// constraints, so what it costs is the last couple of digits — see
/// `PULL`. Two hundredths of a millionth of a sketch unit, which is five
/// decades under one pixel of the drawing on screen.
pub(super) const DRAGGED: f64 = 1e-7;

/// A fixed anchor at the origin and a free point one along, with a distance
/// stated over the pair.
///
/// The smallest sketch here that has an answer worth naming, and what the two
/// redundancy fixtures below start from.
#[derive(Debug)]
pub(super) struct Apart {
    pub(super) sketch: Sketch,
    pub(super) anchor: PointId,
    pub(super) free: PointId,
    /// The distance itself, for the fixture that states it a second time.
    pub(super) stated: ConstraintId,
}

impl Apart {
    /// The pair stated five apart, which is what every fixture here but one
    /// wants.
    pub(super) fn new() -> Self {
        Self::stating(5.0)
    }

    /// The same pair with the distance said to be `value` instead.
    pub(super) fn stating(value: f64) -> Self {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.0));
        sketch.fix(anchor);
        let stated = sketch.add_constraint(Constraint::apart(anchor, free, value));
        Self {
            sketch,
            anchor,
            free,
            stated,
        }
    }
}

/// An [`Apart`] with its distance stated a second time.
#[derive(Debug)]
pub(super) struct Doubled {
    pub(super) sketch: Sketch,
    pub(super) free: PointId,
    pub(super) stated: [ConstraintId; 2],
}

impl Doubled {
    pub(super) fn new() -> Self {
        let Apart {
            mut sketch,
            anchor,
            free,
            stated,
        } = Apart::new();
        let again = sketch.add_constraint(Constraint::apart(anchor, free, 5.0));
        Self {
            sketch,
            free,
            stated: [stated, again],
        }
    }
}

/// One pair with two distances over it, of one and of two, which cannot both
/// hold.
#[derive(Debug)]
pub(super) struct Conflicting {
    pub(super) sketch: Sketch,
    pub(super) free: PointId,
}

impl Conflicting {
    pub(super) fn new() -> Self {
        let Apart {
            mut sketch,
            anchor,
            free,
            ..
        } = Apart::stating(1.0);
        sketch.add_constraint(Constraint::apart(anchor, free, 2.0));
        Self { sketch, free }
    }
}

/// Three corners of a five-by-three rectangle, the first of them pinned.
///
/// Four free parameters against four independent equations, which is what makes
/// it the large sketch wherever one here is measured against a small one.
#[derive(Debug)]
pub(super) struct Rectangle {
    pub(super) sketch: Sketch,
    pub(super) corner: [PointId; 3],
}

impl Rectangle {
    pub(super) fn new() -> Self {
        let mut sketch = Sketch::default();
        let corner = [
            sketch.add_point(DVec2::ZERO),
            sketch.add_point(DVec2::new(5.1, 0.2)),
            sketch.add_point(DVec2::new(4.9, 3.1)),
        ];
        sketch.fix(corner[0]);
        sketch.add_constraint(Constraint::Horizontal {
            a: corner[0],
            b: corner[1],
        });
        sketch.add_constraint(Constraint::apart(corner[0], corner[1], 5.0));
        sketch.add_constraint(Constraint::Vertical {
            a: corner[1],
            b: corner[2],
        });
        sketch.add_constraint(Constraint::apart(corner[1], corner[2], 3.0));
        Self { sketch, corner }
    }
}

/// A point riding a circle of stated size about a fixed hub.
#[derive(Debug)]
pub(super) struct Orbit {
    pub(super) sketch: Sketch,
    pub(super) rider: PointId,
    pub(super) ring: CircleId,
}

impl Orbit {
    pub(super) fn new() -> Self {
        let mut sketch = Sketch::default();
        let hub = sketch.add_point(DVec2::ZERO);
        let rider = sketch.add_point(DVec2::new(2.0, 0.5));
        sketch.fix(hub);
        let ring = sketch.add_circle(hub, 2.0);
        sketch.add_constraint(Constraint::Radius {
            circle: ring,
            dimension: Dimension::new(2.0),
        });
        sketch.add_constraint(Constraint::PointOnCircle {
            point: rider,
            circle: ring,
        });
        Self {
            sketch,
            rider,
            ring,
        }
    }
}

/// A circle of radius three whose rim passes through a fixed point.
///
/// One drawing whose radius and centre cannot move independently: the rim is
/// held to that point, so the centre stands exactly its radius away — and
/// growing the circle walks the centre unless something holds it.
#[derive(Debug)]
pub(super) struct Pegged {
    pub(super) sketch: Sketch,
    pub(super) centre: PointId,
    pub(super) circle: CircleId,
}

impl Pegged {
    pub(super) fn new() -> Self {
        let mut sketch = Sketch::default();
        let edge = sketch.add_point(DVec2::ZERO);
        sketch.fix(edge);
        let centre = sketch.add_point(DVec2::new(3.0, 0.0));
        let circle = sketch.add_circle(centre, 3.0);
        sketch.add_constraint(Constraint::PointOnCircle {
            point: edge,
            circle,
        });
        Self {
            sketch,
            centre,
            circle,
        }
    }
}

/// An [`Apart`] with a level across it, so the free point has one place to be.
pub(super) fn determined_pair() -> Apart {
    let mut pair = Apart::new();
    pair.sketch.add_constraint(Constraint::Horizontal {
        a: pair.anchor,
        b: pair.free,
    });
    pair
}
