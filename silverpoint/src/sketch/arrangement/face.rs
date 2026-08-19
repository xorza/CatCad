//! One region a drawing shuts in.

use crate::loops::Loops;
use crate::sketch::arrangement::bound::Bound;
use crate::sketch::arrangement::edge::Half;

/// One region the drawing shuts in.
///
/// Reused across rebuilds rather than stood up afresh, like everything else
/// here: every list below is emptied and written over, so a face that comes
/// back the same shape it was costs nothing. See
/// [`Arrangement::rebuild`](super::Arrangement::rebuild).
///
/// What bounds it is worked out by the rebuild and kept, rather than derived
/// whenever it is asked for. It is a property of the drawing's topology, so it
/// moves only when the drawing is cut differently — where the callers ask per
/// solid, per face, per frame.
#[derive(Debug, Default)]
pub struct Face {
    pub(super) outline: Vec<Half>,
    /// What is missing from it — the loops of any drawing that sits inside this
    /// face without touching it.
    pub(super) holes: Loops<Half>,
    /// How much the outline shuts in, with the holes taken out.
    pub(super) area: f64,
    /// What the outline is bounded by — see [`Face::named`].
    pub(super) named: Vec<Bound>,
}

impl Face {
    /// How much of the plane this face covers, holes taken out.
    pub fn area(&self) -> f64 {
        self.area
    }

    /// What bounds the outside of it, each curve once and with the side it lies
    /// on — the name anything built on the region keeps.
    ///
    /// The answer to what a *profile* is. A face's position among the faces
    /// holds only while the drawing's topology does — see
    /// [`Arrangement::faces`](super::Arrangement::faces) — so a feature that
    /// remembered "face 3" would silently build on another region the first
    /// time an edge was added upstream. This holds instead: the curves are the
    /// sketch's own handles, which survive being moved and being cut, and the
    /// side is what tells two regions bounded by the same curves apart. Read
    /// back by [`Arrangement::face_named_by`](super::Arrangement::face_named_by).
    ///
    /// The outline alone, where the whole edge of the region takes in its holes
    /// as well — and which of the two names it is a real choice rather than a
    /// convenience. It is the outline, because a hole appearing or vanishing
    /// changes what the region is *like* without changing which region it is.
    pub fn named(&self) -> &[Bound] {
        &self.named
    }

    /// The loop around the outside of it.
    pub(crate) fn outline(&self) -> &[Half] {
        &self.outline
    }

    /// The loop around each hole punched out of it.
    pub(crate) fn punched(&self) -> impl Iterator<Item = &[Half]> + Clone {
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
