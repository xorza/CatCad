//! What the user is working *with*, as against what they are working *on*.

use crate::intent::{Choice, Intent, Intents};
use crate::model::{Model, Models};
use crate::part::Part;
use crate::prompt::Prompt;
use crate::selection::Selection;
use crate::timeline::FeatureId;
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
#[derive(Debug)]
pub(crate) struct Session {
    tool: Tool,
    selection: Selection,
    /// The sketch open for editing, where one is.
    ///
    /// What every edit names, and so what decides which sketch a click builds
    /// in. Session state like the rest of this: nothing about what you happen
    /// to have open is written down by saving, and an undo should not close the
    /// sketch you were working in.
    ///
    /// **`None` is a document being looked at rather than drawn in**, which is
    /// a state in its own right and not a mistake to guard against: a document
    /// may hold no sketch at all, and one that holds several is opened on none
    /// of them. Every reader below answers for it — what a click means, what
    /// the tool bar offers, what the readout says and what a prune has to
    /// let go of.
    editing: Option<FeatureId>,
    /// The form open against the drawing, where one is.
    ///
    /// Session state on the same terms as the rest: a draft is not in the
    /// document until it is committed, an undo should not reopen a form, and
    /// opening one is not a step to take back. See [`Prompt`].
    prompt: Option<Prompt>,
}

impl Session {
    /// A session in `editing`, holding nothing and with nothing picked out —
    /// which is what raising a document leaves.
    pub(crate) fn new(editing: Option<FeatureId>) -> Self {
        Self {
            tool: Tool::default(),
            selection: Selection::default(),
            editing,
            prompt: None,
        }
    }

    /// The tool in hand, which is what a click in the viewport means.
    pub(crate) fn tool(&self) -> Tool {
        self.tool
    }

    /// The sketch open for editing, where one is.
    pub(crate) fn editing(&self) -> Option<FeatureId> {
        self.editing
    }

    /// What the next command would act on.
    pub(crate) fn selection(&self) -> &Selection {
        &self.selection
    }

    /// The form open against the drawing, where one is.
    pub(crate) fn prompt(&self) -> Option<&Prompt> {
        self.prompt.as_ref()
    }

    /// The same, to show and be typed into.
    ///
    /// The one thing here written outside [`Session::apply`], and the reason is
    /// that the writer is a *widget*: what a keystroke does to a line is
    /// palantir's, decided where the form is shown, which is the asking half of
    /// a frame. What the draft then comes to — a value on the document, a form
    /// put away — goes through the inbox like everything else, so the document
    /// is still written in one place at one time.
    pub(crate) fn prompt_mut(&mut self) -> Option<&mut Prompt> {
        self.prompt.as_mut()
    }

