//! What a pick found.

use crate::tag::Tag;
use glam::Vec3;
use std::cmp::Ordering;

/// Where on a primitive a pick landed.
///
/// A marker has no interior, so there is nothing to say beyond that it was
/// hit, and a label is the same — what a caller does with one is about the
/// whole run. A stroke does have an interior: which of its segments, and how
/// far along.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HitAt {
    Point,
    /// A run of text, anywhere within the box it is drawn in.
    Text,
    Segment {
        /// Index into the curve's segments, counting the closing one last.
        index: usize,
        /// Where along that segment, from 0 at its start to 1 at its end.
        t: f32,
    },
    Ring {
        /// Radians round from the ring's own `x_axis`.
        angle: f32,
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
    ///
    /// A label sits between the two. It is a larger target than a marker, so a
    /// marker drawn over one still wins; but it is an opaque thing deliberately
    /// placed, and the edge it labels usually runs right under it — a dimension
    /// sits on its own dimension line — so an edge crossing a label must not
    /// take the click meant for the label.
    pub(crate) fn rank(&self) -> u8 {
        match self {
            Self::Point => 0,
            Self::Text => 1,
            // An edge is an edge however it curves, so a stroke and a rim rank
            // together and the cursor's distance decides between them.
            Self::Segment { .. } | Self::Ring { .. } => 2,
        }
    }
}

/// One primitive the cursor was near enough to.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Hit {
    /// The [`Tag`] the primitive was named with. Untagged primitives are
    /// scenery and never appear here.
    pub tag: Tag,
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

impl Hit {
    /// Which of two hits the aim was more likely meant for, lowest first.
    ///
    /// How specific the hit is, then how near the cursor it fell, then how
    /// near the eye. Stated once because two queries order by it — the whole
    /// list and the single nearest — and an ordering that disagreed between
    /// them would make the first of one differ from the answer of the other.
    pub(crate) fn aim_order(&self, other: &Self) -> Ordering {
        self.at
            .rank()
            .cmp(&other.at.rank())
            .then(self.screen.total_cmp(&other.screen))
            .then(self.distance.total_cmp(&other.distance))
    }
}
