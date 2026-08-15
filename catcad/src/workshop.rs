//! The room an edit works in, and everything the solve it runs leaves behind.

use silverpoint::{Arrangement, Outcome, PointId, Removed, Sketch, Solver};

/// The solver, and everything derived from a [`Drawing`] rather than written
/// down in one.
///
/// The runtime half of the model, held apart from it because the boundary is
/// the file format: a [`Drawing`] is a sketch and a plane, which is the whole
/// of what saving has to write, and every answer below follows from those two
/// by running the solver over them again. Loading rebuilds this rather than
/// reading it.
///
/// The solver is *in* rather than beside, which is the whole of what makes the
/// rest of it trustworthy. Everything here is what a solve leaves behind, so a
/// caller that could reach the solver without reaching this would be a caller
/// who could solve and not record — and every miss is silent: an outcome left
/// stale paints the drawing in the colours of where it used to be, and a
/// revision left alone leaves the picture on screen unrepainted. Kept together,
/// there is no solving without settling, because there is no solver to reach.
///
/// Owned by the application and lent to each edit. The buffers it keeps are
/// worth keeping for the length of a drag and worth nothing at all in a file;
/// and a document holding many drawings wants the room for one between them
/// rather than one apiece.
///
/// [`Drawing`]: crate::drawing::Drawing
#[derive(Debug, Default)]
pub(crate) struct Workshop {
    /// The room a solve works in, kept across calls rather than stood up for
    /// each: a drag solves sixty times a second and the buffers come out the
    /// same size every time.
    solver: Solver,
    /// What the last solve left behind: how the run went, and which geometry
    /// the constraints have decided — which is what the drawing is painted in
    /// the colour of.
    outcome: Outcome,
    /// Which version of the drawing this describes, so anything holding a
    /// layout of it can tell whether that layout is still current.
    revision: Revision,
    /// What the curves shut in.
    ///
    /// Derived like `outcome` beside it and for the same reason: the sketch
    /// says where its curves are, and what those enclose follows from that
    /// rather than being kept in step by hand. A face nobody could have drawn
    /// on purpose — half a circle, the ring between two others — exists exactly
    /// as much as one that traces edges the user placed.
    arrangement: Arrangement,
    /// What the last cleanup took out, or `None` where the last thing done to
    /// the drawing was not one.
    ///
    /// Cleared by every other edit — see [`Workshop::settled`] — because a
    /// count left standing would go on reporting a cleanup from two drags ago.
    ///
    /// `None` and `Some(Removed::default())` are different answers, and the one
    /// the reader most needs: a cleanup that found nothing has to say so, or
    /// pressing the command on a tidy drawing looks like it did not work.
    cleaned: Option<Removed>,
}

impl Workshop {
    /// Add to the sketch with `edit`, and settle everything around what that
    /// left.
    ///
    /// The first of the three shapes an edit takes, and what separates them is
    /// what the solver is asked for. This asks it everything, holding nothing.
    ///
    /// It ought to have nothing to do. What is added arrives already satisfying
    /// what it states — a point put on an edge is *placed* on the edge, by
    /// [`Anchor::on_sketch`](crate::drawing::anchor::Anchor) — and what was
    /// there was satisfied before, so the solve takes no step and nothing
    /// moves. That is what a solve is for here: it is the check, not the fix. A
    /// future addition that could not place itself exactly would be settled by
    /// it, and until then it costs one assembly to say that nothing needed
    /// settling.
    pub(crate) fn solved(&mut self, sketch: &mut Sketch, edit: impl FnOnce(&mut Sketch)) {
        self.settled(sketch, |solver, sketch, outcome| {
            edit(sketch);
            solver.solve(sketch, outcome);
        });
    }

