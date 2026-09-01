//! Which piece of a drawing each corner belongs to.

use crate::groups::Groups;
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
    /// Which corners the edges join, collapsed as the fill goes — see
    /// [`Groups`].
    groups: Groups,
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
        let Self { groups, joined } = self;
        groups.apart(corners.len());
        for edge in edges {
            groups.join(edge.from, edge.to);
        }
        // Read out once here rather than asked of the groups per question: what
        // a caller wants is a number to compare, and reading one back would
        // want the groups mutably for the flattening.
        joined.clear();
        joined.reserve_exact(corners.len());
        joined.extend((0..corners.len()).map(|at| groups.of(at)));
    }

    /// Which piece `corner` ended up in.
    pub(super) fn of(&self, corner: usize) -> usize {
        self.joined[corner]
    }
}
