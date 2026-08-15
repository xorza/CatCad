//! What the last rebuild made of one extrude.

use crate::timeline::FeatureId;

/// Which region of the drawing an extrude is currently grown from.
///
/// The whole of what an extrude leaves behind, and it is one number — because
/// the [`Arrangement`](silverpoint::Arrangement) next door is already a boundary
/// representation of the plane the profile lies in, and a prism over one of its
/// faces is settled by which face and how far. Everything a solid *is* follows
/// from those two, so nothing here holds a face, an edge or a triangle.
///
/// Beside [`Settled`](super::settled::Settled) rather than inside it, because an
/// extrude is a step of the timeline in its own right: several may be grown from
/// one sketch, and what a solve decided is about the sketch rather than about
/// anything built on it.
#[derive(Debug)]
pub(crate) struct Modelled {
    /// The step of the timeline this describes.
    ///
    /// Carried rather than implied by position, for the reason
    /// [`Settled`](super::settled::Settled) carries one: a list that fell out of
    /// step with the timeline would be found out rather than quietly answering
    /// with its neighbour.
    of: FeatureId,
    /// Where the profile's region falls among the faces of the sketch it names,
    /// or `None` where the drawing no longer holds one bounded by exactly what
    /// the profile lists.
    ///
    /// `None` is a state and not a failure to be reported: someone who draws a
    /// line across a region has taken away what an extrude was built on, and a
    /// document says so the way an unsolved sketch says so — see
    /// [`Outcome::converged`](silverpoint::Outcome::converged).
    at: Option<usize>,
}

impl Modelled {
    /// The extrude at `of`, grown from the region at `at`.
    pub(super) fn new(of: FeatureId, at: Option<usize>) -> Self {
        Self { of, at }
    }

    /// Which extrude this describes.
    pub(super) fn of(&self) -> FeatureId {
        self.of
    }

    /// Where its region falls, or `None` where its profile names none.
    pub(super) fn at(&self) -> Option<usize> {
        self.at
    }
}
