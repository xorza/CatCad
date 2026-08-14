//! What the user asked for, and the frame's inbox of it.

use aperture::Projection;
use glam::Vec3;
use silverpoint::{Constraint, Entity};

use crate::drawing::Grip;
use crate::drawing::anchor::Anchor;
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

/// What the document answers, and the whole of what it answers.
///
/// One of exactly two ways a document changes, the other being an undo putting
/// a snapshot back. Everything here reaches
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
    Drag { grip: Grip, to: Vec3 },
    /// Put a point where this click landed, held to whatever it landed on.
    ///
    /// An [`Anchor`] rather than a place, because where a click landed is only
    /// half of what it said: on an edge or a rim it also said what the new point
    /// is to be held to, and a position alone would have thrown that away.
    AddPoint(Anchor),
    /// Put a straight edge between these two ends.
    ///
    /// One intent for the whole edge, though it is asked for by two clicks and
    /// may make two points on the way. Nothing reaches the document until the
    /// second click, so a line abandoned half-drawn leaves no stray point
    /// behind — and the one that is finished is one step to take back rather
    /// than three.
    AddSegment { from: Anchor, to: Anchor },
    /// Put a circle about `center`, out as far as `rim`.
    ///
    /// The rim says how big and nothing else: a radius is a number, so no point
    /// is made out there however the click that gave it landed.
    AddCircle { center: Anchor, rim: Anchor },
    /// State this relation over the drawing.
    ///
    /// The whole constraint rather than what was picked and which button was
    /// pressed, because working out what a selection admits is the drawing's —
    /// see [`Drawing::offers`](crate::drawing::Drawing). What arrives here is
    /// already an answer, so a replayed pass states the same relation twice
    /// rather than reading a selection that has since moved on.
    Constrain(Constraint),
    /// Take this out of the drawing, with whatever was built on it.
    ///
    /// Names what to remove rather than saying "the selection", for the same
    /// reason: a replayed pass would otherwise delete whatever is picked out by
    /// the time it ran, which after the first pass is nothing.
    Delete(Entity),
    /// Turn the camera about what it is looking at, in radians.
    Orbit { yaw: f32, pitch: f32 },
    /// Move the camera in or out by a multiple of how far off it is.
    Dolly { factor: f32 },
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
    pub(crate) fn coalesces(self) -> bool {
        matches!(self, Change::Drag { .. })
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
    /// Pick out this entity and nothing else, or nothing at all when it is
    /// `None`.
    ///
    /// The whole of what is selected rather than one addition to it, so a
    /// replayed pass lands on the same answer — see the note on naming above.
    /// A plain click raises one whatever it landed on: `None` is what a click
    /// on empty space asks for, by the same rule that a click on a point asks
    /// for that point.
    Select(Option<Entity>),
    /// Pick this out as well as whatever already is.
    ///
    /// What a shift-click asks for. Names an addition where [`Choice::Select`]
    /// names the whole, and is safe to land twice for a different reason: an
    /// entity already picked out is not picked out again.
    Include(Entity),
    /// Take up this tool, or put down whatever is in hand by naming
    /// [`Tool::Pointer`].
    ///
    /// The tool the user wants held, not the button they pressed. Pressing an
    /// armed tool's button puts it down, which is a *toggle* — so the toolbar
    /// works out what that leaves and names it, rather than asking for a flip a
    /// replayed pass would perform twice. See the note on naming above.
    Hold(Tool),
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
}
