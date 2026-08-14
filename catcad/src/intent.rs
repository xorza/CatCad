//! What the user asked for, and the frame's inbox of it.

use aperture::Projection;
use glam::Vec3;

use crate::drawing::Grip;
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
/// `Copy`, so applying one can lift it out of the inbox and let go of the
/// borrow before touching what it lands on.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Intent {
    /// Take what a drag has hold of to a point in the world.
    ///
    /// Names where the entity should end up rather than how far to move it,
    /// which is what lets a settling frame apply the same drag twice and land
    /// in the same place. See the note on clearing the inbox in `CatCad::record`.
    Drag {
        grip: Grip,
        to: Vec3,
    },
    /// Put a free point on the drawing's plane, under this point in the world.
    ///
    /// Where in the world rather than where on the plane, like the drag above
    /// and for the same reason: what the pointer resolves is a ray against a
    /// motion, and where that lands on the *sketch* is the drawing's to say.
    AddPoint {
        at: Vec3,
    },
    /// The drag let go.
    ///
    /// Changes nothing by itself — it closes the step the drag has been
    /// extending, so a gesture is one thing to take back rather than one per
    /// frame it lasted.
    Release,
    /// Turn the camera about what it is looking at, in radians.
    Orbit {
        yaw: f32,
        pitch: f32,
    },
    /// Move the camera in or out by a multiple of how far off it is.
    Dolly {
        factor: f32,
    },
    /// Look through this projection.
    Project(Projection),
    /// Take up this tool, or put down whatever is in hand by naming
    /// [`Tool::Select`].
    ///
    /// The tool the user wants held, not the button they pressed. Pressing an
    /// armed tool's button puts it down, which is a *toggle* — so the toolbar
    /// works out what that leaves and names it, rather than asking for a flip a
    /// replayed pass would perform twice. See the note on naming above.
    Hold(Tool),
    /// Take back the last step, and put it back.
    Undo,
    Redo,
}

impl Intent {
    /// Whether this belongs to a gesture already under way, so a history
    /// extends the step it is recording rather than starting another.
    ///
    /// A drag is the whole of it. It arrives a frame at a time and is one thing
    /// the user did, so sixty of them are one step back — where a point put
    /// down, or anything else that happens once, stands alone.
    pub(crate) fn coalesces(self) -> bool {
        matches!(self, Intent::Drag { .. })
    }
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

    pub(crate) fn push(&mut self, intent: Intent) {
        self.queue.push(intent);
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
