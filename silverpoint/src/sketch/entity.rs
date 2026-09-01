//! The three things a drawing is made of, and the one name over all four kinds.

use crate::arena::Id;
use crate::sketch::constraint::ConstraintId;
use glam::DVec2;

/// Handle to a point in a [`Sketch`](crate::Sketch).
pub type PointId = Id<Point>;

/// Handle to a segment in a [`Sketch`](crate::Sketch).
pub type SegmentId = Id<Segment>;

/// Handle to a circle in a [`Sketch`](crate::Sketch).
pub type CircleId = Id<Circle>;

/// A point's position, and whether the solver may move it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Point {
    pub position: DVec2,
    /// The solver leaves it where it is. Anchors the sketch so a
    /// well-constrained system isn't still free to translate and rotate.
    pub fixed: bool,
}

/// A straight edge between two points. Carries no parameters of its own — it
/// is entirely defined by its endpoints.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Segment {
    pub a: PointId,
    pub b: PointId,
}

/// A circle about a point. The radius is a solver parameter, so a
/// [`Constraint::Radius`](crate::Constraint) can pin it or a tangency can drive
/// it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Circle {
    pub center: PointId,
    pub radius: f64,
}

/// Anything in a [`Sketch`](crate::Sketch) that can be named, picked out, or
/// removed.
///
/// One type over the kinds, for every caller that holds one without yet knowing
/// which it is: what a [`Constraint`](crate::Constraint) is about, what a
/// removal has to cascade to, what a cursor is over, what is picked out.
///
/// **Everything nameable, not everything geometric.** A constraint is in here
/// because it is a thing a user selects and deletes. What it is *not* is
/// something other geometry can be built on: `Sketch::add_segment` wants points,
/// not entities, and the handle a caller holds already says which it has.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Point(PointId),
    Segment(SegmentId),
    Circle(CircleId),
    Constraint(ConstraintId),
}

impl Entity {
    /// The point this names, or `None` where it names something else.
    ///
    /// The way back from the [`From`] impls below, and for one shape of caller:
    /// one sifting a mixed run of entities for the single kind it can use.
    /// Walking what a constraint is about is the whole of that today — the run
    /// reads `referents().filter_map(Entity::point)`, where a `match` inside a
    /// loop would say the same thing in four lines and a nesting level.
    ///
    /// Three of these rather than four. Nothing in a sketch names a constraint
    /// — see [`Constraint::referents`](crate::Constraint::referents) — so a
    /// sift for one would have nothing it could ever find.
    pub(crate) fn point(self) -> Option<PointId> {
        match self {
            Entity::Point(id) => Some(id),
            _ => None,
        }
    }

    /// The segment this names, or `None` where it names something else.
    pub(crate) fn segment(self) -> Option<SegmentId> {
        match self {
            Entity::Segment(id) => Some(id),
            _ => None,
        }
    }

    /// The circle this names, or `None` where it names something else.
    pub(crate) fn circle(self) -> Option<CircleId> {
        match self {
            Entity::Circle(id) => Some(id),
            _ => None,
        }
    }
}

// Every handle names itself, so a caller holding one need not say which kind it
// is twice — the type already did. Three impls rather than one, because `Id` is
// generic over what it points at and the three are genuinely different types;
// nothing here could be written once without a trait to write it against.
impl From<PointId> for Entity {
    fn from(id: PointId) -> Self {
        Entity::Point(id)
    }
}

impl From<SegmentId> for Entity {
    fn from(id: SegmentId) -> Self {
        Entity::Segment(id)
    }
}

impl From<CircleId> for Entity {
    fn from(id: CircleId) -> Self {
        Entity::Circle(id)
    }
}

impl From<ConstraintId> for Entity {
    fn from(id: ConstraintId) -> Self {
        Entity::Constraint(id)
    }
}
