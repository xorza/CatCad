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
/// an edit only where there was one, and treat a drag the constraints refused
/// as the nothing it was.
#[derive(Debug, Default, PartialEq)]
pub struct Snapshot {
    pub(super) sketch: Sketch,
}

impl Snapshot {
    /// Whether this describes a sketch as wide as `sketch` — the same count of
    /// solver parameters, however they are arranged.
    ///
    /// Not what makes a restore safe, which is nothing: a snapshot carries a
    /// whole sketch and can be put into one of any width. It answers the
    /// narrower question [`Solver::edit_holding`](crate::Solver) asks, which is
    /// whether an edit that promised only to *move* geometry kept its word.
    pub(super) fn fits(&self, sketch: &Sketch) -> bool {
        self.sketch.param_count() == sketch.param_count()
    }

    /// Whether `sketch` stands where this says, to within `epsilon` in every
    /// parameter.
    ///
    /// The blunt cousin of the equality above, and the two answer different
    /// questions. Equality asks whether a sketch is the one recorded, and a
    /// caller deciding whether there is a step to take back wants exactly that.
    /// This asks whether it has *materially* moved, which is what a caller
    /// wants after a solve: an answer reached twice by two routes is the same
    /// answer geometrically and rarely the same bits, so a solve that put
    /// everything back where it belonged would otherwise read as a change.
    ///
    /// Every parameter, so a radius counts alongside a position — a drag that
    /// drives one is as much a change as a drag that moves the other.
    pub(super) fn within(&self, sketch: &Sketch, epsilon: f64) -> bool {
        self.fits(sketch)
            && self
                .sketch
                .params()
                .zip(sketch.params())
                .all(|(was, now)| (now - was).abs() <= epsilon)
    }
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
