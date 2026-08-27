//! What a frame asks for, and the queue it asks through.
//!
//! One enum per kind of asking — an edit to the drawing, a step of the
//! history, something the session should take up, an errand for the shell —
//! and [`Intent`] over the four of them. The edits are the long half and live
//! in [`change`].

pub(crate) mod change;

use crate::drawing::anchor::Anchor;

use crate::intent::change::Change;
use crate::part::Part;
use crate::timeline::FeatureId;
use crate::tool::Tool;

/// One thing the user asked for.
///
/// Asked for rather than done. Whatever raises one is handed what it needs to
/// read and never to write, so a gesture arrives as a request and lands in one
/// place afterwards — which is what leaves a single point every change passes
/// through. Most land on the document, and an undo stack watches there; so,
/// later, will whatever decides a document has gone unsaved. The rest land on
/// what is not the document but is still the user's to change: which step of
/// the history is current, and which tool is in hand.
///
/// **Every one names where it wants to end up, never how far to go.** A
/// settling frame records twice and palantir may replay a pass up to three
/// times, so an intent that said "toggle" or "move by" would land two or three
/// times over. That is why a drag names a point in the world, the projection
/// toggle names the projection it wants, and the toolbar names the tool it
/// wants held rather than saying that it was pressed.
///
/// **Which of the three it is decides where it lands**, and the type says so.
/// A [`Change`] is the document's to answer, a [`Step`] the history's and a
/// [`Choice`] the application's; each of the three is handed its own payload
/// and can neither be given nor forget one of the others. One enum over the
/// three rather than three inboxes, because order is promised across all of
/// them: a dolly and a drag in one frame must land the way the pointer made
/// them — see [`Intents::iter`].
///
/// `Copy`, so applying one can lift it out of the inbox and let go of the
/// borrow before touching what it lands on.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Intent {
    /// The document's to answer — see [`Document::apply`](crate::document::Document).
    Change(Change),
    /// The history's.
    Step(Step),
    /// The [`Session`](crate::session::Session)'s, and nobody else's: none of it
    /// is in the document, and none of it is a step to take back.
    Choice(Choice),
    /// The application's own — the one group that lands on none of the three
    /// things below it, because what it does is *replace* them.
    Errand(Errand),
}

impl From<Change> for Intent {
    fn from(change: Change) -> Self {
        Intent::Change(change)
    }
}

impl From<Step> for Intent {
    fn from(step: Step) -> Self {
        Intent::Step(step)
    }
}

impl From<Choice> for Intent {
    fn from(choice: Choice) -> Self {
        Intent::Choice(choice)
    }
}

impl From<Errand> for Intent {
    fn from(errand: Errand) -> Self {
        Intent::Errand(errand)
    }
}

/// What the history answers: where in what has been done the document stands.
///
/// None of it changes what the document *says* — an undo puts a snapshot back,
/// and the other two do not touch it at all — which is why they are the
/// history's and reach the document only through it.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Step {
    /// The drag let go.
    ///
    /// Changes nothing by itself — it closes the step the drag has been
    /// extending, so a gesture is one thing to take back rather than one per
    /// frame it lasted.
    Release,
    /// Take back the last step, and put it back.
    Undo,
    Redo,
}

