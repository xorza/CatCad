//! A walk over the faces of a body, stepping across shared edges.

use crate::solid::topology::Topology;
use crate::solid::topology::face::FaceId;

/// Which faces a walk across shared edges reaches, and the room it takes.
///
/// **The one definition of what a shell is.** A shell is whatever a walk from
/// one of its faces reaches, and both readers of that — the sewing that gathers
/// faces into shells, and the check that a shell is one connected sheet rather
/// than several put in the same list — ask it here. Written twice they could
/// disagree, and a body built to one rule and checked against the other is a
/// body whose check says nothing.
///
/// Kept across calls, buffers and all: sewing runs on the path a drag takes,
/// where a walk that asked for its side tables again would be the only thing on
/// that path reaching the heap. Indexed by arena slot rather than hashed, like
/// every side table in this crate.
#[derive(Debug, Default)]
pub(crate) struct Spreading {
    /// Which faces the walk has taken in, by slot.
    standing: Vec<bool>,
    waiting: Vec<FaceId>,
    reached: Vec<FaceId>,
}

impl Spreading {
    /// Forget every face taken in, and make room for the ones `topology` holds.
    ///
    /// Apart from [`Spreading::across`] because the two questions are
    /// different: a caller cutting a whole body into shells walks it once per
    /// shell and wants each face taken in by exactly one of those walks, and a
    /// caller asking about one shell alone wants a clean sheet.
    pub(crate) fn restart(&mut self, topology: &Topology) {
        self.standing.clear();
        self.standing.resize(topology.face_slots(), false);
    }

    /// Whether a walk since the last [`Spreading::restart`] has taken `face`
    /// in.
    pub(crate) fn standing(&self, face: FaceId) -> bool {
        self.standing[face.slot()]
    }

    /// Every face reachable from `face` by stepping across shared edges, less
    /// whatever an earlier walk since the last [`Spreading::restart`] took in.
    ///
    /// **`face` must not be standing already.** A walk out of a face some
    /// earlier walk reached answers a truncated set, because every neighbour
    /// that walk took in is standing too.
    pub(crate) fn across(&mut self, topology: &Topology, face: FaceId) -> &[FaceId] {
        debug_assert!(!self.standing(face), "{face:?} was reached already");
        self.reached.clear();
        self.waiting.clear();
        self.waiting.push(face);
        self.standing[face.slot()] = true;
        while let Some(here) = self.waiting.pop() {
            self.reached.push(here);
            for coedge in topology.loops_of(topology.face(here)).flatten() {
                for across in topology.edge(coedge.edge).between {
                    if !std::mem::replace(&mut self.standing[across.slot()], true) {
                        self.waiting.push(across);
                    }
                }
            }
        }
        &self.reached
    }
}
