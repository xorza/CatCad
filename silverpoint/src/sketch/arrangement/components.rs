//! Which piece of a drawing each corner belongs to.

use crate::sketch::arrangement::edge::Edge;
use glam::DVec2;

/// Which piece of drawing each corner belongs to.
///
/// Two curves are the same piece when a walk along edges gets from one to the
/// other. What this decides is which faces an outside loop may be assigned to —
/// see [`Arrangement::owner_of`](super::Arrangement) — and nothing else.
///
/// Its own type because the two lists below only mean anything together, and
/// only once a fill has run: one is the working state the other is read out of,
/// and neither says anything about a drawing nobody has walked yet.
#[derive(Debug, Default)]
pub(super) struct Components {
    /// Union-find over the corners, which the fill collapses as it goes.
    parent: Vec<usize>,
    /// The piece each corner ended up in.
    joined: Vec<usize>,
}

impl Components {
    /// Work out which piece of the drawing each corner belongs to.
    ///
    /// Takes the corners rather than how many there are, so it reads at a call
    /// site as [`Departures::fill`](super::departures::Departures::fill) beside it
    /// does. Only the count is wanted:
    /// which piece a corner is in follows from what the edges join, not from
    /// where anything lies.
    pub(super) fn fill(&mut self, corners: &[DVec2], edges: &[Edge]) {
        let Self { parent, joined } = self;
        parent.clear();
        parent.reserve_exact(corners.len());
        parent.extend(0..corners.len());
        for edge in edges {
            let (a, b) = (root(parent, edge.from), root(parent, edge.to));
            parent[a] = b;
        }
        joined.clear();
        joined.reserve_exact(corners.len());
        for at in 0..corners.len() {
            joined.push(root(parent, at));
        }
    }

    /// Which piece `corner` ended up in.
    pub(super) fn of(&self, corner: usize) -> usize {
        self.joined[corner]
    }
}

/// Which corner stands for the piece `at` belongs to.
fn root(parent: &mut [usize], mut at: usize) -> usize {
    while parent[at] != at {
        // Halve the path on the way up, which is what keeps the walk from
        // growing into a list.
        parent[at] = parent[parent[at]];
        at = parent[at];
    }
    at
}
