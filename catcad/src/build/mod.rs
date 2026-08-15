//! The room an edit works in, and everything replaying the timeline leaves
//! behind.

use silverpoint::{Drive, PointId, Removed, Sketch, Solver};

use crate::build::modelled::Modelled;
use crate::build::settled::Settled;
use crate::profile::Profile;
use crate::timeline::FeatureId;

pub(crate) mod modelled;
pub(crate) mod settled;

/// The solver, and everything derived from a [`Timeline`] rather than written
/// down in one.
///
/// The runtime half of the model, held apart from it because the boundary is
/// the file format: a timeline is what saving has to write, and every answer
/// below follows from it by running the solver over its sketches again.
/// Loading rebuilds this rather than reading it.
///
/// The solver is *in* rather than beside, which is the whole of what makes the
/// rest of it trustworthy. Everything here is what a solve leaves behind, so a
/// caller that could reach the solver without reaching this would be a caller
/// who could solve and not record — and every miss is silent: an outcome left
/// stale paints a sketch in the colours of where it used to be, and a revision
/// left alone leaves the picture on screen unrepainted. Kept together, there is
/// no solving without settling, because there is no solver to reach.
///
/// One solver between every sketch, and one [`Settled`] apiece. That asymmetry
/// is what the split is for: the room a solve works in is worth keeping and
/// worth sharing, and what a solve *decided* is about one sketch and cannot be
/// shared at all.
///
/// Owned by the application and lent to each edit. The buffers it keeps are
/// worth keeping for the length of a drag and worth nothing at all in a file.
///
/// [`Timeline`]: crate::timeline::Timeline
#[derive(Debug, Default)]
pub(crate) struct Build {
    /// The room a solve works in, kept across calls rather than stood up for
    /// each: a drag solves sixty times a second and the buffers come out the
    /// same size every time.
    solver: Solver,
    /// What the last solve made of each sketch, in the order they were first
    /// settled.
    ///
    /// A list searched by handle rather than a map: a document holds a few
    /// sketches, a walk of a few entries beats hashing one, and the order they
    /// arrived in is a fair order to hold them in.
    settled: Vec<Settled>,
    /// Which region each extrude is currently grown from, in the order the
    /// timeline holds them.
    ///
    /// Emptied and refilled whole by [`Build::remodel`] rather than kept in step
    /// entry by entry: an extrude names its region by what bounds it, and every
    /// edit to a sketch is an edit that could have taken one of those away.
    modelled: Vec<Modelled>,
    /// Which version of the document this describes, so anything holding a
    /// layout of it can tell whether that layout is still current.
    revision: Revision,
    /// What the last cleanup took out, or `None` where the last thing done to
    /// the document was not one.
    ///
    /// Cleared by every other edit — see [`Build::settle`] — because a
    /// count left standing would go on reporting a cleanup from two drags ago.
    ///
    /// `None` and `Some(Removed::default())` are different answers, and the one
    /// the reader most needs: a cleanup that found nothing has to say so, or
    /// pressing the command on a tidy drawing looks like it did not work.
    cleaned: Option<Removed>,
}

impl Build {
    /// Settle the sketch at `of` by asking the constraints everything, holding
    /// nothing.
    ///
    /// The first of the three shapes an edit takes, and what separates them is
    /// what the solver is asked for.
    ///
    /// Whatever changed the sketch has already changed it. A caller holds the
    /// sketch — that is what it hands over here — so an edit taken as a closure
    /// would be a call deferred by one line and nothing else, and it cost the
    /// callers that wanted an *answer* out of their edit a place to put it.
    ///
    /// It ought to have nothing to do. What is added arrives already satisfying
    /// what it states — a point put on an edge is *placed* on the edge, by
    /// [`Anchor::on_sketch`](crate::drawing::anchor::Anchor) — and what was
    /// there was satisfied before, so the solve takes no step and nothing
    /// moves. That is what a solve is for here: it is the check, not the fix. A
    /// future addition that could not place itself exactly would be settled by
    /// it, and until then it costs one assembly to say that nothing needed
    /// settling.
    pub(crate) fn solved(&mut self, of: FeatureId, sketch: &mut Sketch) {
        self.settle(of, sketch, Settling::Solved);
    }

    /// Settle the sketch at `of` by measuring it and asking the constraints
    /// nothing.
    ///
    /// The second shape, and the one that asks the constraints nothing — so it
    /// only measures, and measuring moves nothing. That is the whole of what it
    /// is for: an edit whose result is already the answer, where a solve would
    /// be free to wander off it. Like [`Build::solved`], whatever changed the
    /// sketch has already changed it.
    pub(crate) fn measured(&mut self, of: FeatureId, sketch: &mut Sketch) {
        self.settle(of, sketch, Settling::Measured);
    }

