//! One curve bounding a region, and which side of it the region lies on.

use crate::sketch::entity::Entity;

/// One of a region's bounding curves, together with the side the region is on.
///
/// What a face is *named* by, where its position among the faces is only where
/// it happened to be walked. See
/// [`Arrangement::bounds`](super::Arrangement::bounds), which mints these, and
/// [`Arrangement::face_named_by`](super::Arrangement::face_named_by), which
/// reads them back.
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
    /// The far side of the same curve.
    pub(crate) fn turned(self) -> Self {
        Self {
            along: !self.along,
            ..self
        }
    }
}
