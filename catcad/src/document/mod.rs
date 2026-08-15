//! What a saved file holds, and the one thing that owns it.

pub(crate) mod file;

use std::fs;
use std::path::Path;

use aperture::{Camera, Extent};

use crate::build::Build;
use crate::document::file::error::{LoadError, SaveError};
use crate::document::file::saved::Saved;
use crate::drawing::Drawing;
use crate::drawing::sketching::Sketching;
use crate::intent::Change;
use crate::model::Models;
use crate::profile::Profile;
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Movable, Timeline};

/// A drawing and how it is being looked at — everything a session would have to
/// write down to be opened again.
///
/// The point of gathering these is that the boundary is the file format. What
/// is in here is what saving has to write and loading has to rebuild; what is
/// not is either derived from it — the solve's report, which geometry the
/// constraints have decided, the tags the renderer picks against — or belongs
/// to this run of the program alone: the GPU buffers, and where the pointer
/// happens to be. What stands *around* the drawing without any step having put
/// it there is not in either half, and is not here — see
/// [`CatCad::scenery`](crate::CatCad).
///
/// The camera is in rather than out, though nothing about it is modelled.
/// Reopening a drawing at someone else's viewpoint is not reopening it, and a
/// document that could not say where it was being looked at from would leave
/// that to whatever raised it.
#[derive(Debug)]
pub(crate) struct Document {
    /// Every step taken to build it, which is the whole of what it says.
    timeline: Timeline,
    camera: Camera,
    /// How many times it has been changed, so a caller can tell whether what it
    /// last wrote down still describes this.
    edits: Edits,
}

impl Document {
    /// A document built by `timeline`, seen from wherever the camera starts.
    ///
    /// The camera is left at its default rather than aimed at anything: what
    /// has to fit on screen is what will be *drawn*, and that is not known
    /// until the document has been raised. Whoever raises one is who can
    /// measure it, so whoever raises one is who aims the camera.
    pub(crate) fn new(build: &mut Build, timeline: Timeline) -> Self {
        let mut document = Self {
            timeline,
            camera: Camera::default(),
            edits: Edits::default(),
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
        // After every sketch, because an extrude is grown from a region of one
        // and a sketch nothing has settled encloses nothing.
        document.remodel(build);
        document
    }

    /// Grow a solid off `profile`, as the newest step.
    ///
    /// The third way a document changes, and the odd one out: [`Document::apply`]
    /// and [`Document::restore`] both rewrite a step that is already there,
    /// where this puts one on the end. That is why it is not a [`Change`] — the
    /// history records a step by keeping what it held before and after, and a
    /// step that was not there has no before. Recording one is
    /// [`History`](crate::history::History)'s to learn, and it is the same
    /// lesson deleting and reordering will want.
    ///
    /// Nothing to solve: what the region *is* was settled by the solve that
    /// found it, and how far the solid stands off the plane is a number. What
    /// this does need is the remodel below, because the step it just added has
    /// no answer in `build` until one is worked out.
    pub(crate) fn extrude(&mut self, build: &mut Build, profile: Profile, distance: f64) {
        self.timeline.add(Feature::Extrude { profile, distance });
        build.revised();
        self.remodel(build);
        self.edits = self.edits.next();
    }

    /// Work out afresh which region each extrude is grown from.
    ///
    /// Every way a document changes ends here, which is what keeps a feature
    /// standing downstream of a sketch honest: a profile names its region by
    /// what bounds it, and any edit at all may have drawn a line across one.
    /// Replaying is what a timeline is *for* — see
    /// [`Build::remodel`](crate::build::Build::remodel), which is where the cost
    /// of replaying the lot rather than the part is argued.
    ///
    /// Reads the document and writes only `build`, like everything derived: what
    /// an extrude says is written down, and where that currently lands is not.
    fn remodel(&self, build: &mut Build) {
        build.remodel(self.timeline.extrudes());
    }

    /// Write the document to `path`, making it if it is not there and replacing
    /// whatever is if it is.
    ///
    /// The whole of it in memory before a byte reaches the disk, which is what
    /// keeps a document that fails to encode from having half-overwritten the
    /// one that was already there.
    pub(crate) fn save_to(&self, path: &Path) -> Result<(), SaveError> {
        let text = Saved::of(&self.timeline, self.camera).text()?;
        fs::write(path, text).map_err(SaveError::Write)
    }

    /// The document `path` holds, solved and ready to be looked at.
    ///
    /// An associated function rather than something done to a document, because
    /// opening one is not an edit: what it hands back is a *different*
    /// document, and the caller decides what becomes of the one it had. That is
    /// what keeps a file that will not open from having disturbed anything —
    /// see [`Filing`](crate::filing::Filing), which is what holds one side of
    /// that decision.
    ///
    /// `build` is the caller's, as it is everywhere: opening a document is a
    /// solve, and a solve wants room to work in. What it settled for the
    /// document that was open is forgotten here rather than by the caller,
    /// because the ordering is what makes a refusal harmless: every way this
    /// can fail happens before `build` is touched, so a file that will not
    /// parse or does not make sense leaves the caller holding exactly the
    /// document and the build it had, still agreeing with each other. Reset
    /// first and a refused file would have taken the *open* document's report
    /// with it — see [`Build::reopened`].
    pub(crate) fn open(build: &mut Build, path: &Path) -> Result<Self, LoadError> {
        let text = fs::read_to_string(path).map_err(LoadError::Read)?;
        let saved = Saved::parse(&text)?;
        let timeline = saved.timeline().map_err(LoadError::Fault)?;
        let camera = saved.camera();
        build.reopened();
        // Through `new`, so a document read from a file is raised exactly as
        // any other is: every sketch solved, because coordinates arrive as
        // something the constraints have not been checked against however they
        // were come by. The camera is written after, being the one thing here
        // that `new` has an opinion about.
        let mut document = Self::new(build, timeline);
        document.camera = camera;
        Ok(document)
    }

    /// The sketch at `at`, paired with the plane it lies on.
    ///
    /// Named rather than implied. A document holds several and every reader
    /// wants a particular one — which is the one open, and that is the
    /// session's to say.
    pub(crate) fn drawing_at(&self, at: FeatureId) -> Drawing<'_> {
        self.timeline.drawing(at)
    }

