//! The regions one plane is cut into.

use crate::loops::Loops;
use crate::solid::boolean::splitting::corner::Corner;
use std::ops::Range;

/// Regions of one plane, each an outline and the loops punched out of it.
///
/// Flat, like everything else the kernel holds several of: one buffer of loops
/// and a range per region. A cut reads one of these and writes another, and the
/// two are swapped rather than replaced, so cutting a face by a dozen planes
/// reaches the heap for none of them.
#[derive(Debug, Default)]
pub(crate) struct Cells {
    loops: Loops<Corner>,
    /// Which runs each region owns: the outline first, then its holes.
    owned: Vec<Range<usize>>,
}

impl Cells {
    pub(crate) fn clear(&mut self) {
        self.loops.clear();
        self.owned.clear();
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
            self.owned.push(from..self.loops.len());
        }
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
