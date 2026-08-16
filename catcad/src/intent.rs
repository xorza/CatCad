//! What the user asked for, and the frame's inbox of it.

use aperture::Projection;
use glam::Vec3;
use silverpoint::{Constraint, ConstraintId, Entity};

use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
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

/// What the document answers, and the whole of what it answers.
///
/// Everything a document can be *asked* for, which is not quite everything that
/// changes one: an undo puts a snapshot back without passing through here, and
/// adding a step — an extrude, so far — has no before for the history to record
/// and so is not a change anyone can ask for yet. Everything here reaches
/// [`Document::apply`](crate::document::Document), which matches it exhaustively
/// — so a new one added to this enum is a compile error until the document says
/// what to do with it, where a new one added beside it in [`Intent`] cannot
/// reach the document at all.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Change {
    /// Take what a drag has hold of to a point in the world.
    ///
    /// Names where the entity should end up rather than how far to move it,
    /// which is what lets a settling frame apply the same drag twice and land
    /// in the same place. See the note on clearing the inbox in `CatCad::record`.
    Drag {
        sketch: FeatureId,
        grip: Grip,
        to: Vec3,
    },
    /// Put a point where this click landed, held to whatever it landed on.
    ///
    /// An [`Anchor`] rather than a place, because where a click landed is only
    /// half of what it said: on an edge or a rim it also said what the new point
    /// is to be held to, and a position alone would have thrown that away.
    AddPoint { sketch: FeatureId, at: Anchor },
    /// Put a straight edge between these two ends.
    ///
    /// One intent for the whole edge, though it is asked for by two clicks and
    /// may make two points on the way. Nothing reaches the document until the
    /// second click, so a line abandoned half-drawn leaves no stray point
    /// behind — and the one that is finished is one step to take back rather
    /// than three.
    AddSegment {
        sketch: FeatureId,
        from: Anchor,
        to: Anchor,
    },
    /// Put a circle about `center`, out as far as `rim`.
    ///
    /// The rim says how big and nothing else: a radius is a number, so no point
    /// is made out there however the click that gave it landed.
    AddCircle {
        sketch: FeatureId,
        center: Anchor,
        rim: Anchor,
    },
    /// State this relation over the drawing.
    ///
    /// The whole constraint rather than what was picked and which button was
    /// pressed, because working out what a selection admits is the drawing's —
    /// see [`Drawing::offers`](crate::drawing::Drawing). What arrives here is
    /// already an answer, so a replayed pass states the same relation twice
    /// rather than reading a selection that has since moved on.
    Constrain {
        sketch: FeatureId,
        constraint: Constraint,
    },
    /// Restate a dimension at a new magnitude, and let the drawing settle onto
    /// it.
    ///
    /// Names the value it wants rather than a step to take, like everything
    /// here: a scrub sends one of these a frame and a replayed pass restates the
    /// same number, where "a bit larger" would grow twice over.
    Resize {
        sketch: FeatureId,
        constraint: ConstraintId,
        to: f64,
    },
    /// Take out geometry that duplicates other geometry and carries nothing.
    ///
    /// Names no geometry, unlike [`Change::Delete`] beside it, and needs to
    /// name none: what qualifies is a property of the drawing rather than of
    /// what is picked out, so a replayed pass asks the same question of the
    /// same sketch. The second pass finds nothing left to remove, which is the
    /// same answer arrived at twice rather than a second removal.
    Tidy { sketch: FeatureId },
    /// Take this out of the drawing, with whatever was built on it.
    ///
    /// Names what to remove rather than saying "the selection", for the same
    /// reason: a replayed pass would otherwise delete whatever is picked out by
    /// the time it ran, which after the first pass is nothing.
    Delete { sketch: FeatureId, entity: Entity },
    /// Grow a solid off a region of a sketch.
    ///
    /// The one change that *adds* a step rather than rewriting one, which is
    /// what [`Change::creates`] answers and what the history has to be told
    /// before it can record anything: a step that was not there has no before.
    ///
    /// The region by position rather than as a
    /// [`Profile`](crate::profile::Profile), and that is forced: an intent is
    /// [`Copy`] and load-bearingly so, where a profile owns the list of curves
    /// naming it. Minting the durable name is the document's, one line later —
    /// see [`Document::apply`](crate::document::Document). Nothing is lost by
    /// the wait, because a position holds for as long as the arrangement it was
    /// read from does, and that is the frame this intent lands in.
    Extrude {
        sketch: FeatureId,
        region: usize,
        distance: f64,
    },
    /// Take a plane to a new offset from the one it is measured off.
    ///
    /// Names the offset it wants rather than a step to take, like everything
    /// here: a drag sends one of these a frame and a replayed pass restates the
    /// same number, where "a little further" would travel twice over.
    ///
    /// The one change that edits no sketch. What it moves is where the sketches
    /// hanging off that plane *land*, and none of them says anything different
    /// for it — see [`Datum::Offset`](crate::timeline::feature::Datum).
    MovePlane { plane: FeatureId, to: f64 },
    /// Carry a solid to a new distance off the plane its region was drawn on.
    ///
    /// Names the distance it wants rather than a step to take, like
    /// [`Change::MovePlane`] above and for the same reason: a drag sends one of
    /// these a frame and a replayed pass restates the same number, where "a
    /// little deeper" would grow twice over.
    ///
    /// Signed, so this is also how a solid is flipped to the other side of its
    /// plane — see [`Feature::Extrude`](crate::timeline::feature::Feature).
    Carry { extrude: FeatureId, to: f64 },
    /// Turn the camera about what it is looking at, in radians.
    Orbit { yaw: f32, pitch: f32 },
    /// Move the camera in or out by a multiple of how far off it is.
    Dolly { factor: f32 },
    /// Slide the camera across the view, without turning it or changing how
    /// far off it is.
    ///
    /// A world step rather than the pixels the gesture was made of, like
    /// [`Change::Drag`] and for the same reason: what a pixel is worth depends
    /// on the viewport, and the viewport is the view's. The document has no
    /// screen and is owed no chance to guess at one.
    Pan { by: Vec3 },
    /// Look through this projection.
    Project(Projection),
}

