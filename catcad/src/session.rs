//! What the user is working *with*, as against what they are working *on*.

use crate::intent::{Choice, Intent, Intents};
use crate::model::Models;
use crate::part::Part;
use crate::selection::Selection;
use crate::timeline::FeatureId;
use crate::tool::Tool;
use crate::typing::Typing;

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
#[derive(Debug)]
pub(crate) struct Session {
    tool: Tool,
    selection: Selection,
    /// The sketch open for editing.
    ///
    /// What every edit names, and so what decides which sketch a click builds
    /// in. Session state like the rest of this: nothing about what you happen
    /// to have open is written down by saving, and an undo should not close the
    /// sketch you were working in.
    ///
    /// Never absent, because a session is always *in* something: a document is
    /// raised with its first sketch open and nothing here closes one. When
    /// looking at a document without a sketch open becomes a thing a user can
    /// do, this becomes an `Option` and the compiler will point at every place
    /// that then has to answer for it.
    editing: FeatureId,
    /// The dimension being retyped, where one is.
    ///
    /// Session state on the same terms as the rest: a draft is not in the
    /// document until it is committed, an undo should not reopen a field, and
    /// opening one is not a step to take back. See [`Typing`].
    typing: Option<Typing>,
}

impl Session {
    /// A session in `editing`, holding nothing and with nothing picked out —
    /// which is what raising a document leaves.
    pub(crate) fn new(editing: FeatureId) -> Self {
        Self {
            tool: Tool::default(),
            selection: Selection::default(),
            editing,
            typing: None,
        }
    }

    /// The tool in hand, which is what a click in the viewport means.
    pub(crate) fn tool(&self) -> Tool {
        self.tool
    }

    /// The sketch open for editing.
    pub(crate) fn editing(&self) -> FeatureId {
        self.editing
    }

    /// What the next command would act on.
    pub(crate) fn selection(&self) -> &Selection {
        &self.selection
    }

    /// The dimension being retyped, where one is.
    pub(crate) fn typing(&self) -> Option<&Typing> {
        self.typing.as_ref()
    }