    /// Land everything a frame asked of the session.
    ///
    /// Reads the whole inbox rather than being handed the part that concerns it,
    /// so the match is exhaustive over [`Intent`] and a fourth group could not be
    /// added without this saying what it makes of one.
    ///
    /// `models` is the drawing the *asking* was read against, and the reason it
    /// is wanted here is one intent: opening a form over a region turns a
    /// position into a durable name, and a position is only good for the
    /// arrangement it came from. So this is called before the history writes —
    /// afterwards the positions in the inbox would be being resolved against a
    /// drawing they were never read from. See
    /// [`Asking::Extrude`](crate::prompt::Asking).
    pub(crate) fn apply(&mut self, models: Models<'_>, intents: &Intents) {
        for intent in intents.iter() {
            match intent {
                // A tool put down or taken up abandons whatever the last one
                // was half-way through, and a form asking about that half is
                // part of it — see [`Tool`]. Closed here rather than by
                // whoever raises the `Hold`, so the three ways to put a tool
                // down cannot come to disagree.
                Intent::Choice(Choice::Hold(tool)) => {
                    if !self.tool.is(tool) {
                        self.prompt = None;
                    }
                    // A tool draws, and drawing wants somewhere to draw. With
                    // no sketch open the bar offers none of them — see
                    // [`Hud::tools`](crate::hud::Hud) — and this is the other
                    // half of that rule rather than a second one: the bar says
                    // what can be asked for, and this says what is answered.
                    self.tool = match self.editing {
                        Some(_) => tool,
                        None => Tool::Pointer,
                    };
                }
                // Everything the session holds is about a sketch, so closing one
                // puts all of it down: what is in hand draws in the sketch you
                // are in, and a form stands against it.
                //
                // What is picked *out* stays. A selection may name parts of any
                // sketch and of no sketch at all — a plane, a solid's face — and
                // none of that stops meaning anything for your having left the
                // drawing it was picked from.
                Intent::Choice(Choice::Close) => {
                    self.editing = None;
                    self.tool = Tool::Pointer;
                    self.prompt = None;
                }
                // Picking something out opens the sketch it came from. The
                // one gesture that says which sketch you mean is the one that
                // says which *thing* you mean, so there is no second one to
                // learn — and a click that names no sketch says nothing about
                // which, so it leaves open whatever was. Empty space is one
                // such click; a datum plane is the other, being what sketches
                // are drawn on rather than anything drawn.
                Intent::Choice(Choice::Select(what)) => {
                    self.editing = what.and_then(Part::sketch).or(self.editing);
                    self.selection.select(what);
                }
                Intent::Choice(Choice::Include(what)) => {
                    self.editing = what.sketch().or(self.editing);
                    self.selection.include(what);
                }
                Intent::Choice(Choice::Ask(Some(opening))) => {
                    // What a form for `opening` is made of is
                    // [`Prompt::opening`]'s — which fields it shows and what
                    // each starts out saying is the form's business rather than
                    // the session's. What is left here is *which* form is open,
                    // which is session state and nobody else's.
                    let Some(opened) = Prompt::opening(opening, models) else {
                        continue;
                    };
                    // A second open of the form already open would start its
                    // drafts over, which is why the guard is here rather than
                    // left to whoever raises this: a double-click on a field
                    // already open should place a caret, not undo the typing.
                    if self
                        .prompt
                        .as_ref()
                        .is_none_or(|open| open.about() != opened.about())
                    {
                        self.prompt = Some(opened);
                    }
                }
                Intent::Choice(Choice::Ask(None)) => self.prompt = None,
                // Landing on no form is landing on nothing: a drag that
                // outlived the form it was writing into has nothing to say.
                Intent::Choice(Choice::Set { nth, to }) => {
                    if let Some(open) = self.prompt.as_mut() {
                        open.write(nth, to);
                    }
                }
                Intent::Choice(Choice::Suggest { nth, to }) => {
                    if let Some(open) = self.prompt.as_mut() {
                        open.suggest(nth, to);
                    }
                }
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
        // A dimension an undo took back is one there is nothing left to
        // restate, and a form left open over it would commit onto a handle
        // naming nothing. The draft goes with it: what was typed was about
        // *that* dimension. A form about something the timeline does not hold
        // yet — an extrude still being decided — has nothing here to lose.
        if self
            .prompt
            .as_ref()
            .and_then(Prompt::marks)
            .is_some_and(|part| !models.holds(part))
        {
            self.prompt = None;
        }
        // The tool draws in the open sketch and nowhere else, so what it is
        // half-way through hangs off that one — asking the rest would be asking
        // after a point they never held. And with no sketch open there is
        // nothing for a gesture to hang *from*: what a tool is half-way through
        // is a place or a handle in a drawing, and there is no drawing.
        let Some(drawing) = models.open().map(Model::drawing) else {
            self.tool = self.tool.restarted();
            return;
        };
        let hanging = self
            .tool
            .started()
            .is_some_and(|anchor| !drawing.holds_anchor(anchor))
            // And what it has *picked*, which is the other way a gesture hangs
            // off geometry: the dimension tool names entities rather than
            // places, and a handle to something an undo took away is not merely
            // inert — the sketch is restored arenas and all, so the next entity
            // created takes the very same handle and the tool would come back
            // pointing at something nobody picked.
            || self.tool.picking().any(|entity| !drawing.holds(entity))
            // And whether what it is half-way through still *means* anything,
            // which is not the same question: an undo can leave every handle
            // good and the gesture dead all the same — see [`Tool::stands`].
            //
            // Last, and load-bearing that it is last. This reads the geometry
            // the tool picked, and reading a handle the sketch no longer holds
            // is a panic rather than a `false`; the test above has already
            // answered for that, and `||` is what makes the order count.
            || !self.tool.stands(drawing.sketch());
        if hanging {
            self.tool = self.tool.restarted();
        }
    }
}

/// What a harness raising a document reaches for.
///
/// A document is opened on no sketch — see
/// [`Document::opening`](crate::document::Document) — and every harness in this
/// crate is about a drawing being *worked in*, which wants one open. So the way
/// in is here, once, rather than spelled out by each of them.
#[cfg(any(test, feature = "internals"))]
mod internals {
    use crate::build::Build;
    use crate::document::Document;
    use crate::intent::{Choice, Intents};
    use crate::part::Part;
    use crate::session::Session;

    impl Session {
        /// Open `document`'s first sketch, the way a user opens one: by
        /// clicking something in it.
        ///
        /// What a user would do rather than a write of the field, and through
        /// the inbox like every other change to a session — a harness that set
        /// it would be testing against a session no gesture could produce.
        ///
        /// Two clicks and not one. The click that opens a sketch also picks out
        /// what it landed on, and a harness that left that standing would hand
        /// every test a selection it never asked for; the second lands on empty
        /// space, which clears it and says nothing about which sketch — see
        /// [`Choice::Select`].
        pub(crate) fn enter_first_sketch(&mut self, document: &Document, build: &Build) {
            let sketch = document.first_sketch();
            let (entity, _) = document
                .drawing_at(sketch)
                .sketch()
                .points()
                .next()
                .expect("a document is raised on a sketch with a point to click");
            let part = Part::Entity {
                sketch,
                entity: entity.into(),
            };
            let mut intents = Intents::default();
            intents.push(Choice::Select(Some(part)));
            intents.push(Choice::Select(None));
            self.apply(document.models(build, self.editing), &intents);
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
    use crate::timeline::feature::{Datum, Feature, World};
    use crate::tool::dimensioning::Dimensioning;
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
        let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
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
        let first = Models::new(&timeline, &build, Some(here))
            .open()
            .expect("a fixture opens the sketch it names");
        let second = Models::new(&timeline, &build, Some(there))
            .open()
            .expect("a fixture opens the sketch it names");

        let mut session = Session::new(Some(here));

        let mut intents = Intents::default();
        intents.push(Choice::Select(Some(second.part(other))));
        session.apply(Models::new(&timeline, &build, session.editing()), &intents);
        assert_eq!(
            session.editing(),
            Some(there),
            "the pick did not open its sketch"
        );

        // A click on empty space picks nothing out and says nothing about which
        // sketch, so the one that was open stays open.
        intents.clear();
        intents.push(Choice::Select(None));
        session.apply(Models::new(&timeline, &build, session.editing()), &intents);
        assert_eq!(
            session.editing(),
            Some(there),
            "an empty click closed the sketch"
        );
        assert_eq!(session.selection().count(), 0);

        // Shift-adding does the same as a plain pick, because it too names a
        // thing and so names a sketch.
        intents.clear();
        intents.push(Choice::Include(first.part(Entity::Point(one))));
        session.apply(Models::new(&timeline, &build, session.editing()), &intents);
        assert_eq!(session.editing(), Some(here));

        // A datum plane is the other click that names no sketch: it is what
        // sketches are drawn *on*, so picking one leaves open whatever was —
        // and it stays picked out, because the document still holds it.
        intents.clear();
        intents.push(Choice::Select(Some(Part::Plane(ground))));
        session.apply(Models::new(&timeline, &build, session.editing()), &intents);
        assert_eq!(
            session.editing(),
            Some(here),
            "picking a plane closed the sketch"
        );
        assert!(session.selection().contains(Part::Plane(ground)));

        let models = Models::new(&timeline, &build, Some(here));
        session.prune(models);
        assert!(
            session.selection().contains(Part::Plane(ground)),
            "a plane the document still holds was pruned out of the selection"
        );
    }

    /// A prune puts down a gesture whose geometry has stopped meaning anything,
    /// not only one whose geometry has gone.
    ///
    /// The two are different failures and only one of them leaves a handle
    /// dangling. A dimension half-placed over two points an undo has brought
    /// together still names two points the sketch holds — nothing is dangling at
    /// all — and there is no longer a distance between them to state, so every
    /// click the tool took from then on would do nothing and say nothing about
    /// why.
    ///
    /// The last part is the order, which is load-bearing: asking what a gesture
    /// still *means* reads the geometry it picked, and reading a handle the
    /// sketch no longer holds is a panic rather than an answer. So a pair that
    /// has been deleted has to be answered by the test *before* it, and the way
    /// to see that is that this does not panic.
    #[test]
    fn a_prune_puts_down_a_gesture_that_has_stopped_meaning_anything() {
        // Two points `apart`, and the timeline holding them. Rebuilt rather than
        // moved, which is exact where a solve would land near enough: two fresh
        // arenas mint the same handles, so the gesture below names the same pair
        // in either — see
        // `two_sketches_mint_the_same_handles_and_are_told_apart_by_the_part`.
        let raised = |apart: DVec2| {
            let mut sketch = Sketch::default();
            let here = sketch.add_point(DVec2::ZERO);
            let there = sketch.add_point(apart);
            let mut timeline = Timeline::default();
            let ground = timeline.add(Feature::Plane(Datum::World(World::Ground)));
            let at = timeline.add(Feature::Sketch { on: ground, sketch });
            let mut build = Build::default();
            timeline.edit(at).opened(&mut build);
            (timeline, build, at, [here, there])
        };

        let (timeline, build, at, [here, there]) = raised(DVec2::new(3.0, 4.0));
        let placing = Tool::Dimension(Dimensioning::Placing {
            first: Entity::Point(here),
            second: Some(Entity::Point(there)),
            along: None,
        });
        let mut session = Session::new(Some(at));
        let mut intents = Intents::default();
        intents.push(Choice::Hold(placing));
        session.apply(Models::new(&timeline, &build, Some(at)), &intents);

        // Three-four-five apart, so the gesture still has a distance to state
        // and the prune has nothing to say about it.
        session.prune(Models::new(&timeline, &build, Some(at)));
        assert_eq!(session.tool(), placing, "a live gesture was put down");

        // In one place, with both handles still perfectly good.
        let (together, build, at, _) = raised(DVec2::ZERO);
        session.prune(Models::new(&together, &build, Some(at)));
        assert_eq!(
            session.tool(),
            Tool::Dimension(Dimensioning::Empty),
            "a gesture with nothing left to state stayed in hand"
        );

        // And a pair that has gone outright is answered without ever asking what
        // it would have meant — which is a panic rather than a `false`, so the
        // whole of the claim is that this line returns.
        let (mut timeline, mut build, at, _) = raised(DVec2::new(3.0, 4.0));
        session.apply(Models::new(&timeline, &build, Some(at)), &intents);
        timeline.edit(at).remove(&mut build, Entity::Point(there));
        session.prune(Models::new(&timeline, &build, Some(at)));
        assert_eq!(
            session.tool(),
            Tool::Dimension(Dimensioning::Empty),
            "a gesture hanging off geometry that has gone stayed in hand"
        );
    }
}