    /// Drive `driving` where the pointer is asking for it, `holding` pinned for
    /// the length of it, and take everything the solve that follows decides.
    ///
    /// The third shape: what a drag is. It reaches for something and asks the
    /// constraints to accommodate it, where the two above ask them nothing.
    ///
    /// [`Solver::drag`] pulls toward the ask through the constraints rather than
    /// writing it and settling afterwards, so a drag they will not take moves
    /// nothing — and is still settled, because what the run decided is what the
    /// sketch is painted in the colour of either way.
    pub(crate) fn dragged(
        &mut self,
        of: FeatureId,
        sketch: &mut Sketch,
        driving: &[Drive],
        holding: &[PointId],
    ) {
        self.settle(of, sketch, Settling::Dragged { driving, holding });
    }

    /// Run `solve` over the sketch at `of`, and record everything the document
    /// then says about itself.
    ///
    /// The one place any of this is written, and the reason it takes the solve
    /// rather than its answer: the two describe one moment, and a caller that
    /// solved for itself and then reported would be a caller who could do
    /// either half and skip the other.
    ///
    /// Private, and the three above are the whole of what it is reachable
    /// through — one per entry point the solver has, each of which fills the
    /// whole outcome from the same measurement it reports.
    fn settle(&mut self, of: FeatureId, sketch: &mut Sketch, settling: Settling<'_>) {
        let Self {
            solver,
            settled,
            revision,
            cleaned,
            ..
        } = self;
        // Made on first use rather than by walking the timeline, because a
        // sketch nothing has settled has nothing to say: what a caller would
        // read off an empty one is an unsolved report and an arrangement of
        // nothing, and both are answers it should not be given.
        let at = match settled.iter().position(|had| had.of() == of) {
            Some(at) => at,
            None => {
                settled.push(Settled::new(of));
                settled.len() - 1
            }
        };
        settled[at].settle(solver, sketch, settling);
        *revision = revision.next();
        // Whatever this edit was, it is now the last thing done — so a cleanup
        // before it has stopped describing the document. The one that *is* a
        // cleanup writes its own answer back after this returns.
        *cleaned = None;
    }

    /// Forget everything settled for the document that was open, for one that
    /// is about to be.
    ///
    /// Everything here is keyed by [`FeatureId`], and a document opened from a
    /// file numbers its steps from zero like any other — so what the last one
    /// settled is not stale so much as *wrong*, a report about a sketch that no
    /// longer exists filed under the name of one that does.
    ///
    /// The revision counts on rather than starting over, and that is the whole
    /// reason this is a call rather than a fresh [`Build`]. A view compares the
    /// revision it last drew against this one to decide whether to draw again
    /// — see [`Made`](crate::paint::layout::Made) — so a document opened into a
    /// reset counter could land on a number the view believes it has already
    /// drawn, and leave the old picture on screen with no way to notice.
    pub(crate) fn reopened(&mut self) {
        self.settled.clear();
        self.modelled.clear();
        self.cleaned = None;
        self.revision = self.revision.next();
    }

