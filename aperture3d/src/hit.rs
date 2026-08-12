//! What a pick found.

use glam::Vec3;

/// Where on a primitive a pick landed.
///
/// A marker has no interior, so there is nothing to say beyond that it was
/// hit. A stroke does: which of its segments, and how far along.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitAt {
    Point,
    Segment {
        /// Index into the curve's segments, counting the closing one last.
        index: usize,
        /// Where along that segment, from 0 at its start to 1 at its end.
        t: f32,
    },
}

impl HitAt {
    /// How specific this kind of hit is, lowest first.
    ///
    /// A vertex under the cursor beats an edge through it even when the edge
    /// is nearer the eye, because the smaller thing is the harder one to aim
    /// at and so the one the aim was meant for. Sorting on depth alone would
    /// make the corner of a rectangle unselectable — every corner has two
    /// edges running through it.
    pub(crate) fn rank(&self) -> u8 {
        match self {
            Self::Point => 0,
            Self::Segment { .. } => 1,
        }
    }
}

/// One primitive the cursor was near enough to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The [tag](crate#picking) the primitive was named with. Untagged
    /// primitives are scenery and never appear here.
    pub tag: u64,
    pub at: HitAt,
    /// Where on the primitive, in world space — the marker's own position, or
    /// the point of the stroke nearest the cursor.
    pub world: Vec3,
    /// How far that landed from the cursor on screen, in the units the pick
    /// was asked in.
    pub screen: f32,
    /// How far along the cursor's ray, in world units. Comparable across
    /// primitives and across projections, which is what makes it a usable
    /// tiebreak.
    pub distance: f32,
}
