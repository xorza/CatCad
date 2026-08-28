//! Putting two bodies together, and taking one out of the other.
//!
//! Four stages, each with its two-dimensional precedent already working next
//! door in [`Arrangement`](crate::Arrangement) — see `.notes/KERNEL.md` §7.4.
//! Every face of each body is cut by every surface of the other ([`splitting`])
//! — by the surface and not by the face; each region that falls out is asked
//! where it stands
//! ([`sounding`]); the operator says which of those to keep; and what is kept
//! is sewn back into a body.
//!
//! **Curved as well as flat**, over the exact tier: a face is laid out in its
//! own parameters and a cut is a line or a circle in them, the polyline that
//! comes of it decides which regions to keep, and the *curve* the meeting gave
//! builds the edge. Nothing here is refused for being round. What is refused is
//! a crossing that cannot be written down in a face's own parameters — see
//! `imprinted` — which is a narrower thing and says so where it happens.

use crate::solid::boolean::combining::Combining;
use crate::solid::boolean::operation::Operation;
use crate::solid::boolean::sewing::Sewing;
use crate::solid::topology::body::Body;

mod combining;
mod imprints;
pub(crate) mod operation;
mod sewing;
mod sounding;
mod splitting;

/// Puts two bodies together, keeping the room it works in.
///
/// The public face of the four stages below, and what a caller holds: like
/// [`Builder`](crate::Builder) beside it, one of these is kept for the length
/// of a session rather than stood up per call, because a document is rebuilt on
/// every frame of a drag through the drawing under it and every buffer the
/// stages want comes out the same size each time.
#[derive(Debug, Default)]
pub struct Boolean {
    combining: Combining,
    sewing: Sewing,
}

impl Boolean {
    /// Put `one` and `two` together as `doing` says, into `into`.
    ///
    /// `false`, with `into` emptied, where it will not — and a refusal is an
    /// answer rather than a failure. Four things are refused: a crossing no
    /// face's own parameters can carry, which today means an ellipse or a line
    /// along a cylinder; a result whose regions leave an edge with one face or
    /// three, which two solids meeting along nothing but an edge genuinely do;
    /// one that closes into shells sharing a corner, which two meeting at
    /// nothing but a point genuinely do; and a cavity with more than one lump
    /// to hang it on. Guessing at any of them would hand back something that
    /// reads as a solid and is not.
    pub fn combine(&mut self, one: &Body, two: &Body, doing: Operation, into: &mut Body) -> bool {
        if !self.combining.combine(one, two, doing) {
            into.clear();
            return false;
        }
        self.sewing.sew(self.combining.sewn(), into)
    }
}

#[cfg(test)]
mod tests;
