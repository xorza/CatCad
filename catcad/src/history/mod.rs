//! What has been done to the document, and how to take it back.

use crate::build::Build;
use crate::document::Document;
use crate::intent::{Change, Intent, Intents, Step};
use crate::timeline::FeatureId;
use crate::timeline::feature::Feature;

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
/// **State, not actions.** A step records where the drawing was and where it
/// went, and undoing puts a value back rather than doing the opposite of what
/// was done. There is no choice about that: an edit is followed by a solve, and
/// a solve moves whatever the constraints couple to what was edited. Dragging a
/// point back to where it started runs a *different* solve from a *different*
/// starting guess, and one that can settle on a different branch — the demo's
/// own hub admits a mirrored solution. And a drag the constraints refuse did
/// nothing at all, which has no opposite to do.
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
    /// Says nothing about what it did. Whether the drawing moved is a fact
    /// about the drawing, and anything that needs it reads
    /// [`Build::revision`](crate::build::Build::revision) — which cannot
    /// be forgotten to pass on, or passed on wrongly.
    pub(crate) fn apply(&mut self, document: &mut Document, build: &mut Build, intents: &Intents) {
        for intent in intents.iter() {
            match intent {
                Intent::Step(Step::Undo) => self.undo(document, build),
                Intent::Step(Step::Redo) => self.redo(document, build),
                Intent::Step(Step::Release) => self.close(),
                Intent::Change(change) => self.edit(document, build, change),
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
    /// A change that names no sketch is the camera's, and the camera is not the
    /// drawing — so it lands and is not recorded. Which side of that line a
    /// change falls on is [`Change::sketch`]'s to say, and it says it with an
    /// exhaustive match: a variant added later cannot quietly join either side
    /// without the compiler asking which.
    ///
    /// Everything else is recorded only if it moved the sketch it named, and
    /// the comparison is the whole of what decides that. A drag the constraints
    /// refuse records nothing, because
    /// [`Solver::drag`](silverpoint::Solver) has already put the
    /// geometry back by the time this looks.
    fn edit(&mut self, document: &mut Document, build: &mut Build, change: Change) {
        // Asked before anything else, because a step that was not there has no
        // *before* to compare against — the whole shape below is about what a
        // step held on either side of an edit, and a creation has only one side.
        if change.creates() {
            self.close();
            let at = document
                .apply(build, change)
                .expect("a change that creates says which step it made");
            let feature = document.feature(at).clone();
            self.record(Edit::Added { at, feature });
            return;
        }
        let Some(at) = change.feature() else {
            document.apply(build, change);
            return;
        };
        // The open step has to name the same step of the timeline, not merely
        // be open. With one drawing there was nothing else a change could be
        // about; with several, a gesture in one followed by a gesture in
        // another would otherwise extend the first step with the second's far
        // end.
        let extending = self.open
            && change.coalesces()
            && self.edits.last().is_some_and(|edit| edit.rewrote(at));
        if extending {
            document.apply(build, change);
            // The open step's far end follows the gesture, in place: a drag
            // lasting a second rewrites one buffer sixty times rather than
            // leaving sixty steps to take back one at a time.
            let Some(Edit::Wrote { after, .. }) = self.edits.last_mut() else {
                unreachable!("an open step is a rewrite, and one is on the stack");
            };
            document.feature_into(at, after);
            return;
        }

        self.close();
        let before = document.feature(at).clone();
        document.apply(build, change);
        let after = document.feature(at).clone();
        if after == before {
            return;
        }
        self.record(Edit::Wrote { at, before, after });
        self.open = change.coalesces();
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
            Edit::Added { at, .. } => document.take_back(build, *at),
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
            // [`Timeline::append`](crate::timeline::Timeline).
            Edit::Added { at, feature } => document.put_again(build, *at, feature.clone()),
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
}

impl Edit {
    /// Whether this is a rewrite of the step at `at`, which is what a gesture
    /// still under way extends.
    ///
    /// A creation is never extended: it happened once, and the next thing the
    /// pointer does is a different thing done.
    fn rewrote(&self, at: FeatureId) -> bool {
        matches!(self, Edit::Wrote { at: had, .. } if *had == at)
    }
}

#[cfg(test)]
mod tests;
