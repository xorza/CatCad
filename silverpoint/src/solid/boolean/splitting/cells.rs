//! The regions one plane is cut into.

use crate::loops::Loops;
use crate::math::bounds::Bounds;
use crate::solid::boolean::splitting::corner::Corner;
use glam::DVec2;
use std::ops::Range;

/// Regions of one plane, each an outline and the loops punched out of it.
///
/// Flat, like everything else the kernel holds several of: one buffer of loops
/// and a range per region.
///
/// **Cut in place.** A cut divides almost none of what it is handed — a hundred
/// and twenty-eight walls leave a block's face in as many slices and the next
/// wall crosses two — so a store the cut copied wholesale would spend its time
/// carrying regions past cuts that miss them. What a cut moves instead is a
/// range and a box per region it keeps, and the corners stay where they were
/// written. See [`Splitting::split`](super::Splitting).
///
/// **The loops of a region it divided are left behind**, their runs no longer
/// named by any region. That is the room a cut in place costs, and it comes to
/// about what the face is finally cut into: the *k*th line across a face
/// already in *k* pieces crosses about *k* of them, so what every cut together
/// divides is of the order of what the last one leaves.
#[derive(Debug, Default)]
pub(crate) struct Cells {
    loops: Loops<Corner>,
    /// Which runs each region owns: the outline first, then its holes.
    owned: Vec<Range<usize>>,
    /// The box each region's outline fills, which is the region's own — a hole
    /// stands inside the outline that holds it.
    ///
    /// Held rather than measured, for the reason
    /// [`Bounds`] gives: the corners are walked to
    /// be written anyway, and folding four floats out of that walk costs
    /// nothing beside it. What reads them is the cut that comes next, which
    /// asks of every region whether it is worth walking at all.
    fills: Vec<Bounds<DVec2>>,
}

impl Cells {
    pub(crate) fn clear(&mut self) {
        self.loops.clear();
        self.owned.clear();
        self.fills.clear();
    }

    /// How many regions there are.
    pub(crate) fn len(&self) -> usize {
        self.owned.len()
    }

    /// Every loop of the region at `at`, the outline first.
    pub(crate) fn cell(&self, at: usize) -> impl Iterator<Item = &[Corner]> + Clone {
        self.owned[at].clone().map(|run| self.loops.get(run))
    }

    /// Add a region, its loops written by `write` — the outline first.
    ///
    /// A region that writes no loop is no region, and is dropped rather than
    /// recorded: a cut that keeps nothing has to leave nothing behind, or every
    /// reader afterwards has to know about regions that are not there.
    pub(crate) fn add(&mut self, write: impl FnOnce(&mut Loops<Corner>)) {
        let from = self.loops.len();
        write(&mut self.loops);
        if self.loops.len() > from {
            let fills = self
                .loops
                .get(from)
                .iter()
                .map(|corner| corner.at)
                .collect();
            self.owned.push(from..self.loops.len());
            self.fills.push(fills);
        }
    }

    /// The box the region at `at` fills.
    pub(crate) fn fills(&self, at: usize) -> Bounds<DVec2> {
        self.fills[at]
    }

    /// Move the region at `from` down to `to`, which stands at or before it.
    ///
    /// **The range and the box, and not the corners.** What a region is made of
    /// stays where it was written — see the note above, which is the whole
    /// reason a cut walks its regions rather than copying them.
    pub(crate) fn carry(&mut self, from: usize, to: usize) {
        debug_assert!(to <= from, "a region cannot be carried forwards");
        self.owned[to] = self.owned[from].clone();
        self.fills[to] = self.fills[from];
    }

    /// Forget every region past `len`, keeping the loops they were written
    /// into.
    pub(crate) fn truncate(&mut self, len: usize) {
        self.owned.truncate(len);
        self.fills.truncate(len);
    }
}

#[cfg(test)]
mod internals {
    use crate::solid::boolean::splitting::cells::Cells;
    use crate::solid::boolean::splitting::corner::Corner;

    impl Cells {
        /// The outline of the region at `at`.
        pub(crate) fn outline(&self, at: usize) -> &[Corner] {
            self.loops.get(self.owned[at].start)
        }
    }
}
