//! A sketch's geometry, taken down as a value that can be put back.

use crate::sketch::Sketch;

/// Where every piece of a sketch's geometry stands, as one value.
///
/// Opaque on purpose. What it holds is the solver's parameter vector, and a
/// caller able to index it would be a caller obliged to know that vector's
/// layout — which is the sketch's own arrangement rather than a promise to
/// anyone. Neither thing a snapshot is for needs one: put a sketch back the way
/// it was, and tell whether it has moved since.
///
/// Equality answers the second, and answers it exactly. The parameters come
/// back through the same arithmetic that wrote them, so a sketch nothing moved
/// compares equal rather than nearly equal — which is what lets a caller record
/// an edit only where there was one, and treat a drag the constraints refused
/// as the nothing it was.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Snapshot {
    /// Every parameter in index order — the layout `Param` states, next door.
    pub(super) at: Vec<f64>,
}

impl Snapshot {
    /// Whether this describes a sketch the size of `sketch`, which is the whole
    /// of what makes it safe to put back.
    ///
    /// A snapshot names parameters by position, so one taken before geometry
    /// was added or removed names the wrong ones. Nothing here can tell *which*
    /// sketch it came from — only that the two are the same width, which is as
    /// much as anything keyed by position can ever say.
    pub(super) fn fits(&self, sketch: &Sketch) -> bool {
        self.at.len() == sketch.param_count()
    }
}