    /// The same, to type into.
    ///
    /// The one thing here written outside [`Session::apply`], and the reason is
    /// on [`Typing`]: a keystroke says how far to go rather than where to end
    /// up, so it cannot be an intent under the rule the inbox is built on. What
    /// *opens* and *closes* a field still goes through the inbox.
    pub(crate) fn typing_mut(&mut self) -> Option<&mut Typing> {
        self.typing.as_mut()
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
                // Picking something out opens the sketch it came from. The
                // one gesture that says which sketch you mean is the one that
                // says which *thing* you mean, so there is no second one to
                // learn — and a click that names no sketch says nothing about
                // which, so it leaves open whatever was. Empty space is one
                // such click; a datum plane is the other, being what sketches
                // are drawn on rather than anything drawn.
                Intent::Choice(Choice::Select(what)) => {
                    self.editing = what.and_then(Part::sketch).unwrap_or(self.editing);
                    self.selection.select(what);
                }
                Intent::Choice(Choice::Include(what)) => {
                    self.editing = what.sketch().unwrap_or(self.editing);
                    self.selection.include(what);
                }
                // Seeded from what the dimension says now, and picked out
                // whole — see [`Typing::on`]. A second open of the one already
                // open would start the draft over, which is why the guard is
                // here rather than left to whoever raises this: a double-click
                // on a field already open should place a caret, not undo the
                // typing.
                Intent::Choice(Choice::Type(Some(typed))) => {
                    if self
                        .typing
                        .as_ref()
                        .is_none_or(|open| open.part() != typed.part)
                    {
                        self.typing = Some(Typing::on(typed.part, typed.from));
                    }
                }
                Intent::Choice(Choice::Type(None)) => self.typing = None,
                // The history's, and the document's through it. Landed by
                // `CatCad::apply` right after this, in the order the pointer
                // made them.
                //
                // An errand is the application's, and is the one that takes
                // this away: opening a file leaves a session that was never
                // started rather than this one carried over. See
                // [`CatCad::run`](crate::CatCad).
                Intent::Step(_) | Intent::Change(_) | Intent::Errand(_) => {}
            }
        }
    }

    /// Let go of whatever the model no longer holds.
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
    pub(crate) fn prune(&mut self, models: Models<'_>) {
        self.selection.retain(|part| models.holds(part));
        // A dimension an undo took back is one there is nothing left to retype,
        // and a field left open over it would commit onto a handle naming
        // nothing. The draft goes with it: what was typed was about *that*
        // dimension.
        if self
            .typing
            .as_ref()
            .is_some_and(|open| !models.holds(open.part()))
        {
            self.typing = None;
        }
        // The tool draws in the open sketch and nowhere else, so what it is
        // half-way through hangs off that one — asking the rest would be asking
        // after a point they never held.
        if self
            .tool
            .started()
            .is_some_and(|anchor| !models.open().drawing().holds_anchor(anchor))
        {
            self.tool = self.tool.restarted();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::build::Build;
    use crate::intent::Choice;
    use crate::model::Models;
    use crate::timeline::Timeline;
    use crate::timeline::feature::{Datum, Feature};
    use glam::DVec2;
    use silverpoint::{Entity, Sketch};

    /// Picking something out opens the sketch it came from, and clicking
    /// nothing leaves open whatever was.
    ///
    /// The one gesture that says which sketch you mean is the one that says
    /// which *thing* you mean, so there is no second one to learn. The empty
    /// click is the half worth pinning: it says nothing about which sketch, and
    /// a rule that read it as "none" would drop you out of the sketch you were
    /// working in every time you clicked past the geometry.
    #[test]
    fn picking_something_out_opens_the_sketch_it_came_from() {
        let mut timeline = Timeline::default();
        let ground = timeline.add(Feature::Plane(Datum::Ground));
        let mut lone = || {
            let mut sketch = Sketch::default();
            let point = sketch.add_point(DVec2::ZERO);
            (timeline.add(Feature::Sketch { on: ground, sketch }), point)
        };
        let (here, one) = lone();
        let (there, other) = lone();

        let mut build = Build::default();
        timeline.edit(here).opened(&mut build);
        timeline.edit(there).opened(&mut build);
        let first = Models::new(&timeline, &build, here).open();
        let second = Models::new(&timeline, &build, there).open();

        let mut session = Session::new(here);

        let mut intents = Intents::default();
        intents.push(Choice::Select(Some(second.part(other))));
        session.apply(&intents);
        assert_eq!(session.editing(), there, "the pick did not open its sketch");

        // A click on empty space picks nothing out and says nothing about which
        // sketch, so the one that was open stays open.
        intents.clear();
        intents.push(Choice::Select(None));
        session.apply(&intents);
        assert_eq!(session.editing(), there, "an empty click closed the sketch");
        assert_eq!(session.selection().count(), 0);

        // Shift-adding does the same as a plain pick, because it too names a
        // thing and so names a sketch.
        intents.clear();
        intents.push(Choice::Include(first.part(Entity::Point(one))));
        session.apply(&intents);
        assert_eq!(session.editing(), here);

        // A datum plane is the other click that names no sketch: it is what
        // sketches are drawn *on*, so picking one leaves open whatever was —
        // and it stays picked out, because the document still holds it.
        intents.clear();
        intents.push(Choice::Select(Some(Part::Plane(ground))));
        session.apply(&intents);
        assert_eq!(session.editing(), here, "picking a plane closed the sketch");
        assert!(session.selection().contains(Part::Plane(ground)));

        let models = Models::new(&timeline, &build, here);
        session.prune(models);
        assert!(
            session.selection().contains(Part::Plane(ground)),
            "a plane the document still holds was pruned out of the selection"
        );
    }
}