    /// Change the sketch with `edit`, and take everything measuring what that
    /// leaves decides.
    ///
    /// The second shape, and the one that asks the constraints nothing — so it
    /// only measures, and measuring moves nothing. That is the whole of what it
    /// is for: an edit whose result is already the answer, where a solve would
    /// be free to wander off it.
    pub(crate) fn measured(&mut self, sketch: &mut Sketch, edit: impl FnOnce(&mut Sketch)) {
        self.settled(sketch, |solver, sketch, outcome| {
            edit(sketch);
            solver.measure(sketch, outcome);
        });
    }

    /// Move the geometry with `edit`, `held` pinned for the length of it, and
    /// take everything the solve that follows decides.
    ///
    /// The third shape: what a drag is. It holds what the pointer has and asks
    /// the constraints to accommodate it, where the two above hold nothing.
    ///
    /// [`Solver::edit_holding`] puts the geometry back where it found it if the
    /// constraints refuse, so a drag they will not take moves nothing — and is
    /// still settled, because what the run decided is what the drawing is
    /// painted in the colour of either way.
    pub(crate) fn dragged(
        &mut self,
        sketch: &mut Sketch,
        held: &[PointId],
        edit: impl Fn(&mut Sketch),
    ) {
        self.settled(sketch, |solver, sketch, outcome| {
            solver.edit_holding(sketch, held, outcome, edit);
        });
    }

    /// Run `solve` over `sketch`, and record everything the drawing then says
    /// about itself.
    ///
    /// The one place any of this is written, and the reason it takes the solve
    /// rather than its answer: the two describe one moment, and a caller that
    /// solved for itself and then reported would be a caller who could do
    /// either half and skip the other.
    ///
    /// Private, and the three above are the whole of what it is reachable
    /// through — one per entry point the solver has, each of which fills the
    /// whole outcome from the same measurement it reports.
    fn settled(
        &mut self,
        sketch: &mut Sketch,
        solve: impl FnOnce(&mut Solver, &mut Sketch, &mut Outcome),
    ) {
        let Self {
            solver,
            outcome,
            revision,
            arrangement,
            cleaned,
        } = self;
        solve(solver, sketch, outcome);
        *revision = revision.next();
        // After the solve, because what the curves enclose depends on where the
        // solve left them — and unconditionally, because there is no cheaper
        // question than this one to ask first. Rebuilt in place rather than
        // replaced, which is what keeps a drag off the heap: the lists it works
        // in come out the same size every frame.
        arrangement.rebuild(sketch);
        // Whatever this edit was, it is now the last thing done — so a cleanup
        // before it has stopped describing the drawing. The one that *is* a
        // cleanup writes its own answer back after this returns.
        *cleaned = None;
    }

    /// Record what a cleanup took out.
    ///
    /// After the settle rather than inside it, because settling is what wipes
    /// the last answer — writing this first would be writing it to be cleared.
    pub(crate) fn cleaned_up(&mut self, removed: Removed) {
        self.cleaned = Some(removed);
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

    /// Which version of the drawing this describes.
    ///
    /// What a caller holding a layout compares against its own, to tell whether
    /// what it drew still describes what is here.
    pub(crate) fn revision(&self) -> Revision {
        self.revision
    }

    /// What the drawing's curves shut in.
    pub(crate) fn arrangement(&self) -> &Arrangement {
        &self.arrangement
    }

    /// What the last cleanup took out, or `None` where the last edit was not
    /// one.
    pub(crate) fn cleaned(&self) -> Option<Removed> {
        self.cleaned
    }
}

/// Which version of a drawing something describes.
///
/// Bumped whenever the drawing settles, which is whenever it has been solved
/// again. Compared and never read: the number means nothing beyond not being
/// the one before it.
///
/// Conservative on purpose — it can move where the geometry did not. A drag the
/// constraints refuse is solved and put back, and this counts that, because
/// what can cheaply be said is that the drawing has been worked on and not
/// whether the work came to anything. The asymmetry is the point: a revision
/// that missed a change would leave a stale picture on screen, where a spare
/// one costs a refill of buffers that already have the room.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Revision(u64);

impl Revision {
    /// The one after this.
    fn next(self) -> Self {
        Self(self.0 + 1)
    }
}