impl Change {
    /// Whether this belongs to a gesture already under way, so a history
    /// extends the step it is recording rather than starting another.
    ///
    /// A drag is the whole of it. It arrives a frame at a time and is one thing
    /// the user did, so sixty of them are one step back — where a point put
    /// down, or anything else that happens once, stands alone.
    ///
    /// Scrubbing a dimension is the same shape by the same argument: the
    /// pointer travels, the number follows it, and what the user did was set a
    /// value once. Both are closed by a [`Step::Release`], which the widget
    /// driving them raises when its gesture ends.
    pub(crate) fn coalesces(self) -> bool {
        matches!(
            self,
            Change::Drag { .. }
                | Change::Resize { .. }
                | Change::MovePlane { .. }
                | Change::Carry { .. }
        )
    }

    /// Whether this adds a step to the timeline rather than rewriting one that
    /// is already there.
    ///
    /// The first thing the history asks, and the reason it has to ask: a step
    /// that was not there has no *before*, so undoing one puts back its absence
    /// rather than a value. See [`Edit`](crate::history::Edit).
    ///
    /// An exhaustive match rather than a `matches!`, unlike [`Change::coalesces`]
    /// beside it, because the two get a wrong answer differently. A gesture
    /// mistaken for a single edit costs an extra press of undo; a creation
    /// mistaken for a rewrite is a step nothing can take back.
    pub(crate) fn creates(self) -> bool {
        match self {
            Change::Extrude { .. } => true,
            Change::Drag { .. }
            | Change::AddPoint { .. }
            | Change::AddSegment { .. }
            | Change::AddCircle { .. }
            | Change::Constrain { .. }
            | Change::Resize { .. }
            | Change::Tidy { .. }
            | Change::Delete { .. }
            | Change::MovePlane { .. }
            | Change::Carry { .. }
            | Change::Orbit { .. }
            | Change::Dolly { .. }
            | Change::Pan { .. }
            | Change::Project(_) => false,
        }
    }

    /// Which step of the timeline this is about, or `None` where it is about no
    /// step that is already there — the camera, or a change that makes one.
    ///
    /// What the history records a step against — see
    /// [`History::edit`](crate::history::History). The camera is not the
    /// document, so turning it names no step and there is nothing to take back.
    ///
    /// A match rather than a comparison of before and after, which is what this
    /// used to be. Naming the step is what a document of several needs of every
    /// edit, and once an edit names one there is nothing to compare a camera
    /// move *against*. What the exhaustive match buys back is that a variant
    /// added later cannot quietly land on either side: the compiler asks which
    /// it is.
    pub(crate) fn feature(self) -> Option<FeatureId> {
        match self {
            Change::Drag { sketch, .. }
            | Change::AddPoint { sketch, .. }
            | Change::AddSegment { sketch, .. }
            | Change::AddCircle { sketch, .. }
            | Change::Constrain { sketch, .. }
            | Change::Resize { sketch, .. }
            | Change::Tidy { sketch }
            | Change::Delete { sketch, .. } => Some(sketch),
            Change::MovePlane { plane, .. } => Some(plane),
            Change::Carry { extrude, .. } => Some(extrude),
            // The sketch it names is what the region is *of* rather than what
            // this is about: the step it is about does not exist until it
            // lands, and what the history records for one is
            // [`Change::creates`]'s business.
            Change::Extrude { .. }
            | Change::Orbit { .. }
            | Change::Dolly { .. }
            | Change::Pan { .. }
            | Change::Project(_) => None,
        }
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
    /// Grow a solid off a region, seeded at no depth at all.
    ///
    /// Nothing reaches the timeline until the form is committed — see
    /// [`Asking::Extrude`](crate::prompt::Asking) — so this names the region
    /// rather than a step, and cancelling leaves the document untouched.
    Extrude { sketch: FeatureId, region: usize },
}

/// What the application answers: putting the document away, and fetching one
/// back.
///
/// The fourth group, and the odd one out. The other three each land on
/// something the application holds — the document, the history, the session —
/// and this one lands on the application itself, because opening a file
/// replaces all three at once: a different document, nothing done to it yet, and
/// nothing in hand. Nothing below the application could answer that, which is
/// why it is not a [`Change`].
///
/// **None of these carries a path.** An [`Intent`] is `Copy` and load-bearingly
/// so, and a [`PathBuf`](std::path::PathBuf) would end that — applying one could
/// no longer lift it out of the inbox. It would be the wrong place for one
/// anyway: which file is a question the desktop answers, and it is asked when
/// the errand lands rather than when it is raised — see [`dialog`](crate::dialog).
/// Where the answer then lives is [`Filing`](crate::filing::Filing)'s.
///
/// Landing twice is harmless, by the rule every intent here follows — though
/// none of them can, because all three are gated on a keypress or a click and
/// palantir delivers those to one pass of a frame. Writing the same document to
/// the same path twice would put the same bytes there; opening the same file
/// twice would arrive at the same document. Neither is a step, so neither asks
/// the history for anything.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Errand {
    /// Write the document back where it came from, or ask where to put it if it
    /// came from nowhere.
    Save,
    /// Ask where to put it, whether or not it already has somewhere.
    SaveAs,
    /// Ask which document to open.
    Open,
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