    /// The plane at `at` as something that can be moved, or `None` where it is
    /// the ground.
    ///
    /// What a press on a datum asks before it decides it has hold of one. Here
    /// rather than through [`Models`], though the drawn datums are found that
    /// way: a press has the document and not the build, and what it needs to
    /// know — where this plane may travel — is the timeline's alone.
    pub(crate) fn movable(&self, at: FeatureId) -> Option<Movable> {
        self.timeline.movable(at)
    }

    /// The sketch a session should start in.
    ///
    /// The first one the timeline holds, which is where a document that has
    /// just been raised puts you. Which sketch is open after that is the
    /// session's — see [`Session::editing`](crate::session::Session) — because
    /// nothing about what you have open is written down by saving.
    pub(crate) fn opening(&self) -> FeatureId {
        self.timeline.first_sketch()
    }

    /// The sketch at `at`, open for editing.
    fn sketching(&mut self, at: FeatureId) -> Sketching<'_> {
        self.timeline.edit(at)
    }

    /// What the step at `at` holds, for a history taking it down.
    pub(crate) fn feature(&self, at: FeatureId) -> &Feature {
        self.timeline.feature(at)
    }

    /// The same, written over `into` rather than handed back — so a history
    /// rewriting one step's far end every frame refills what it already has.
    pub(crate) fn feature_into(&self, at: FeatureId, into: &mut Feature) {
        into.clone_from(self.timeline.feature(at));
    }

    /// The drawing as `build` last left it.
    ///
    /// The pairing everything that reads the model reads it through — see
    /// [`Model`]. Here because a caller holding a document and a build is
    /// holding both halves already, and naming the type to put them together
    /// would be ceremony.
    /// Every sketch it holds as `build` last left them, with `editing` the one
    /// being worked in.
    pub(crate) fn models<'a>(&'a self, build: &'a Build, editing: FeatureId) -> Models<'a> {
        Models::new(&self.timeline, build, editing)
    }

    /// Where the document is being looked at from.
    pub(crate) fn camera(&self) -> Camera {
        self.camera
    }

    /// How many times this has been changed.
    ///
    /// What [`Filing`](crate::filing::Filing) stamps at a save and compares
    /// against afterwards, which is the whole of how a document knows it has
    /// gone unsaved.
    ///
    /// Its own count rather than [`Build::revision`](crate::build::Build),
    /// which is the nearest thing already here and cannot serve: turning the
    /// camera changes the document and settles nothing, so the revision does
    /// not move for it — and making it move would relayout the whole drawing on
    /// every frame of an orbit.
    pub(crate) fn edits(&self) -> Edits {
        self.edits
    }

    /// Aim the camera to take `extent` in.
    ///
    /// Named rather than handing out the camera, for the same reason everything
    /// below is: a document that lent out `&mut Camera` would be a document
    /// whose every change no longer passed a place that could be watched. This
    /// is the one aiming nobody asked for — what a document does on being opened,
    /// before anyone has looked at it.
    pub(crate) fn frame(&mut self, extent: Extent) {
        self.camera.frame(extent);
        self.edits = self.edits.next();
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
    pub(crate) fn restore(&mut self, build: &mut Build, at: FeatureId, was: &Feature) {
        self.timeline.put_back(at, was);
        // Restored rather than re-solved, and then measured. Solving from the
        // restored geometry would derive the report through the one path that
        // already produces one, but a solve is free to *move* what it is given
        // — and an undo that landed the drawing near where it was rather than
        // on it would not be an undo. A plane has nothing to measure: putting
        // its number back is the whole of it.
        match was {
            Feature::Sketch { .. } => self.sketching(at).measured(build),
            // Neither has anything to solve. A plane's number and an extrude's
            // profile and distance are what they are; putting them back is the
            // whole of it, and where the drawing then *lands* is worked out by
            // whoever reads it.
            Feature::Plane(_) | Feature::Extrude { .. } => build.revised(),
        }
        self.remodel(build);
        self.edits = self.edits.next();
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
            // Nothing to solve. The sketches on this plane are in its own
            // coordinates and say exactly what they said before; all that has
            // changed is where they land, which nothing keeps a copy of.
            Change::MovePlane { plane, to } => {
                self.timeline.offset(plane, to);
                build.revised();
            }
            Change::Orbit { yaw, pitch } => self.camera.orbit(yaw, pitch),
            Change::Dolly { factor } => self.camera.dolly(factor),
            Change::Pan { by } => self.camera.pan(by),
            Change::Project(projection) => self.camera.projection = projection,
        }
        // After the change rather than as part of one, because it is a fact
        // about every step and not about the one that moved — a line drawn in
        // one sketch can take away what an extrude two steps later was grown
        // from. Turning the camera passes through here too, and costs a walk of
        // however many extrudes the document holds.
        self.remodel(build);
        self.edits = self.edits.next();
    }
}

