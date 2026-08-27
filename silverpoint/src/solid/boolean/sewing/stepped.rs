//! One vertex of a loop being sewn, and what leaves it.

use crate::solid::topology::vertex::VertexId;

/// One vertex of one loop, and what the stretch leaving it runs along.
///
/// **One buffer rather than two kept in step**, which is the same argument
/// `Corner` makes one stage earlier: the walk is truncated, popped and
/// reversed in four places, and a second list beside it would be four chances
/// to do one and forget the other.
///
/// The mark cannot be worked out from the vertices later, which is why it is
/// carried at all: by the time an edge is made, the flattened corners it was
/// collapsed out of are gone, and two vertices standing on one circle say
/// nothing about which of the two arcs between them is the edge — or whether
/// the edge is an arc at all.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Stepped {
    pub(super) vertex: VertexId,
    pub(super) along: Runs,
}

/// What the stretch leaving one vertex of a loop runs along.
///
/// `Came` with the arc's *extent* filled in, which is the one thing a mark
/// cannot carry and an edge cannot do without: two places on a circle say
/// nothing about which of the two ways round between them the edge goes, and
/// the corners that would have said were dropped on the way here. Worked out
/// while they are still to hand — see `swept`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) enum Runs {
    /// Straight to the next vertex.
    Straight,
    /// Along the imprint at this index, over these parameters — see
    /// `Edge::bounds`, whose convention this is: a start and a finish, the
    /// second free to be the smaller where the walk runs backwards.
    Arc { run: u32, bounds: [f64; 2] },
}
