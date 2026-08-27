//! Walking a curve as a chain of chords.

/// A stretch of curve a walk cuts into chords.
///
/// One rule, in a drawing and in a body alike: an arrangement's half-edge and a
/// body's coedge are the same thing one dimension apart, and how finely either
/// is cut, where the cuts land and which of them are taken from store rather
/// than worked out are the same three answers. Written twice they could drift,
/// and the two are laid end to end against each other every time a profile
/// becomes a solid.
///
/// Each dimension supplies what a place is, how finely its curves are cut,
/// where its stored ends are kept and how to read a curve between them. The two
/// rules a reader can get wrong — which places come from store, and where a
/// walk stops — are here and nowhere else.
pub(crate) trait Chorded {
    /// A place on the curve.
    type At: Copy;

    /// How many chords the curve is worth, flattened no further than `sagitta`
    /// from it.
    ///
    /// Straight is exact however coarsely it is cut, so only a round curve is
    /// asked — see [`arc::chords`](crate::math::arc::chords), which is where
    /// the rule lives.
    fn steps(&self, sagitta: f64) -> usize;

    /// The stored places the walk starts and finishes at, in that order.
    fn ends(&self) -> [Self::At; 2];

    /// Where the `step`th of `steps` lands, both ends left out.
    ///
    /// **At the curve's *own* parameter rather than the walk's.** The two faces
    /// sharing an edge walk it opposite ways, and `start + Δ(n−k)/n` is not the
    /// same arithmetic as `end − Δk/n` — two roundings of one place, which is a
    /// hairline between two faces that are meant to meet.
    fn at(&self, step: usize, steps: usize) -> Self::At;

    /// Where the `step`th of `steps` cuts along the curve lands.
    ///
    /// **The stored place at either end rather than the curve evaluated
    /// there.** An end is shared with everything else that meets there, and two
    /// walks that each recomputed it could land a rounding apart — which is a
    /// hairline between a face and the wall swept off its own boundary.
    fn cut(&self, step: usize, steps: usize) -> Self::At {
        if step == 0 {
            return self.ends()[0];
        }
        if step == steps {
            return self.ends()[1];
        }
        self.at(step, steps)
    }

    /// The corners of a polyline following the curve from its start, stopping
    /// short of its end — so a loop's curves laid end to end name each corner
    /// once.
    ///
    /// **Appends**, because a caller tracing a whole loop traces it into one
    /// buffer.
    fn walk(&self, sagitta: f64, into: &mut Vec<Self::At>) {
        let steps = self.steps(sagitta);
        for step in 0..steps {
            into.push(self.cut(step, steps));
        }
    }
}
