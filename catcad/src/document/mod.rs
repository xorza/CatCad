//! What a saved file would hold, and the one thing that owns it.

use aperture::{Bounds, Camera, Object};

use crate::drawing::Drawing;
use crate::intent::Change;
use crate::model::Model;
use crate::workshop::Workshop;
use silverpoint::Plane;
use silverpoint::{Sketch, Snapshot};

/// A drawing, the solids modelled beside it, and how it is being looked at —
/// everything a session would have to write down to be opened again.
///
/// The point of gathering these is that the boundary is the file format. What
/// is in here is what saving has to write and loading has to rebuild; what is
/// not is either derived from it — the solve's report, which geometry the
/// constraints have decided, the tags the renderer picks against — or belongs
/// to this run of the program alone: the GPU buffers, and where the pointer
/// happens to be.
///
/// The camera is in rather than out, though nothing about it is modelled.
/// Reopening a drawing at someone else's viewpoint is not reopening it, and a
/// document that could not say where it was being looked at from would leave
/// that to whatever raised it.
#[derive(Debug)]
pub(crate) struct Document {
    drawing: Drawing,
    /// The solids the drawing is modelled alongside. Read by whatever lays the
    /// document out and kept here as the record of them, which is the
    /// difference between what the document *is* and what is being drawn.
    solids: Vec<Object>,
    camera: Camera,
}

impl Document {
    /// A document holding `sketch` on `plane`, with `solids` standing around
    /// it, seen from wherever the camera starts.
    ///
    /// The camera is left at its default rather than aimed at anything: what
    /// has to fit on screen is what will be *drawn*, and that is not known
    /// until the document has been raised. Whoever raises one is who can
    /// measure it, so whoever raises one is who aims the camera.
    pub(crate) fn new(
        workshop: &mut Workshop,
        sketch: Sketch,
        plane: Plane,
        solids: Vec<Object>,
    ) -> Self {
        Self {
            drawing: Drawing::new(workshop, sketch, plane),
            solids,
            camera: Camera::default(),
        }
    }

    /// The model, which is the whole of what the document says.
    pub(crate) fn drawing(&self) -> &Drawing {
        &self.drawing
    }

    /// The drawing as `workshop` last left it.
    ///
    /// The pairing everything that reads the model reads it through — see
    /// [`Model`]. Here because a caller holding a document and a workshop is
    /// holding both halves already, and naming the type to put them together
    /// would be ceremony.
    pub(crate) fn model<'a>(&'a self, workshop: &'a Workshop) -> Model<'a> {
        Model::new(&self.drawing, workshop)
    }

    /// The solids modelled alongside the drawing.
    pub(crate) fn solids(&self) -> &[Object] {
        &self.solids
    }

    /// Where the document is being looked at from.
    pub(crate) fn camera(&self) -> Camera {
        self.camera
    }

    /// Aim the camera to take `bounds` in.
    ///
    /// Named rather than handing out the camera, for the same reason everything
    /// below is: a document that lent out `&mut Camera` would be a document
    /// whose every change no longer passed a place that could be watched. This
    /// is the one aiming nobody asked for — what a document does on being opened,
    /// before anyone has looked at it.
    pub(crate) fn frame(&mut self, bounds: Bounds) {
        self.camera.frame(bounds);
    }

    /// Put the drawing back the way `snapshot` found it.
    ///
    /// The history's, and only the history's. It sits beside [`Document::apply`]
    /// rather than inside it because a snapshot is not an intent: undoing is a
    /// question about what has been *done*, which the inbox has no vocabulary
    /// for — see the refusal at the end of `apply`.
    ///
    /// Named here rather than reached through a borrowed drawing, so that the
    /// two ways a document changes are two calls on the document. An undo is the
    /// path that most wants watching — it is the one that can make geometry stop
    /// existing — and it would have been the one going round the back.
    pub(crate) fn restore(&mut self, workshop: &mut Workshop, snapshot: &Snapshot) {
        self.drawing.restore(workshop, snapshot);
    }

    /// Land what `change` asks for.
    ///
    /// The one place an intent becomes a change, which is the point of there
    /// being intents at all: every edit a *gesture* asks for passes through
    /// here, so there is one place to watch rather than one per gesture. What
    /// watches is [`History`](crate::history::History), which is also what
    /// drives this — it takes each of a frame's intents in turn and notes what
    /// this did.
    ///
    /// Takes a [`Change`] rather than an [`Intent`](crate::intent::Intent), so
    /// the match below is exhaustive over exactly what a document can answer.
    /// What is in hand, what is picked out and where in the history the document
    /// stands are each somebody else's, and none of them can be handed here to
    /// be refused at runtime — the type refuses them.
    ///
    /// One of exactly two ways a document changes, the other being
    /// [`Document::restore`], and the pair is the whole of it: what someone
    /// asked for, and what the history puts back. Everything else a document
    /// hands out is `&self`.
    ///
    /// `workshop` is the caller's. Solving is what an edit to a drawing *is*,
    /// and a solve wants room to work in — and leaves a report behind — that is
    /// worth keeping across a drag and worth nothing in a saved file. So both
    /// belong to whoever is doing the editing, and the document borrows them
    /// for the length of the call. An edit that could happen without one in
    /// hand would be an edit that left its report stale.
    pub(crate) fn apply(&mut self, workshop: &mut Workshop, change: Change) {
        match change {
            Change::Drag { grip, to } => self.drawing.drag_to(workshop, grip, to),
            Change::AddPoint(at) => self.drawing.add_point(workshop, at),
            Change::AddSegment { from, to } => self.drawing.add_segment(workshop, from, to),
            Change::AddCircle { center, rim } => self.drawing.add_circle(workshop, center, rim),
            Change::Constrain(constraint) => self.drawing.constrain(workshop, constraint),
            Change::Resize { constraint, to } => self.drawing.resize(workshop, constraint, to),
            Change::Delete(entity) => self.drawing.remove(workshop, entity),
            Change::Tidy => self.drawing.remove_duplicates(workshop),
            Change::Orbit { yaw, pitch } => self.camera.orbit(yaw, pitch),
            Change::Dolly { factor } => self.camera.dolly(factor),
            Change::Pan { by } => self.camera.pan(by),
            Change::Project(projection) => self.camera.projection = projection,
        }
    }
}

/// What a harness reaches past the document for.
///
/// Turning the camera by hand is standing outside a frame: the application only
/// ever moves it through an intent, so a caller wanting the camera itself is a
/// test or a bench aiming one without a pointer to aim it with.
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::document::Document;
    use aperture::Camera;

    impl Document {
        pub(crate) fn camera_mut(&mut self) -> &mut Camera {
            &mut self.camera
        }
    }
}
