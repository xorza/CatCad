//! A corner of a body.

use crate::arena::Id;
use glam::DVec3;

pub(crate) type VertexId = Id<Vertex>;

/// One place several edges and faces meet.
#[derive(Debug)]
pub(crate) struct Vertex {
    /// Where it stands.
    ///
    /// The truth today, and a cache tomorrow: the design this is built towards
    /// holds the surfaces whose intersection the vertex *is* and re-evaluates
    /// them exactly whenever a rounding cannot decide something — see
    /// `.notes/KERNEL.md` §4.2. Nothing above here reads the position for
    /// anything but display and checking, so that swap reaches no caller.
    pub(crate) at: DVec3,
    /// The radius of the ball this vertex stands for.
    ///
    /// Parasolid's sphere. Zero for everything an exact construction raises,
    /// and widened by whatever admits a coincidence into it — see
    /// [`slack`](crate::number::predicate::slack). The top rung of the
    /// ladder in `.notes/KERNEL.md` §4.3: no edge or face meeting here may
    /// claim to be looser than this.
    pub(crate) tolerance: f64,
}
