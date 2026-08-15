//! One region a drawing shuts in.

use crate::loops::Loops;
use crate::sketch::arrangement::edge::Half;

/// One region the drawing shuts in.
///
/// Reused across rebuilds rather than stood up afresh, like everything else
/// here: both lists below are emptied and written over, so a face that comes
/// back the same shape it was costs nothing. See
/// [`Arrangement::rebuild`](super::Arrangement::rebuild).
#[derive(Debug, Default)]
pub struct Face {
    pub(super) outline: Vec<Half>,
    /// What is missing from it — the loops of any drawing that sits inside this
    /// face without touching it.
    pub(super) holes: Loops<Half>,
    /// How much the outline shuts in, with the holes taken out.
    pub(super) area: f64,
}

impl Face {
    /// How much of the plane this face covers, holes taken out.
    pub fn area(&self) -> f64 {
        self.area
    }

    /// The loop around the outside of it.
    pub(super) fn outline(&self) -> &[Half] {
        &self.outline
    }

    /// The loop around each hole punched out of it.
    pub(super) fn punched(&self) -> impl Iterator<Item = &[Half]> {
        self.holes.iter()
    }
}

#[cfg(test)]
mod counting {
    use crate::sketch::arrangement::face::Face;

    impl Face {
        /// How many holes are punched out of it.
        ///
        /// Nothing outside the tests wants a *count*: a caller holding a face
        /// fills it, and the fill has the holes cut out of it already. What
        /// asserts how many there are is the sweep next door, where a hole
        /// landing in the wrong face is the failure being looked for.
        pub(crate) fn holes(&self) -> usize {
            self.holes.len()
        }
    }
}
