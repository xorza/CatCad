//! Every curve a boundary runs along, and which run lies on which.

use crate::solid::buckets::Buckets;
use crate::solid::geometry::curve::Curve;

/// One curve, and the run every crossing along it shares.
#[derive(Debug)]
struct Along {
    curve: Curve,
    /// `None` until a crossing asks for one — a curve reaches this list by
    /// being a face's own edge too, and those take a run apiece.
    crossing: Option<u32>,
}

/// The curves the runs of a boundary lie on, and the run each stretch was
/// marked with.
///
/// **Two tables and not one, because a mark has to answer two questions and
/// they want opposite things of it.** [`Came::Arc`](super::splitting::Came) is
/// one number per stretch, and it is read by two sorts of caller:
///
/// - *Is this the same stretch?* — [`passing`](super::splitting::passing), which
///   drops a corner the boundary merely carries on through. Two arcs of one
///   circle are two edges of a face with a vertex between them, no face being
///   allowed to wrap, and marked alike that vertex is dropped and a disc comes
///   back as one edge from nowhere to nowhere.
/// - *Is this the same curve?* — [`Sewing::pin`](super::sewing::Sewing::pin) and
///   what reads it, which have to tell that a place another face put on a curve
///   is a place on *this* one. The same crossing imprinted on two faces has to
///   come back as one arc, and marked apart it cannot.
///
/// So the mark is a **run**, and the run says which curve it lies on. Two runs
/// on one curve are two stretches that know they are the same circle, which is
/// what both callers wanted and neither could have had alone.
#[derive(Debug, Default)]
pub(super) struct Imprints {
    /// Every distinct curve, in the order they were first met.
    along: Vec<Along>,
    /// Which of those each run lies on.
    runs: Vec<u32>,
    /// Which of the curves above key alike, so a curve met again is told from
    /// a handful rather than from every curve met so far — see
    /// [`Imprints::met`].
    found: Buckets,
}

impl Imprints {
    pub(super) fn clear(&mut self) {
        self.along.clear();
        self.runs.clear();
        self.found.clear();
    }

    /// Make room for `curves` curves and the runs along them.
    ///
    /// **A lower bound and not the count**, which is why it is
    /// [`Vec::reserve`] rather than the exact form: what a caller can say
    /// before the work starts is how many curved edges the two bodies already
    /// have, and every crossing the surfaces are found to make is one more
    /// curve on top of that. Two faces walk each of those edges, so each
    /// carries two runs — see [`Imprints::edge`].
    ///
    /// Without it the two lists grow a doubling at a time, and every one of
    /// those doublings falls in the frame a model got bigger. That is the
    /// frame that can least afford them.
    pub(super) fn reserve(&mut self, curves: usize) {
        self.along.reserve(curves);
        self.runs.reserve(2 * curves);
    }

    /// The run a crossing of two surfaces takes — one per curve, shared by
    /// every face the crossing is imprinted on.
    ///
    /// Shared because [`Meeting::of`](crate::solid::meeting::Meeting::of) is one
    /// routine whichever way round it is asked, so the circle a plane cuts out
    /// of a cylinder is the identical value both times — and because a cylinder
    /// is two faces of one surface, so one body meets it twice over. Given a run
    /// apiece, an arc of a block's rim and an arc of the bore's wall would be
    /// two stretches that happen to coincide, and the sewing could not tell that
    /// they are one edge.
    pub(super) fn crossing(&mut self, curve: Curve) -> u32 {
        let on = self.met(curve);
        match self.along[on as usize].crossing {
            Some(run) => run,
            None => {
                let run = self.ran(on);
                self.along[on as usize].crossing = Some(run);
                run
            }
        }
    }

    /// A run of its own along `curve`, for one edge of a face's own boundary.
    ///
    /// Its own, so that the vertex between two arcs of one circle survives —
    /// and still knowing the curve, so that a place another face put on it is
    /// still found.
    pub(super) fn edge(&mut self, curve: Curve) -> u32 {
        let on = self.met(curve);
        self.ran(on)
    }

