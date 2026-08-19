//! What has been done to the document, and how to take it back.

use crate::build::Build;
use crate::document::{Document, Shaped};
use crate::intent::change::{About, Change};
use crate::intent::{Intent, Intents, Step};
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Uprooted};

/// How many steps back the history goes.
///
/// Each step holds the drawing's geometry twice over — a few hundred bytes for
/// a sketch this size, a few kilobytes for a large one — so a hundred of them
/// is a bound on memory that no session will notice reaching, and a depth
/// nobody will reach the end of either.
const DEPTH: usize = 100;

/// Every step taken since the document was opened, and where in them the
/// document currently stands.
///
/// Not part of the document, deliberately. What is in a document is what saving
/// writes down; how it came to say that belongs to this run of the program and
/// is thrown away with it.
///
/// **State, not actions, wherever there is a state to keep.** A rewritten step
/// records where the drawing was and where it went, and undoing puts a value
/// back rather than doing the opposite of what was done. There is no choice
/// about that: an edit is followed by a solve, and a solve moves whatever the
/// constraints couple to what was edited. Dragging a point back to where it
/// started runs a *different* solve from a *different* starting guess, and one
/// that can settle on a different branch — the demo's own hub admits a mirrored
/// solution. And a drag the constraints refuse did nothing at all, which has no
/// opposite to do.
///
/// A *creation* is the one place the rule cannot hold, and it is why [`Edit`] is
/// an enum. There is no earlier state of a step that was not there, so undoing
/// one really is doing the opposite: the step comes off again. Nothing is lost
/// by it, because taking a step away has none of the ambiguity putting geometry
/// somewhere has — it is gone or it is not.
#[derive(Debug, Default)]
pub(crate) struct History {
    edits: Vec<Edit>,
    /// How many of `edits` have been done. Undo steps it down, redo steps it
    /// back up, and `edits[applied..]` is what redo has left to put back.
    applied: usize,
    /// Whether the newest step is still being extended by a gesture in
    /// progress.
    open: bool,
}

impl History {
    /// Land everything a frame asked for, recording whatever moved the drawing.
    ///
    /// Says nothing about *whether the drawing moved*. That is a fact about the
    /// drawing, and anything that needs it reads
    /// [`Build::revision`](crate::build::Build::revision) — which cannot
    /// be forgotten to pass on, or passed on wrongly.
    ///
    /// What it does answer is the step the frame **made**, where one was made:
    /// a durable name that did not exist when the asking happened, minted by the
    /// document on the way through. The one thing about a frame that cannot come
    /// back through the inbox, and the reason is that the inbox is read before
    /// any of this runs — see [`Session::entered`](crate::session::Session),
    /// which is its only caller.
    ///
    /// The newest, where a frame somehow made two. Nothing raises two: each of
    /// the two changes that add a step is one press or one click, and a press is
    /// one frame's asking. If two ever arrive, the later is the one a session
    /// would have been taken into by the one that landed last.
    pub(crate) fn apply(
        &mut self,
        document: &mut Document,
        build: &mut Build,
        intents: &Intents,
    ) -> Option<FeatureId> {
        let mut made = None;
        for intent in intents.iter() {
            match intent {
                Intent::Step(Step::Undo) => self.undo(document, build),
                Intent::Step(Step::Redo) => self.redo(document, build),
                Intent::Step(Step::Release) => self.close(),
                Intent::Change(change) => made = self.edit(document, build, change).or(made),
                // Taken before this ran, by the app that owns them. What is in
                // hand and what is picked out are not steps to take back — see
                // `CatCad::apply`.
                //
                // Nor is opening a file, which is not a step *in* a history but
                // a different history: what has been done to the document that
                // was open cannot be taken back off the one that replaced it.
                Intent::Choice(_) | Intent::Errand(_) => {}
            }
        }
        made
    }

    /// Whether there is anything to take back.
    fn can_undo(&self) -> bool {
        self.applied > 0
    }

    /// Whether there is anything to put back.
    fn can_redo(&self) -> bool {
        self.applied < self.edits.len()
    }