    /// Work out afresh which region each extrude is grown from.
    ///
    /// After a settle rather than inside one, because the two are about
    /// different things: a solve is about one sketch, and this is about every
    /// step standing downstream of whichever moved. Which is also why it replays
    /// the whole list rather than the part the last edit could have reached —
    /// what an edit reaches is a graph the timeline does not keep, and resolving
    /// one extrude is a walk of a few faces comparing a few handles. Narrowing
    /// it is worth doing when a document is large enough to notice, and there is
    /// nothing to narrow *by* until then.
    ///
    /// Takes the walk rather than the timeline, because the timeline is
    /// [`Document`](crate::document::Document)'s: what crosses between the two
    /// is the pair each step names and nothing else, which is the same line
    /// [`Models`](crate::model::Models) sits on.
    pub(crate) fn remodel<'a>(&mut self, extrudes: impl Iterator<Item = (FeatureId, &'a Profile)>) {
        let Self {
            settled, modelled, ..
        } = self;
        // Emptied and refilled rather than written over in place: a step may
        // have been added or taken away since the last time, and there is no
        // position here worth keeping — a caller names an extrude by its handle.
        modelled.clear();
        for (of, profile) in extrudes {
            let settled = filed_under(settled, profile.sketch(), Settled::of, UNSETTLED);
            modelled.push(Modelled::new(of, profile.face_in(settled.arrangement())));
        }
    }

    /// Note that the document has moved without anything being solved.
    ///
    /// What moving a plane leaves behind. A sketch's coordinates are its
    /// plane's own, so a plane that moves changes where every sketch hanging
    /// off it *lands* and nothing about what any of them says — no solve, no
    /// arrangement, and nothing here to rewrite but the number that tells a
    /// picture it is out of date.
    pub(crate) fn revised(&mut self) {
        self.revision = self.revision.next();
        // Whatever this edit was, it is now the last thing done — for the same
        // reason [`Build::settle`] says so.
        self.cleaned = None;
    }

    /// Record what a cleanup took out.
    ///
    /// After the settle rather than inside it, because settling is what wipes
    /// the last answer — writing this first would be writing it to be cleared.
    pub(crate) fn cleaned_up(&mut self, removed: Removed) {
        self.cleaned = Some(removed);
    }

    /// What the last solve made of the sketch at `of`.
    ///
    /// Every sketch the document holds has been settled by the time anything
    /// asks — opening one solves each of them — so a sketch with no answer here
    /// is a sketch that was never opened, which is a mistake in whatever raised
    /// the document rather than a state a reader has to handle.
    pub(crate) fn settled(&self, of: FeatureId) -> &Settled {
        filed_under(&self.settled, of, Settled::of, UNSETTLED)
    }

    /// Which region the extrude at `of` is grown from, or `None` where its
    /// profile no longer names one.
    ///
    /// Every extrude the document holds has been through [`Build::remodel`] by
    /// the time anything asks — raising a document remodels it, and so does
    /// every edit — so an extrude with no answer here is one nothing has
    /// replayed, which is a mistake in whatever raised the document rather than
    /// a state a reader has to handle. The `None` inside is the other question,
    /// and is a fair answer: see [`Modelled`].
    pub(crate) fn modelled(&self, of: FeatureId) -> Option<usize> {
        filed_under(&self.modelled, of, Modelled::of, UNMODELLED).at()
    }

    /// Which version of the document this describes.
    ///
    /// What a caller holding a layout compares against its own, to tell whether
    /// what it drew still describes what is here.
    ///
    /// One for the document rather than one per sketch. What reads it is a
    /// picture of the *whole* of it, redrawn whole — so a second number would
    /// buy nothing a caller could spend.
    pub(crate) fn revision(&self) -> Revision {
        self.revision
    }

    /// What the last cleanup took out, or `None` where the last edit was not
    /// one.
    pub(crate) fn cleaned(&self) -> Option<Removed> {
        self.cleaned
    }
}

/// The entry of `filed` that answers for `of`, or the mistake `missing` names.
///
/// Both lists the build keeps are held the same way — a short run searched by
/// the handle each entry carries, walked rather than hashed because a document
/// holds a few of either. One function so that is said once.
///
/// A free function over the slice rather than a method on [`Build`], because one
/// caller has the build taken apart to write one of its fields while reading
/// another — see [`Build::remodel`] — and cannot borrow the whole of one.
///
/// `key` is a closure rather than a trait over the two entry types: one method
/// apiece is not a trait's worth of agreement between them.
fn filed_under<'a, T>(
    filed: &'a [T],
    of: FeatureId,
    key: impl Fn(&T) -> FeatureId,
    missing: &str,
) -> &'a T {
    filed.iter().find(|had| key(had) == of).expect(missing)
}

// What a caller reaching for an answer the build has not worked out is told.
// Reaching one means the document was raised without being settled or replayed,
// which is a mistake in whatever raised it rather than anything a reader can
// handle.
const UNSETTLED: &str = "this sketch has not been settled";
const UNMODELLED: &str = "this extrude has not been modelled";

/// Which of the solver's entry points an edit is settled through.
///
/// Named rather than handed over as a closure that runs one. A closure would be
/// free to run none of them, or two, or something else entirely with the three
/// `&mut`s it was given — where this is the whole list, in one place, and the
/// only thing a settle can be.
///
/// The edit that *precedes* a solve is still a closure, and rightly: adding a
/// point, adding a constraint and rewriting a dimension are not a closed set and
/// never will be. What a solve is, is.
#[derive(Debug, Clone, Copy)]
enum Settling<'a> {
    /// Ask the constraints everything, holding nothing.
    Solved,
    /// Ask them nothing, and only read what the sketch already says.
    Measured,
    /// Drive geometry where the pointer is asking for it. See [`Solver::drag`].
    Dragged {
        driving: &'a [Drive],
        holding: &'a [PointId],
    },
}

/// Which version of a document something describes.
///
/// Bumped whenever anything settles, which is whenever something has been
/// solved again. Compared and never read: the number means nothing beyond not
/// being the one before it.
///
/// Conservative on purpose — it can move where the geometry did not. A drag the
/// constraints refuse is solved and put back, and this counts that, because
/// what can cheaply be said is that the document has been worked on and not
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

#[cfg(test)]
mod tests;
