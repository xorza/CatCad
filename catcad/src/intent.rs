//! What the user asked for, and the frame's inbox of it.

use aperture::Projection;
use glam::Vec3;

use crate::drawing::Grip;

/// One thing the user asked for.
///
/// Asked for rather than done. The view that raises one is handed the document
/// to read and never to write, so a gesture arrives as a request and lands in
/// one place afterwards — which is what leaves a single point every change to a
/// document passes through. An undo stack watches there; so, later, will
/// whatever decides a document has gone unsaved.
///
/// `Copy`, so applying one can lift it out of the inbox and let go of the
/// borrow before touching the document.
#[derive(Debug, Clone, Copy)]
pub(crate) enum Intent {
    /// Take what a drag has hold of to a point in the world.
    Drag { grip: Grip, to: Vec3 },
    /// Turn the camera about what it is looking at, in radians.
    Orbit { yaw: f32, pitch: f32 },
    /// Move the camera in or out by a multiple of how far off it is.
    Dolly { factor: f32 },
    /// Look through this projection.
    Project(Projection),
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
