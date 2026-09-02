//! An edge walked one way.

/// One side of an edge — the edge walked in one direction.
///
/// A face is bounded by these rather than by edges, because the same piece of
/// curve bounds a face on either side of it and the two walk it opposite ways.
/// Orientation, adjacency and parameter space all meet here; ACIS's own
/// documentation calls its version of this the glue of most modelers, which is
/// fair.
///
/// One type for both dimensions — [`Half`](crate::sketch::arrangement::edge::Half)
/// in a drawing and [`Coedge`](crate::solid::topology::coedge::Coedge) in a
/// body — because they differ in nothing but how an edge is named. [`Copy`] and
/// stored inline rather than kept in an arena: nothing hangs off one, so there
/// is nothing for a handle to name. See `.notes/KERNEL.md` §4.7 and §4.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Sided<Id> {
    pub(crate) edge: Id,
    /// Whether this walks the edge from its `from` towards its `to`.
    pub(crate) forward: bool,
}

impl<Id: Copy> Sided<Id> {
    /// The same edge walked the other way, which is how the face across it
    /// uses it.
    pub(crate) fn turned(self) -> Self {
        Self {
            edge: self.edge,
            forward: !self.forward,
        }
    }

    /// Walk the run `of` the other way round: reversed, and every step turned
    /// over.
    ///
    /// **The two together and never one of them.** Reversed alone, the run
    /// still walks each edge the way it did and the loop comes back inside out;
    /// turned alone, it walks them the other way in the order they were
    /// written, which is not a loop at all. What every caller wants is the walk
    /// the face across it takes, and that is both.
    ///
    /// Nothing steps round by one here, which is the whole of the difference
    /// from the splitting's own `corner::turned`:
    /// a coedge says which way its own edge is walked and nothing about the
    /// stretch leaving it, so turning the run over is all there is to do.
    pub(crate) fn turn(of: &mut [Self]) {
        of.reverse();
        for step in of.iter_mut() {
            *step = step.turned();
        }
    }
}

impl Sided<usize> {
    /// A position to key this by, so a walk can note where it has been without
    /// a map.
    pub(crate) fn slot(self) -> usize {
        self.edge * 2 + usize::from(self.forward)
    }
}