    /// Do what `change` asks, and record an [`Edit`] if it moved the drawing.
    ///
    /// A change about no step is the camera's, and the camera is not the drawing
    /// — so it lands and is not recorded. Which of the three a change is is
    /// [`Change::about`]'s to say, and it says it with an exhaustive match: a
    /// variant added later cannot quietly join any of them without the compiler
    /// asking which.
    ///
    /// Everything else is recorded only if it moved the step it named, and
    /// the comparison is the whole of what decides that. A drag the constraints
    /// refuse records nothing, because
    /// [`Solver::drag`](silverpoint::Solver) has already put the
    /// geometry back by the time this looks.
    /// Hands back the step it made, where the change was one that makes one —
    /// see [`History::apply`], which is what carries it on.
    fn edit(
        &mut self,
        document: &mut Document,
        build: &mut Build,
        change: Change,
    ) -> Option<FeatureId> {
        // Matched rather than tested arm by arm, so the exhaustiveness
        // [`Change::about`] buys reaches here: a fourth thing a change could do
        // — a step *removed*, which the timeline does not offer yet — would
        // otherwise fall through to whichever branch was written last, and the
        // one it would fall through to is the one that records nothing.
        match change.about() {
            // A creation, and it is answered on its own because a step that was
            // not there has no *before* to compare against — the arm below is
            // all about what a step held on either side of an edit, and a
            // creation has only one side.
            About::Makes => {
                self.close();
                let Shaped::Made(at) = document.apply(build, change) else {
                    panic!("a change that makes a step says which one it made");
                };
                let feature = document.feature(at).clone();
                self.record(Edit::Added { at, feature });
                Some(at)
            }
            // A removal, and it is answered on its own for the reason the
            // creation above is: there is no *after* to compare against, only
            // the steps' absence. What it takes back is several at once, and
            // which several is the document's answer — see
            // [`Change::DeleteStep`].
            About::Removes => {
                self.close();
                let Shaped::Took(steps) = document.apply(build, change) else {
                    panic!("a change that removes steps says which ones it took");
                };
                // Nothing where the document refused — a world plane is not a
                // step anybody may take out. A refusal is nothing done, so there
                // is nothing to take back, and an empty step on the stack would
                // be an undo that looked broken.
                if !steps.is_empty() {
                    self.record(Edit::Removed { steps });
                }
                None
            }
            // The camera, which is not the drawing: it lands and is not
            // recorded, so there is nothing here to take back.
            About::Nothing => {
                document.apply(build, change);
                None
            }
            About::Rewrites { at, coalesces } => {
                // The open step has to be a rewrite *of this step*, not merely
                // open. With one drawing there was nothing else a change could
                // be about; with several, a gesture in one followed by a gesture
                // in another would otherwise extend the first step with the
                // second's far end.
                //
                // One chain rather than a test and a match, so what the test
                // established is what the binding carries — there is no arm left
                // over for a `unreachable!` to stand in.
                if self.open
                    && coalesces
                    && let Some(Edit::Wrote { at: had, after, .. }) = self.edits.last_mut()
                    && *had == at
                {
                    document.apply(build, change);
                    // The open step's far end follows the gesture, in place: a
                    // drag lasting a second rewrites one buffer sixty times
                    // rather than leaving sixty steps to take back one at a
                    // time.
                    document.feature_into(at, after);
                    return None;
                }

                self.close();
                let before = document.feature(at).clone();
                document.apply(build, change);
                let after = document.feature(at).clone();
                if after == before {
                    return None;
                }
                self.record(Edit::Wrote { at, before, after });
                self.open = coalesces;
                None
            }
        }
    }

    /// Put `edit` on the stack as the newest thing done.
    ///
    /// Anything undone and not yet put back is gone the moment something else is
    /// done — there is no longer a history in which it happened.
    fn record(&mut self, edit: Edit) {
        self.edits.truncate(self.applied);
        self.edits.push(edit);
        self.applied = self.edits.len();
        self.forget_the_oldest();
    }

    /// Take back the last step, if there is one.
    fn undo(&mut self, document: &mut Document, build: &mut Build) {
        self.close();
        if !self.can_undo() {
            return;
        }
        self.applied -= 1;
        match &self.edits[self.applied] {
            Edit::Wrote { at, before, .. } => document.restore(build, *at, before),
            // Undoing a creation puts back the step's *absence*, which is the
            // one thing a restore cannot say.
            Edit::Added { at, .. } => document.uproot_all(build, &[*at]),
            // And undoing a removal is the opposite again: the steps come back,
            // each where it sat.
            Edit::Removed { steps } => document.replant_all(build, steps),
        }
    }

    /// Put back the last step taken away, if there is one.
    fn redo(&mut self, document: &mut Document, build: &mut Build) {
        self.close();
        if !self.can_redo() {
            return;
        }
        match &self.edits[self.applied] {
            Edit::Wrote { at, after, .. } => document.restore(build, *at, after),
            // The same step returning under the same name — see
            // [`Timeline::replant`](crate::timeline::Timeline).
            Edit::Added { at, feature } => document.put_again(build, *at, feature.clone()),
            Edit::Removed { steps } => {
                // Gathered rather than walked in place, because taking them out
                // borrows the document this list is not part of — the history
                // holds it, and holding a borrow of one across the other is what
                // the collect avoids. A keypress, and a handful of handles.
                let steps: Vec<FeatureId> = steps.iter().map(|uprooted| uprooted.at).collect();
                document.uproot_all(build, &steps);
            }
        }
        self.applied += 1;
    }

