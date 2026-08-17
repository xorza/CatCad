//! How much of one thing a sketch's constraints leave undecided.
//!
//! The unit the answer is given in. Whose freedom is whose is
//! [`Outcome`](crate::Outcome)'s, which holds one of these per entity.

/// How many independent ways an entity can still move.
///
/// Counted as directions rather than as coordinates, so the answer does not
/// turn on which axes the sketch happens to be drawn against: a point sliding
/// along a diagonal is as constrained as one sliding along `x`, and both read
/// [`Freedom::Partly`].
///
/// Ordered by how much is left, so the weaker of two answers is their maximum
/// — which is what rolls a segment's two ends up into one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Freedom {
    /// Nowhere left to go. Its constraints admit exactly one answer, so a drag
    /// on it can only be refused.
    Determined,
    /// On a track: it moves, but every way it can move is the same way. A point
    /// on a line, or one on a circle of stated radius — the second travels in
    /// both coordinates at once and is no freer for it, since a cursor that
    /// leaves the circle is still asking for the impossible.
    ///
    /// About the entity, not the sketch: a rigid arm has three degrees of
    /// freedom between them, and every point of it is [`Freedom::Free`].
    Partly,
    /// Free to be put wherever it is asked for.
    Free,
}
