//! A piece of a sketch's geometry, whichever of the three kinds it is.

use crate::sketch::{CircleId, PointId, SegmentId};

/// A piece of geometry in a [`Sketch`](crate::Sketch).
///
/// One type over the three, for every caller that holds a piece of geometry
/// without yet knowing which kind it is: what a [`Constraint`] is about, what a
/// removal has to cascade to, what a cursor is over, what is picked out. Each of
/// those would otherwise carry its own three-way enum, and they would all be
/// this one.
///
/// Geometry only. A constraint never names another constraint, so nothing here
/// does either — which is what makes a removal cascade terminate: constraints
/// are the leaves.
///
/// [`Constraint`]: crate::Constraint
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Entity {
    Point(PointId),
    Segment(SegmentId),
    Circle(CircleId),
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
