//! One nameable thing in a sketch, whichever kind it is.

use crate::sketch::constraint::ConstraintId;
use crate::sketch::{CircleId, PointId, SegmentId};

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
