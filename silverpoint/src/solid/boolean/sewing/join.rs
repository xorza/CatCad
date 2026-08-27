//! One edge of the body being sewn, and one loop's use of it.

use crate::solid::boolean::sewing::stepped::Runs;
use crate::solid::topology::face::FaceId;
use crate::solid::topology::vertex::VertexId;
use glam::DVec3;

/// One edge as it is being found.
#[derive(Debug)]
pub(super) struct Join {
    pub(super) ends: [VertexId; 2],
    /// A place halfway along it, which is what tells two edges between one
    /// pair of vertices apart.
    ///
    /// **Two arcs of a circle share both their ends**, and a bore's rim is
    /// exactly that: the block's face and the bore's wall each walk the circle
    /// in two pieces, and the two pieces run between the same two vertices. Read
    /// by their ends alone they are one edge claimed four times, which closes
    /// nothing and reads as a body that will not sew. For a straight edge the
    /// middle follows from the ends, so this changes nothing there — which is
    /// why it is the rule for every edge rather than a case for round ones.
    pub(super) middle: DVec3,
    /// What the edge runs along — see [`Runs`].
    pub(super) along: Runs,
    /// The faces that have claimed it. Exactly two by the end, or the regions
    /// did not close and there is no body to be had.
    pub(super) between: [Option<FaceId>; 2],
    /// How many have claimed it, which is not how many are recorded above: a
    /// third face reaching for an edge two already share has nowhere to be put
    /// and is exactly the failure this counts.
    pub(super) claims: usize,
}

/// One step of one loop: the edge it walks and which way.
#[derive(Debug, Clone, Copy)]
pub(super) struct Step {
    pub(super) join: usize,
    pub(super) forward: bool,
}
