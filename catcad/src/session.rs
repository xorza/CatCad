//! What the user is working *with*, as against what they are working *on*.

use crate::drawing::Drawing;
use crate::intent::{Choice, Intent, Intents};
use crate::selection::Selection;
use crate::tool::Tool;

/// What is in hand, and what is picked out.
///
/// The half of the application's state that is not the document: none of it
/// would be written down by saving, none of it is a step to take back, and all
/// of it belongs to *this run* rather than to what has been drawn. That is the
/// same line the [`History`](crate::history::History) sits on the far side of —
/// what the document says, how it came to say it, and what you happen to be
/// holding are three different questions.
///
/// Gathered rather than left as two fields on the app, because the two are never
/// apart: they answer the same group of intents ([`Choice`]), an undo prunes
/// both, and a click that picks something out is the same click that a tool in
/// hand would have taken instead. Which drawing is being edited belongs here too
/// once there is more than one — it is session state of exactly this kind.
#[derive(Debug, Default)]
pub(crate) struct Session {
    tool: Tool,
    selection: Selection,
}

impl Session {
    /// The tool in hand, which is what a click in the viewport means.
    pub(crate) fn tool(&self) -> Tool {
        self.tool
    }

    /// What the next command would act on.
    pub(crate) fn selection(&self) -> &Selection {
        &self.selection
    }

    /// Land everything a frame asked of the session.
    ///
    /// Reads the whole inbox rather than being handed the part that concerns it,
    /// so the match is exhaustive over [`Intent`] and a fourth group could not be
    /// added without this saying what it makes of one.
    pub(crate) fn apply(&mut self, intents: &Intents) {
        for intent in intents.iter() {
            match intent {
                Intent::Choice(Choice::Hold(tool)) => self.tool = tool,
                Intent::Choice(Choice::Select(what)) => self.selection.select(what),
                Intent::Choice(Choice::Include(what)) => self.selection.include(what),
                // The history's, and the document's through it. Landed by
                // `CatCad::apply` right after this, in the order the pointer
                // made them.
                Intent::Step(_) | Intent::Change(_) => {}
            }
        }
    }

    /// Let go of whatever `drawing` no longer holds.
    ///
    /// After the history rather than before it, because a step taken back can
    /// take geometry with it — and a handle left pointing at what has gone would
    /// not simply stop matching. The sketch is restored arenas and all, so the
    /// next entity created takes the very same handle and would come up selected
    /// without anyone having picked it.
    ///
    /// A half-drawn shape hangs off a handle in exactly the same way, and the
    /// undo that takes its first point away leaves it hanging off nothing. The
    /// tool stays in hand and starts over rather than going down: what was taken
    /// back is the point, not the intention to draw.
    pub(crate) fn prune(&mut self, drawing: &Drawing) {
        self.selection.retain(|part| drawing.holds_part(part));
        if self
            .tool
            .started()
            .is_some_and(|anchor| !drawing.holds_anchor(anchor))
        {
            self.tool = self.tool.restarted();
        }
    }
}