    /// Finish the step a gesture has been extending.
    ///
    /// Idempotent, which it has to be: a release arrives on both passes of a
    /// settling frame, and undo and redo close whatever is open before they
    /// touch anything.
    ///
    /// Nothing is dropped here. A step is opened only by a frame that moved the
    /// drawing, and a gesture cannot wander back to where it began: what a drag
    /// asks for is a point in the world, which is `f32`, so the geometry the
    /// drawing takes from it never lands on the `f64` it started at.
    fn close(&mut self) {
        self.open = false;
    }

    /// Drop the oldest step once there are more than [`DEPTH`] of them.
    ///
    /// `remove(0)` is linear, at a hundred, once per step taken past the cap. A
    /// `VecDeque` would make it constant and would cost the truncate that
    /// throwing away a redo tail needs, which happens just as often.
    fn forget_the_oldest(&mut self) {
        if self.edits.len() > DEPTH {
            self.edits.remove(0);
            self.applied -= 1;
        }
    }
}

/// One thing that was done, and what it takes to undo it.
///
/// Two arms, because a document changes in two shapes and they are undone by
/// different means. Most edits *rewrite* a step that is already there, and
/// putting one back is putting a value back. A creation adds one, and putting
/// that back means taking the step away again — there is no earlier value to
/// restore, because there was no step.
///
/// The split is what roadmap §5 called for, and it is the same split deleting
/// and reordering will want: each is a structural change with an arm of its own,
/// where every value edit shares one.
#[derive(Debug)]
pub(crate) enum Edit {
    /// A step rewritten: what it held at either end of what was done.
    ///
    /// Both ends rather than one and a way to recompute the other, because there
    /// is no recomputing either — see [`History`].
    ///
    /// One step of the timeline rather than the whole of it, because an edit
    /// only ever touches one: every [`Change`] that records anything names the
    /// step it is about, so a record of the document would be storing everything
    /// that did not move alongside the one thing that did.
    ///
    /// A whole [`Feature`] rather than a sketch, because not every edit is to
    /// one: moving a plane rewrites a number in a datum, and carrying a solid
    /// rewrites one in an extrude.
    ///
    /// A whole feature rather than what changed *inside* it, which was costed
    /// and declined. It would need two cases, because [`Snapshot`] rejects the
    /// parameter-vector form for structural edits — parameters are named by
    /// position, so one taken before a point was added names the wrong ones
    /// after. The saving is real, roughly sixfold on the demo, and it lands
    /// almost nowhere: of the changes that record anything exactly one is
    /// cleanly positional, and that one is [`Change::Drag`], which already
    /// coalesces a gesture's every frame into a single step.
    ///
    /// If the memory ever does bite, two cheaper levers come first: lower
    /// [`DEPTH`], or drop `before` — for one feature the `after` states form a
    /// chain, so each `before` is the previous step's `after` for that feature.
    ///
    /// [`Snapshot`]: silverpoint::Snapshot
    Wrote {
        at: FeatureId,
        before: Feature,
        after: Feature,
    },
    /// A step added to the end, and what it holds.
    ///
    /// The feature travels with it because a redo puts the *same* step back
    /// under the same name, and by then the timeline no longer has it to copy.
    Added { at: FeatureId, feature: Feature },
    /// Steps taken out together, in the order they sat.
    ///
    /// **Several, because a delete takes several**: what a user names is one
    /// step, and what goes is that step and everything built on it — see
    /// [`Timeline::doomed`](crate::timeline::Timeline). One `Edit` for the whole
    /// cascade, so one press of undo brings the whole of it back, which is also
    /// what makes the command safe to offer without asking first.
    ///
    /// Each carries where it *sat* and not only what it held, which is the whole
    /// of why this is [`Uprooted`] rather than the pair the arm above uses: a
    /// step put back on the end would be a different recipe.
    ///
    /// In the order they sat, so an undo walks it forwards and a redo walks it
    /// back. Neither has to work the cascade out again, and neither could: by
    /// the time an undo runs, the steps it would have been read from are gone.
    Removed { steps: Vec<Uprooted> },
}

#[cfg(test)]
mod tests;
