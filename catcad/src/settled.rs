//! What the last solve made of the drawing, and the room it was worked out in.

use silverpoint::{Arrangement, Outcome, Removed, Sketch};

use crate::drawing::Drawing;
use crate::part::Part;

/// Everything derived from a [`Drawing`] rather than written down in one.
///
/// The runtime half of the model, held apart from it because the boundary is
/// the file format: a [`Drawing`] is a sketch and a plane, which is the whole
/// of what saving has to write, and every answer below follows from those two
/// by running the solver over them again. Loading rebuilds this rather than
/// reading it.
///
/// Owned by the application and lent to each edit, exactly as the
/// [`Solver`](silverpoint::Solver) is and for the same two reasons. The buffers
/// it keeps are worth keeping for the length of a drag and worth nothing at all
/// in a file; and a document holding many drawings wants the room for one
/// between them rather than one apiece.
#[derive(Debug, Default)]
pub(crate) struct Settled {
    /// What the last settle left behind: how the run went, and which geometry
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
    /// Cleared by every other edit — see [`Settled::settle`] — because a count
    /// left standing would go on reporting a cleanup from two drags ago.
    ///
    /// `None` and `Some(Removed::default())` are different answers, and the one
    /// the reader most needs: a cleanup that found nothing has to say so, or
    /// pressing the command on a tidy drawing looks like it did not work.
    cleaned: Option<Removed>,
}

impl Settled {
    /// Solve, and record everything the drawing then says about itself.
    ///
    /// The one place any of this is written, and the reason it takes the solve
    /// rather than its answer: the two describe one moment, and a caller that
    /// solved for itself and then reported would be a caller who could do
    /// either half and skip the other. Both misses are silent — an outcome left
    /// stale paints the drawing in the colours of where it used to be, and a
    /// revision left alone leaves the picture on screen unrepainted.
    ///
    /// `solve` is handed the sketch and the buffer to fill; every entry point
    /// the solver has fits, because each of them fills the whole outcome from
    /// the same measurement it reports.
    pub(crate) fn settle(
        &mut self,
        sketch: &mut Sketch,
        solve: impl FnOnce(&mut Sketch, &mut Outcome),
    ) {
        solve(sketch, &mut self.outcome);
        self.revision = self.revision.next();
        // After the solve, because what the curves enclose depends on where the
        // solve left them — and unconditionally, because there is no cheaper
        // question than this one to ask first. Rebuilt in place rather than
        // replaced, which is what keeps a drag off the heap: the lists it works
        // in come out the same size every frame.
        self.arrangement.rebuild(sketch);
        // Whatever this edit was, it is now the last thing done — so a cleanup
        // before it has stopped describing the drawing. The one that *is* a
        // cleanup writes its own answer back after this returns.
        self.cleaned = None;
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

    /// Whether `part` is still there to be picked out.
    ///
    /// The two halves of what a part can be, answered by the two halves of the
    /// model: an entity by the drawing that holds it, and a face by there still
    /// being that many. Here rather than on [`Drawing`] because only one of
    /// those is a question a drawing can answer.
    pub(crate) fn holds_part(&self, drawing: &Drawing, part: Part) -> bool {
        match part {
            Part::Entity(entity) => drawing.holds(entity),
            Part::Face(at) => at < self.arrangement.faces().len(),
        }
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
