//! What the last solve made of one sketch.

use silverpoint::{Arrangement, Outcome, Sketch, Solver};

use crate::timeline::FeatureId;

/// Everything that follows from one sketch by running the solver over it.
///
/// Kept per sketch rather than per document, which is the whole of what makes
/// several of them possible: two sketches are two systems, solved apart and
/// enclosing what they each enclose, and a report that described "the drawing"
/// could only ever describe one of them.
///
/// Reused across rebuilds rather than stood up afresh. The arrangement holds
/// every buffer a rebuild works in, so a sketch that comes back the same shape
/// it was costs nothing — see [`Arrangement`].
#[derive(Debug)]
pub(crate) struct Settled {
    /// The step of the timeline this describes.
    ///
    /// Carried rather than implied by position, so nothing has to keep this
    /// list and the timeline's own in step: a sketch is found by the handle
    /// that names it, and a list that fell out of order would be found out
    /// rather than quietly answering with its neighbour.
    of: FeatureId,
    /// How the last run went, and which geometry the constraints have decided
    /// — which is what the sketch is painted in the colour of.
    outcome: Outcome,
    /// What its curves shut in.
    ///
    /// Derived like `outcome` beside it and for the same reason: the sketch
    /// says where its curves are, and what those enclose follows from that
    /// rather than being kept in step by hand. A face nobody could have drawn
    /// on purpose — half a circle, the ring between two others — exists exactly
    /// as much as one that traces edges the user placed.
    arrangement: Arrangement,
}

impl Settled {
    /// A place for the sketch at `of`, holding nothing until it is solved.
    pub(super) fn new(of: FeatureId) -> Self {
        Self {
            of,
            outcome: Outcome::default(),
            arrangement: Arrangement::default(),
        }
    }

    /// Which sketch this describes.
    pub(super) fn of(&self) -> FeatureId {
        self.of
    }

    /// Run `solve` over `sketch`, and record everything it then says about
    /// itself.
    ///
    /// The solver comes from outside rather than being reached through here,
    /// because one solver serves every sketch — see [`Build`](super::Build),
    /// which owns it and is the only caller.
    pub(super) fn settle(
        &mut self,
        solver: &mut Solver,
        sketch: &mut Sketch,
        solve: impl FnOnce(&mut Solver, &mut Sketch, &mut Outcome),
    ) {
        solve(solver, sketch, &mut self.outcome);
        // After the solve, because what the curves enclose depends on where the
        // solve left them — and unconditionally, because there is no cheaper
        // question than this one to ask first. Rebuilt in place rather than
        // replaced, which is what keeps a drag off the heap: the lists it works
        // in come out the same size every frame.
        self.arrangement.rebuild(sketch);
    }

    /// How the last run went, and what the constraints have and have not
    /// decided.
    ///
    /// Only ever read beside the sketch it was measured over — they are two
    /// readings of one moment, and nothing keeps them together but the order
    /// they are written in.
    pub(crate) fn outcome(&self) -> &Outcome {
        &self.outcome
    }

    /// What the sketch's curves shut in.
    pub(crate) fn arrangement(&self) -> &Arrangement {
        &self.arrangement
    }
}
