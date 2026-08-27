//! One curve bounding a region, and which side of it the region lies on.

use crate::sketch::entity::Entity;

/// One of a region's bounding curves, together with the side the region is on.
///
/// What a face is *named* by, where its position among the faces is only where
/// it happened to be walked. See
/// [`Face::named`](super::face::Face::named), which is where a region's own
/// are kept, and [`Arrangement::face_named_by`](super::Arrangement::face_named_by),
/// which reads them back.
///
/// **The side is what makes it a name.** Two halves of a cut circle are bounded
/// by the same circle and the same chord, so the curves alone tell them apart
/// not at all; which way each half walks the chord between them does.
///
/// A whole curve rather than one piece of one. A curve cut into several pieces
/// by whatever crosses it bounds a face with all of those pieces or with none,
/// so naming the pieces would move the name every time something new crossed
/// the drawing — which is the failure a position already has, and the one this
/// exists to be free of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bound {
    /// The segment or circle this is a side of.
    ///
    /// Never a point or a relation. Neither is ever cut into an edge, so
    /// neither can reach here — but [`Entity`] is what an edge already says it
    /// came from, and a narrower type would be a conversion at every crossing
    /// of the boundary to buy an arm nothing can construct.
    pub of: Entity,
    /// Whether the region's own walk runs along the curve's direction — a
    /// segment from its first end towards its second, a circle counterclockwise.
    pub along: bool,
}

impl Bound {
    /// A number to gather bounds by: which curve, and which side of it.
    ///
    /// **Exact, not a digest.** Every part of a bound reaches it and nothing
    /// else does, so two bounds share this number exactly when they are one —
    /// which is what lets a caller group by it and never compare again.
    ///
    /// Ordered as well as compared, so that the pieces of one curve fall
    /// together in a sort and a region bounded by a hundred curves is described
    /// by walking runs instead of searching for them. The side is the low bit,
    /// which puts the far side of a curve one flip away — and a spur is exactly
    /// a curve whose far side is there too.
    pub(crate) fn key(self) -> u64 {
        // A slot is a `u32` shifted up by the side bit, so it stops short of
        // the thirty-fourth: each kind fills its own band of the range and none
        // can reach into another's.
        let (kind, slot) = match self.of {
            Entity::Point(id) => (0u64, id.slot()),
            Entity::Segment(id) => (1, id.slot()),
            Entity::Circle(id) => (2, id.slot()),
            Entity::Constraint(id) => (3, id.slot()),
        };
        (kind << 33) | ((slot as u64) << 1) | u64::from(self.along)
    }
}