/// What the [`Session`](crate::session::Session) answers: which tool to take up,
/// and what to pick out.
///
/// Named for the choosing rather than for where it lands, like its two peers,
/// and because that is what the three have in common — none of them asks for
/// anything to be *done*, only for what the next thing done will be done with.
///
/// Neither is in the document and neither is a step to take back — an undo puts
/// back a point the tool placed and leaves what is in your hand alone. Asked for
/// through the same inbox all the same, because the order still matters and
/// because three things can put a tool down: a replayed pass that flipped it
/// where it was pressed would arm it and put it straight back down.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Choice {
    /// Pick out this part of the drawing and nothing else, or nothing at all
    /// when it is `None`.
    ///
    /// The whole of what is selected rather than one addition to it, so a
    /// replayed pass lands on the same answer — see the note on naming above.
    /// A plain click raises one whatever it landed on: `None` is what a click
    /// on empty space asks for, by the same rule that a click on a point asks
    /// for that point.
    Select(Option<Part>),
    /// Pick this out as well as whatever already is.
    ///
    /// What a shift-click asks for. Names an addition where [`Choice::Select`]
    /// names the whole, and is safe to land twice for a different reason: a
    /// part already picked out is not picked out again.
    Include(Part),
    /// Open a form against the drawing, or close whatever is open by naming
    /// `None`.
    ///
    /// Names what should be open rather than "the thing under the cursor", so a
    /// replayed pass opens the same form rather than reading a hover that has
    /// since moved on — and closing twice closes nothing twice.
    ///
    /// Carries what the form starts out saying, because a
    /// [`Session`](crate::session::Session) is handed the inbox and not the
    /// drawing: whoever raised this had the geometry in hand and can read it,
    /// where the session would have to be given the whole document to ask. What
    /// is typed *into* a draft afterwards does not come through here at all —
    /// see [`Prompt`](crate::prompt::Prompt).
    Ask(Option<Opening>),
    /// Put `to` in the open form's `nth` field.
    ///
    /// What a handle in the drawing writes. A drag on the arrow carrying a
    /// solid's depth is not an edit — the solid is a form's own reading and the
    /// document has not heard of it — so what the gesture moves is the draft,
    /// and the form still decides what that comes to.
    ///
    /// Names the value it wants rather than a step along the way, like every
    /// change: a drag sends one of these a frame and a replayed pass restates
    /// the same number.
    Set { nth: usize, to: f64 },
    /// Show `to` in the open form's `nth` field while nobody has typed into it.
    ///
    /// What a pointer merely *moving* says, where [`Choice::Set`] is what a
    /// drag says. The difference is deliberateness, and it decides whether the
    /// value may overwrite one somebody typed: a hover may not, a drag may.
    Suggest { nth: usize, to: f64 },
    /// Close the sketch being worked in, leaving the document open on none.
    ///
    /// Names the state it wants like everything here, so a replayed pass closes
    /// nothing twice. There is no `Open` beside it because nothing would raise
    /// one: picking something out already opens the sketch it came from — see
    /// [`Choice::Select`] — and that is the one gesture that says which sketch
    /// you mean, because it is the one that says which *thing* you mean.
    ///
    /// Puts down the tool and the form as well, and that is not a second thing:
    /// a tool draws in the sketch you are in and a form is open against it, so
    /// neither has anything left to be about.
    Close,
    /// Take up this tool, or put down whatever is in hand by naming
    /// [`Tool::Pointer`].
    ///
    /// The tool the user wants held, not the button they pressed. Pressing an
    /// armed tool's button puts it down, which is a *toggle* — so the toolbar
    /// works out what that leaves and names it, rather than asking for a flip a
    /// replayed pass would perform twice. See the note on naming above.
    Hold(Tool),
}

/// A form to open, and what it starts out saying.
///
/// Its own type rather than fields on the variant, because it is also what a
/// caller works out in one place — the double-click that opens a dimension has
/// to establish both halves, and answering with a pair would leave which was
/// which to the call site.
///
/// One arm per operation, mirroring [`Asking`](crate::prompt::Asking): this is
/// the *request* and that is what the form became, and keeping them apart is
/// what lets a request carry a seed the form does not go on holding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) enum Opening {
    /// Restate a dimension, seeded with what it measures now.
    Dimension { part: Part, from: f64 },
    /// Draw a circle about a centre already placed, seeded at no size at all.
    ///
    /// Opened by the click that puts the centre down, so the form is standing
    /// *before* there is a circle — which is the only reason a tool can offer
    /// one at all. What a change makes has no handle until the change lands,
    /// and the session applies before the history does; a form opened after
    /// the fact would have nothing to name.
    Circle { sketch: FeatureId, center: Anchor },
    /// Grow a solid off a region, seeded at no depth at all.
    ///
    /// Nothing reaches the timeline until the form is committed — see
    /// [`Asking::Extrude`](crate::prompt::Asking) — so this names the region
    /// rather than a step, and cancelling leaves the document untouched.
    Extrude { sketch: FeatureId, region: usize },
}

