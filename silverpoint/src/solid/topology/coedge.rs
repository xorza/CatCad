//! One face's use of one edge.

use crate::solid::topology::edge::EdgeId;

/// An edge as one face's loop walks it.
///
/// The same piece of curve bounds a face on either side of it and the two walk
/// it opposite ways, so what a loop is made of is *uses* of edges rather than
/// edges. Every kernel has this and ACIS's own documentation calls it the glue
/// of most modelers, which is fair: orientation, adjacency and parameter space
/// all meet here.
///
/// [`Copy`] and stored inline rather than kept in an arena, exactly as
/// [`Half`](crate::sketch::arrangement::edge::Half) is one dimension down.
/// Nothing hangs off a coedge — no parameter curve, no neighbour pointer — so
/// there is nothing for a handle to name. See `.notes/KERNEL.md` §4.7 and §4.8.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Coedge {
    pub(crate) edge: EdgeId,
    /// Whether this walks the edge from its `from` towards its `to`.
    pub(crate) forward: bool,
}

impl Coedge {
    /// The same edge walked the other way, which is how the face across it
    /// uses it.
    pub(crate) fn turned(self) -> Self {
        Self {
            edge: self.edge,
            forward: !self.forward,
        }
    }
}
