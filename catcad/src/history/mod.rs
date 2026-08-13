//! What has been done to the document, and how to take it back.

use crate::document::Document;
use crate::drawing::Standing;
use crate::intent::{Intent, Intents};

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
    /// Where the drawing stood before the intent now being applied, and where
    /// it stands after. Scratch, so a drag reuses two buffers rather than
    /// taking a pair every frame it lasts.
    before: Standing,
    after: Standing,
}

impl History {
    /// Land everything a frame asked for, recording whatever moved the drawing.
    ///
    /// Answers whether the drawing has to be laid out again — true for a drag
    /// that went somewhere and for an undo or redo that had something to do,
    /// false for a camera that merely turned.
    pub(crate) fn apply(&mut self, document: &mut Document, intents: &Intents) -> bool {
        let mut moved = false;
        for intent in intents.iter() {
            moved |= match intent {
                Intent::Undo => self.undo(document),
                Intent::Redo => self.redo(document),
                Intent::Release => {
                    self.close();
                    false
                }
                edit => self.step(document, edit),
            };
        }
        moved
    }

    /// Whether there is anything to take back.
    fn can_undo(&self) -> bool {
        self.applied > 0
    }

    /// Whether there is anything to put back.
    fn can_redo(&self) -> bool {
        self.applied < self.edits.len()
    }

    /// Do what `intent` asks, and record it if it moved the drawing.
    ///
    /// The comparison is the whole of what decides that, and it earns two
    /// things at once rather than being told either. Turning the camera records
    /// nothing, because the camera is not the drawing — which is the CAD
    /// convention, arrived at rather than declared, and with no list of exempt
    /// intents to keep in step with the intents. And a drag the constraints
    /// refuse records nothing, because
    /// [`Solver::edit_holding`](silverpoint::Solver) has already put the
    /// geometry back by the time this looks.
    fn step(&mut self, document: &mut Document, intent: Intent) -> bool {
        let extending = self.open && intent.coalesces();
        if !extending {
            self.close();
            document.drawing().snapshot_into(&mut self.before);
        }
        document.apply(intent);
        document.drawing().snapshot_into(&mut self.after);

        if extending {
            // The open step's far end follows the gesture, in place: a drag
            // lasting a second rewrites one buffer sixty times rather than
            // leaving sixty steps to take back one at a time.
            let open = self.edits.last_mut().expect("an open step is on the stack");
            let moved = self.after.moved_from(&open.after);
            open.after.clone_from(&self.after);
            return moved;
        }

        if !self.after.moved_from(&self.before) {
            return false;
        }
        // Anything undone and not yet put back is gone the moment something
        // else is done — there is no longer a history in which it happened.
        self.edits.truncate(self.applied);
        self.edits.push(Edit {
            before: self.before.clone(),
            after: self.after.clone(),
        });
        self.applied = self.edits.len();
        self.open = intent.coalesces();
        self.forget_the_oldest();
        true
    }

    /// Take back the last step. Answers whether there was one.
    fn undo(&mut self, document: &mut Document) -> bool {
        self.close();
        if !self.can_undo() {
            return false;
        }
        self.applied -= 1;
        document
            .drawing_mut()
            .restore(&self.edits[self.applied].before);
        true
    }

    /// Put back the last step taken away. Answers whether there was one.
    fn redo(&mut self, document: &mut Document) -> bool {
        self.close();
        if !self.can_redo() {
            return false;
        }
        document
            .drawing_mut()
            .restore(&self.edits[self.applied].after);
        self.applied += 1;
        true
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

/// One step there and back: the drawing at each end of something that was done.
///
/// Both ends rather than one and a way to recompute the other, because there is
/// no recomputing either — see [`History`].
#[derive(Debug)]
struct Edit {
    before: Standing,
    after: Standing,
}

#[cfg(test)]
mod tests;