/// What the application answers, being the one reader that can see all of it at
/// once.
///
/// The fourth group, and the odd one out. The other three each land on one
/// thing the application holds — the document, the history, the session — and
/// what gathers these is that no single one of them could answer. Opening a
/// file *replaces* all three at once: a different document, nothing done to it
/// yet, and nothing in hand. Framing the model needs two of them together: the
/// view knows how far the scene reaches and the document owns the camera that
/// has to take it in.
///
/// **None of these carries a payload.** An [`Intent`] is `Copy` and load-bearingly
/// so, and a [`PathBuf`](std::path::PathBuf) would end that — applying one could
/// no longer lift it out of the inbox. It would be the wrong place for one
/// anyway: which file is a question the desktop answers, and it is asked when
/// the errand lands rather than when it is raised — see [`dialog`](crate::dialog).
/// Where the answer then lives is [`Filing`](crate::filing::Filing)'s.
///
/// Landing twice is harmless, by the rule every intent here follows — though
/// none of them can, because each is gated on a keypress or a click and
/// palantir delivers those to one pass of a frame. Writing the same document to
/// the same path twice would put the same bytes there; opening the same file
/// twice would arrive at the same document; framing an already-framed model
/// frames it where it already is. None is a step, so none asks the history for
/// anything.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Errand {
    /// Start again on an empty document — three world planes and nothing drawn
    /// on any of them.
    ///
    /// Asks nothing first, unlike the two below, and asks nothing *after*
    /// either: there is no dialog because there is nothing to name. What is
    /// thrown away is whatever was open, which is the same thing
    /// [`Errand::Open`] throws away and is not guarded there either.
    New,
    /// Write the document back where it came from, or ask where to put it if it
    /// came from nowhere.
    Save,
    /// Ask where to put it, whether or not it already has somewhere.
    SaveAs,
    /// Ask which document to open.
    Open,
    /// Aim the camera to take the whole model in.
    ///
    /// Here rather than beside the camera's other moves, because it is the one
    /// that cannot name where it wants to end up: what to frame is the
    /// *scene's* extent, and a scene is the view's. So this asks for the
    /// framing and lets the application work out where that puts the camera —
    /// which is also what keeps the walk off every frame, since it runs only on
    /// the press that asks for it.
    Fit,
}

/// Everything asked for during one frame.
///
/// Cleared and refilled rather than rebuilt, so the inbox costs one allocation
/// for the life of the program instead of one a frame — which is what keeps the
/// record pass's allocation gate at zero.
#[derive(Debug, Default)]
pub(crate) struct Intents {
    queue: Vec<Intent>,
}

impl Intents {
    /// Empty it for a new frame, keeping the room it has already taken.
    pub(crate) fn clear(&mut self) {
        self.queue.clear();
    }

    /// Takes what an intent is *made of* rather than the intent, so a caller
    /// pushes `Change::Dolly { .. }` and the group it belongs to comes along
    /// with the type instead of being restated at every call site.
    pub(crate) fn push(&mut self, intent: impl Into<Intent>) {
        self.queue.push(intent.into());
    }

    /// Everything asked for, in the order it was asked for.
    ///
    /// Order is the whole of what an inbox promises: a dolly and a drag in one
    /// frame have to land the way the pointer produced them, or the drag would
    /// be resolved against a camera the wheel had already moved.
    pub(crate) fn iter(&self) -> impl Iterator<Item = Intent> {
        self.queue.iter().copied()
    }

    /// The `nth` thing asked for, or `None` past the end.
    ///
    /// The way in for a reader that has to let go of the inbox between one
    /// intent and the next. Everything else here walks with [`Intents::iter`]
    /// and writes one field of the application beside it, which the borrow
    /// checker is happy to allow; an [`Errand`] replaces most of the
    /// application at once, and cannot be holding a borrow of part of it while
    /// it lands.
    ///
    /// A walk by index rather than a list of errands gathered first, because
    /// the record pass allocates nothing and a `Vec` built to hold the two
    /// errands a session ever raises would be an allocation every frame.
    pub(crate) fn at(&self, nth: usize) -> Option<Intent> {
        self.queue.get(nth).copied()
    }
}
