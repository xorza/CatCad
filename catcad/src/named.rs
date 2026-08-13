//! What a pick reports, turned back into the sketch entity it came from.

use aperture::Tag;
use silverpoint::{CircleId, PointId, SegmentId};

/// A sketch entity a drawn primitive stands for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Named {
    Point(PointId),
    Segment(SegmentId),
    Circle(CircleId),
}

/// Every entity the drawing named, in the order it was drawn.
///
/// A [`Tag`] is one opaque `u64`, and a sketch handle no longer fits in one:
/// [`Id`](silverpoint::Id) spends a `u32` on the slot and another on the
/// generation, leaving nothing to say which of the three kinds it is. Packing
/// would mean stealing bits from a counter and teaching two crates the same
/// bit layout.
///
/// So the tag is an index into this instead. It costs a rebuild whenever the
/// drawing is rebuilt — which is the same moment the entities themselves would
/// have moved — and in exchange nothing has to agree about anything but the
/// order things were pushed in.
#[derive(Debug, Clone, Default)]
pub struct Names {
    entities: Vec<Named>,
}

impl Names {
    /// Name `entity`, and hand back the tag that will report it.
    pub fn tag(&mut self, entity: Named) -> Tag {
        self.entities.push(entity);
        Tag::new((self.entities.len() - 1) as u64)
    }

    /// What `tag` was given to, or `None` if it came from a drawing older than
    /// this one.
    pub fn get(&self, tag: Tag) -> Option<Named> {
        self.entities.get(usize::try_from(tag.get()).ok()?).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.entities.is_empty()
    }
}
