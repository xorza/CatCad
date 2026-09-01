//! What a primitive is *for*, as a pick weighs it.

/// How a primitive stands in the competition for a click, foremost first.
///
/// How specific a [`HitAt`](crate::HitAt) is answers what *kind of shape* was
/// hit, which is true of any drawing: a marker is a harder thing to aim at than
/// an edge, wherever either was drawn. This answers what the thing is *for*,
/// which only whoever drew it knows — a frame around a drawing and an edge of
/// one are the same shape, and no amount of care about shape tells them apart.
///
/// A category rather than a number of steps. There is no continuum here to
/// measure along, the way there is for the depth bias an overlay carries: what
/// a primitive is for is one of a few things, and a number would only be
/// readable against the ranking this crate keeps over shapes — see
/// [`HitAt`](crate::HitAt), which is its own business and no caller's.
///
/// The order below is the order they compete in, and the derive is what makes
/// that true: nothing adds, subtracts or compares against a table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum Precedence {
    /// Ranked by shape alone, which is the usual thing and what everything is
    /// unless it says otherwise.
    #[default]
    Shaped,
    /// Behind whatever is being worked on. Drawn to be seen and read rather
    /// than aimed at, so it yields a click to anything ordinary under the
    /// cursor.
    Aside,
    /// Behind everything: furniture around a drawing rather than part of one.
    ///
    /// Behind every drawn thing, that is — not behind the surfaces they are
    /// drawn on. A surface is never ranked against a drawn thing at all, so
    /// this says nothing about one: see how a pick orders what it found.
    Frame,
}
