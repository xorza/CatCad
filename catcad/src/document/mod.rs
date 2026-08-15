//! What a saved file would hold, and the one thing that owns it.

use aperture::{Bounds, Camera, Object};

use crate::build::Build;
use crate::drawing::Drawing;
use crate::drawing::sketching::Sketching;
use crate::intent::Change;
use crate::model::Model;
use crate::timeline::{FeatureId, Timeline};
use silverpoint::Snapshot;

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
    /// Every step taken to build it, which is the whole of what it says.
    timeline: Timeline,
    /// The solids the drawing is modelled alongside.
    ///
    /// A stand-in, and the one thing here that no step made. Solids become what
    /// an extrude *makes*, and this field goes when one exists — until then the
    /// demo needs ground to stand on, and a document with no way to say where
    /// that came from is the honest state of things.
    solids: Vec<Object>,
    camera: Camera,
}

impl Document {
    /// A document built by `timeline`, with `solids` standing around it, seen
    /// from wherever the camera starts.
    ///
    /// The camera is left at its default rather than aimed at anything: what
    /// has to fit on screen is what will be *drawn*, and that is not known
    /// until the document has been raised. Whoever raises one is who can
    /// measure it, so whoever raises one is who aims the camera.
    pub(crate) fn new(build: &mut Build, timeline: Timeline, solids: Vec<Object>) -> Self {
        let mut document = Self {
            timeline,
            solids,
            camera: Camera::default(),
        };
        // Every sketch, not only the one a session starts in. A sketch arrives
        // as coordinates its constraints have not been checked against, whether
        // they were typed in or read from a file, so opening a document is a
        // solve like any other — and one left unsolved would have no report and
        // no faces for anything reading it to find.
        //
        // Gathered before any of them is opened, because opening one borrows
        // the timeline the rest are still to be found in. One list, once, when
        // a document is raised.
        let sketches: Vec<_> = document.timeline.sketches().collect();
        for at in sketches {
            document.sketching(at).opened(build);
        }
        document
    }

    /// The sketch being edited, paired with the plane it lies on.
    ///
    /// The one sketch, while there is only one — see
    /// [`Timeline::only_sketch`](crate::timeline::Timeline::only_sketch).
    pub(crate) fn drawing(&self) -> Drawing<'_> {
        self.timeline.drawing(self.timeline.only_sketch())
    }

    /// The sketch a session should start in.
    ///
    /// The first one the timeline holds, which is where a document that has
    /// just been raised puts you. Which sketch is open after that is the
    /// session's — see [`Session::editing`](crate::session::Session) — because
    /// nothing about what you have open is written down by saving.
    pub(crate) fn opening(&self) -> FeatureId {
        self.timeline.only_sketch()
    }

    /// The sketch at `at`, open for editing.
    fn sketching(&mut self, at: FeatureId) -> Sketching<'_> {
        self.timeline.edit(at)
    }

    /// Take down where the sketch at `at` stands, so it can be put back later.
    pub(crate) fn snapshot_of(&self, at: FeatureId, into: &mut Snapshot) {
        self.timeline.drawing(at).snapshot_into(into);
    }

    /// The drawing as `build` last left it.
    ///
    /// The pairing everything that reads the model reads it through — see
    /// [`Model`]. Here because a caller holding a document and a build is
    /// holding both halves already, and naming the type to put them together
    /// would be ceremony.
    pub(crate) fn model<'a>(&'a self, build: &'a Build) -> Model<'a> {
        let at = self.timeline.only_sketch();
        Model::new(self.timeline.drawing(at), build, at)
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
    pub(crate) fn restore(&mut self, build: &mut Build, at: FeatureId, snapshot: &Snapshot) {
        self.sketching(at).restore(build, snapshot);
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
    /// `build` is the caller's. Solving is what an edit to a drawing *is*,
    /// and a solve wants room to work in — and leaves a report behind — that is
    /// worth keeping across a drag and worth nothing in a saved file. So both
    /// belong to whoever is doing the editing, and the document borrows them
    /// for the length of the call. An edit that could happen without one in
    /// hand would be an edit that left its report stale.
    pub(crate) fn apply(&mut self, build: &mut Build, change: Change) {
        match change {
            Change::Drag { sketch, grip, to } => self.sketching(sketch).drag_to(build, grip, to),
            Change::AddPoint { sketch, at } => self.sketching(sketch).add_point(build, at),
            Change::AddSegment { sketch, from, to } => {
                self.sketching(sketch).add_segment(build, from, to)
            }
            Change::AddCircle {
                sketch,
                center,
                rim,
            } => self.sketching(sketch).add_circle(build, center, rim),
            Change::Constrain { sketch, constraint } => {
                self.sketching(sketch).constrain(build, constraint)
            }
            Change::Resize {
                sketch,
                constraint,
                to,
            } => self.sketching(sketch).resize(build, constraint, to),
            Change::Delete { sketch, entity } => self.sketching(sketch).remove(build, entity),
            Change::Tidy { sketch } => self.sketching(sketch).remove_duplicates(build),
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

/// What a unit test reaches past the document for.
///
/// Its own mod beside [`internals`] rather than an item within it, because the
/// two are gated differently: the visual suite aims a camera by hand and so
/// wants a feature it can turn on, where nothing outside this crate has any
/// business naming a sketch nobody has open.
#[cfg(test)]
mod unopened {
    use crate::document::Document;
    use crate::drawing::Drawing;
    use crate::timeline::FeatureId;

    impl Document {
        /// The sketch at `at`, paired with the plane it lies on.
        ///
        /// Reaching past the one that is *open* is standing outside a session:
        /// the application draws and edits the sketch it has open, and a caller
        /// naming another is a test asking after one nobody is in.
        pub(crate) fn drawing_of(&self, at: FeatureId) -> Drawing<'_> {
            self.timeline.drawing(at)
        }
    }
}