    /// The curve the run at `run` lies on.
    pub(super) fn curve(&self, run: u32) -> Curve {
        self.along[self.runs[run as usize] as usize].curve
    }

    /// *Which* curve it lies on, as the number two runs share exactly when they
    /// lie on one.
    ///
    /// An index rather than the curve itself, because that is a comparison of
    /// one integer where the other is seven floats — and exact by construction,
    /// where two curves worked out separately fall a rounding apart.
    pub(super) fn on(&self, run: u32) -> u32 {
        self.runs[run as usize]
    }

    /// Where `curve` stands in the list, put there if it is new.
    ///
    /// **Through the index and not by walking**, which matters because every
    /// stretch of every boundary of both bodies asks: a walk held a whole curve
    /// against every curve met so far, and the cost of that grows as the square
    /// of the body. What the index hands back is the curves keying
    /// alike, and equality decides among them as it always did — see
    /// [`Curve::key`].
    ///
    /// At most one of them can match, this being the call that keeps the list
    /// distinct, so the first confirmed is the answer.
    fn met(&mut self, curve: Curve) -> u32 {
        let key = curve.key();
        let found = self
            .found
            .under(key)
            .find(|&at| self.along[at as usize].curve == curve);
        if let Some(found) = found {
            return found;
        }
        let at = self.found.file(key);
        debug_assert_eq!(at as usize, self.along.len(), "the index lost step");
        self.along.push(Along {
            curve,
            crossing: None,
        });
        at
    }

    /// One more run along the curve at `on`.
    fn ran(&mut self, on: u32) -> u32 {
        self.runs.push(on);
        self.runs.len() as u32 - 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::solid::geometry::axis::Axis;
    use crate::solid::geometry::circle::Circle;
    use glam::DVec3;

    fn ring(radius: f64) -> Curve {
        Curve::Circle(Circle {
            axis: Axis::new(DVec3::ZERO, DVec3::Y, DVec3::X),
            radius,
        })
    }

    /// **Two stretches on one curve are two runs that know they are one
    /// curve**, which is the whole of what this is for and neither half of it
    /// is worth anything alone.
    ///
    /// Every crossing along a curve shares a run, because the same circle met
    /// from either side has to come back as one edge. Every edge of a face's own
    /// boundary takes a run apiece, because two arcs of one circle have a vertex
    /// between them. And all of them answer with the same curve, which is what
    /// tells a place another face put on it from a place on some other circle
    /// altogether.
    #[test]
    fn crossings_share_a_run_where_a_faces_own_edges_do_not() {
        let mut imprints = Imprints::default();
        let (one, two) = (ring(1.0), ring(2.0));

        // A crossing met twice is one run — a cylinder is two faces of one
        // surface, so a body meets it once for each.
        let crossed = imprints.crossing(one);
        assert_eq!(imprints.crossing(one), crossed);

        // Two edges of a face along that same circle are two runs.
        let (here, there) = (imprints.edge(one), imprints.edge(one));
        assert_ne!(here, there, "a disc came back as one edge");
        assert_ne!(here, crossed);
        assert_ne!(there, crossed);

        // And all three lie on the one curve, which is the half that lets a
        // place put on it by any of them be found by the others.
        assert_eq!(imprints.on(here), imprints.on(crossed));
        assert_eq!(imprints.on(there), imprints.on(crossed));
        for run in [crossed, here, there] {
            assert_eq!(imprints.curve(run), one);
        }

        // A different circle is a different curve, or the two halves above
        // would both hold of a table that answered the same to everything.
        let other = imprints.crossing(two);
        assert_ne!(imprints.on(other), imprints.on(crossed));
        assert_eq!(imprints.curve(other), two);

        // Emptied, and the numbering starts over.
        imprints.clear();
        assert_eq!(imprints.crossing(two), 0);
        assert_eq!(imprints.on(0), 0);
    }
}
