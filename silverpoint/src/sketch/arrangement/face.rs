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
    pub(crate) fn outline(&self) -> &[Half] {
        &self.outline
    }

    /// The loop around each hole punched out of it.
    pub(crate) fn punched(&self) -> impl Iterator<Item = &[Half]> + Clone {
        self.holes.iter()
    }

    /// Every loop around it: the outline, then one per hole.
    ///
    /// The whole edge of the region, where [`Face::outline`] is only its outer
    /// edge — and which of the two a caller wants is a real choice rather than a
    /// convenience. What *names* a region is its outline, because a hole
    /// appearing or vanishing changes what the region is like without changing
    /// which region it is. What a region has *walls* on is this: a bore carried
    /// off the plane is as much a face of the solid as its outside.
    ///
    /// [`Clone`], because the rules that read it have to walk it twice — see
    /// [`Arrangement::bounding`](super::Arrangement).
    pub(crate) fn boundary(&self) -> impl Iterator<Item = &[Half]> + Clone {
        std::iter::once(self.outline()).chain(self.punched())
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
