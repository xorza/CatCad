//! A sketch taken down as a value that can be put back.

use crate::sketch::Sketch;

/// A whole sketch as it stood at one moment.
///
/// Opaque on purpose. What it holds is a sketch, and a caller able to reach
/// inside would be a caller holding a second sketch it could edit — which is
/// the one thing a record of the past must not be. Neither thing a snapshot is
/// for needs one: put a sketch back the way it was, and tell whether it has
/// changed since.
///
/// The whole sketch rather than the solver's parameter vector, because
/// otherwise there is nothing a snapshot could say about a step that *added*
/// geometry: parameters are named by position, so one taken before a point was
/// added names the wrong ones afterwards, and undoing the addition would need
/// to put back something the record does not contain.
///
/// Equality answers the second question, and answers it exactly. Positions come
/// back through the same arithmetic that wrote them, so a sketch nothing moved
/// compares equal rather than nearly equal — which is what lets a caller record
/// an edit only where there was one. A drag with nowhere to go reads as the
/// nothing it was because [`Solver::drag`](crate::Solver) hands back the
/// parameters it was given, not because anything here is approximate.
#[derive(Debug, Default, PartialEq)]
pub struct Snapshot {
    pub(super) sketch: Sketch,
}

// Written out for `clone_from`, as [`Sketch`]'s own is — see the note there. A
// history extending an open step rewrites its far end every frame the gesture
// lasts, and does it through this.
impl Clone for Snapshot {
    fn clone(&self) -> Self {
        Self {
            sketch: self.sketch.clone(),
        }
    }

    fn clone_from(&mut self, source: &Self) {
        self.sketch.clone_from(&source.sketch);
    }
}