/// How many times a document has been changed.
///
/// Compared and never read, like [`Revision`](crate::build::Revision): the
/// number means nothing beyond not being the one before it.
///
/// Conservative in the same direction, and for the same reason. A drag the
/// constraints refuse counts, and so does an undo that lands back on what was
/// last saved — what can cheaply be said is that the document has been worked
/// on, not whether the work came to anything. A stamp that missed a change
/// would lose it silently at the next quit; a spare one costs an asterisk in
/// the corner.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct Edits(u64);

impl Edits {
    /// The one after this.
    fn next(self) -> Self {
        Self(self.0 + 1)
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
    #[cfg(test)]
    use crate::document::Edits;
    use aperture::Camera;

    /// Narrower than the mod around it, which the visual suite reaches through
    /// as well: this is wanted by one unit test and nothing a harness links.
    #[cfg(test)]
    impl Edits {
        /// One more edit than this.
        ///
        /// A document is what counts its own edits, so nothing outside can
        /// advance one honestly — and a test about what a *stamp* means has no
        /// document to hand and wants no solve to get a second value out of.
        pub(crate) fn stepped(self) -> Self {
            self.next()
        }
    }

    impl Document {
        pub(crate) fn camera_mut(&mut self) -> &mut Camera {
            &mut self.camera
        }
    }
}
