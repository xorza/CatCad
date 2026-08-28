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
use crate::intent::change::{About, Change};
use crate::model::Models;
use crate::timeline::feature::Feature;
use crate::timeline::{FeatureId, Movable, Timeline, Uprooted};
use silverpoint::Sketch;

/// A drawing and how it is being looked at — everything a session would have to
/// write down to be opened again.
///
/// The point of gathering these is that the boundary is the file format. What
/// is in here is what saving has to write and loading has to rebuild; what is
/// not is either derived from it — the solve's report, which geometry the
/// constraints have decided, the tags the renderer picks against — or belongs
/// to this run of the program alone: the GPU buffers, and where the pointer
/// happens to be. Nothing at all stands outside the two: everything drawn was
/// put there by a step, solids included, which is the whole of why writing this
/// down is enough to open the picture again.
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
        document.rebuilt(build);
        document
    }

    /// A document holding the three world planes and nothing else.
    ///
    /// **Where every document starts**, and the state the whole of this item is
    /// for: three planes, nothing drawn on any of them, and a way to start
    /// drawing on one. A document with no planes could be asked for as easily
    /// and would be a document nothing could ever be added to — see
    /// [`Timeline::started`].
    ///
    /// Solved on the way through [`Document::new`], which has nothing to solve
    /// here. Kept as one call anyway, so raising a document is one path however
    /// much is in it.
    pub(crate) fn empty(build: &mut Build) -> Self {
        Self::new(build, Timeline::started())
    }

    /// Put `feature` back on the end under the name it already had, which is the
    /// redo of a creation.
    ///
    /// **On the end, and that is right rather than merely what it used to do.**
    /// A creation puts a step last, and nothing can have moved it since: any
    /// edit at all throws away what an undo left to redo — see
    /// [`History::record`](crate::history::History) — so between taking this
    /// step off and putting it back the recipe cannot have changed at all.
    ///
    /// One step through the call that puts back several, because a creation is a
    /// cascade of one and nothing about it differs. Undoing one goes through
    /// [`Document::uproot_all`] the same way — which is what makes a sketch that
    /// was made and taken back leave nothing behind it, where a second path did
    /// not forget what the build had settled for it.
    pub(crate) fn put_again(&mut self, build: &mut Build, at: FeatureId, feature: Feature) {
        self.replant_all(
            build,
            &[Uprooted {
                at,
                position: self.timeline.count(),
                feature,
            }],
        );
    }

    /// Take each of `steps` out, and forget whatever was solved for them.
    ///
    /// **Newest first**, which is what makes each removal legal in its turn:
    /// nothing may be taken out from under something built on it — see
    /// [`Timeline::uproot`] — and walking back down the list means everything
    /// standing on a step has already gone by the time the step does.
    ///
    /// Handed back in the order they *sat*, not the order they came out, because
    /// that is the order they go back in. Every position is the one it had in
    /// the whole recipe: taking the last one first means each removal only ever
    /// shifts steps that have already been taken.
    ///
    /// **What the build knew of them goes with them.** A `Settled` is made on
    /// first use and never removed otherwise, so every step taken out would
    /// otherwise leave a report about a sketch nothing can reach — see
    /// [`Build::forgot`].
    ///
    /// No rebuild, so the two callers can each say what follows —
    /// [`Document::apply`] lets its own tail do it, and
    /// [`Document::uproot_all`] does it for itself.
    fn pulled(&mut self, build: &mut Build, steps: &[FeatureId]) -> Vec<Uprooted> {
        let mut pulled: Vec<Uprooted> = steps
            .iter()
            .rev()
            .map(|&at| self.timeline.uproot(at))
            .collect();
        pulled.reverse();
        build.forgot(steps);
        pulled
    }

    /// Move the step at `at` to `to`, which is both directions of an undo — one
    /// way puts it back where it was, the other where it went.
    ///
    /// Clamped by whoever asked, like the change itself: what reaches here is a
    /// position the step may take, and putting one back where it came from is
    /// always one of those.
    pub(crate) fn shift_to(&mut self, build: &mut Build, at: FeatureId, to: usize) {
        self.timeline.shift(at, to);
        build.revised();
        self.rebuilt(build);
        self.edits = self.edits.next();
    }

    /// Where the step at `at` would land if it were moved `by` places, or `None`
    /// where it cannot go that way.
    ///
    /// **Refused rather than clamped at the ends**, which is the difference
    /// between a key that does nothing and a key that records a step that did
    /// nothing: an `Edit` for a move that moved nowhere is an undo the user
    /// presses and watches do nothing.
    ///
    /// Beside [`Document::movable`] and for the reason that one is here: a press
    /// has the document and not the build, and where a step may go is the
    /// timeline's alone.
    pub(crate) fn nudged(&self, at: FeatureId, by: isize) -> Option<usize> {
        let to = self.timeline.position_of(at).checked_add_signed(by)?;
        self.timeline.moves_within(at).contains(&to).then_some(to)
    }

    /// Take `steps` out again, which is the redo of a delete.
    ///
    /// The same cascade the delete worked out rather than a fresh one, and it is
    /// the same either way: nothing can have touched the document between an
    /// undo and its redo, because anything at all throws away what there was to
    /// redo. Replaying the list is the shorter of the two ways to say it.
    pub(crate) fn uproot_all(&mut self, build: &mut Build, steps: &[FeatureId]) {
        self.pulled(build, steps);
        build.revised();
        self.rebuilt(build);
        self.edits = self.edits.next();
    }

    /// Put `steps` back, each where it sat, which is the undo of a delete.
    ///
    /// **Oldest first**, the mirror of the order they came out in: each lands in
    /// a recipe the ones before it have already been put back into, so the
    /// position each recorded is the position it takes.
    ///
    /// Measured rather than solved, the same reading [`Document::restore`] takes:
    /// what comes back is the geometry that went away, so there is nothing to
    /// fix and a solve would be free to wander off it. Every sketch among them
    /// needs it, because [`Document::pulled`] forgot them when they went.
    ///
    /// In a pass of its own rather than as each step lands. One pass would do —
    /// a sketch settles against its plane, and a plane is always earlier, so it
    /// is always already back — but splitting them means the settle never has to
    /// know that.
    pub(crate) fn replant_all(&mut self, build: &mut Build, steps: &[Uprooted]) {
        for uprooted in steps {
            self.timeline.replant(uprooted.clone());
        }
        for uprooted in steps {
            if matches!(uprooted.feature, Feature::Sketch { .. }) {
                self.sketching(uprooted.at).measured(build);
            }
        }
        build.revised();
        self.rebuilt(build);
        self.edits = self.edits.next();
    }

    /// Build afresh every solid the timeline stands for.
    ///
    /// Every way a document changes ends here, which is what keeps a feature
    /// standing downstream of a sketch honest: a profile names its region by
    /// what bounds it, and any edit at all may have drawn a line across one.
    /// Replaying is what a timeline is *for* — see
    /// [`Build::rebuild`](crate::build::Build::rebuild), which is where the cost
    /// of replaying the lot rather than the part is argued.
    ///
    /// Reads the document and writes only `build`, like everything derived: what
    /// an extrude says is written down, and where that currently lands is not.
    fn rebuilt(&self, build: &mut Build) {
        build.rebuild(self.timeline.swept());
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
    pub(crate) fn drawing_at(&self, at: FeatureId) -> Option<Drawing<'_>> {
        self.timeline.sketched(at)
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

    /// The solid at `at` as something that can be carried, and the line it
    /// travels.
    ///
    /// Beside [`Document::movable`] and for the reason that one is here: a press
    /// has the document and not the build, and where a thing may travel is the
    /// timeline's alone.
    pub(crate) fn stretching(&self, at: FeatureId) -> Movable {
        self.timeline.stretching(at)
    }

    /// The sketch a session should start in: none.
    ///
    /// **A document is opened, not entered.** What a drawing holds is on screen
    /// the moment it is raised, and which of its sketches you then work in is a
    /// thing you say by clicking one — the same gesture that says which *thing*
    /// you mean, and the only one there is. Starting you in whichever sketch
    /// happened to be first would be answering a question nobody asked, and
    /// answering it differently for a document that holds none.
    ///
    /// Constant, and a method all the same: it is where the answer is argued,
    /// and it is what raising a document reads. Which sketch is open after that
    /// is the session's — see [`Session::editing`](crate::session::Session) —
    /// because nothing about what you have open is written down by saving.
    pub(crate) fn opening(&self) -> Option<FeatureId> {
        None
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

    /// Every sketch the document holds as `build` last left them, with
    /// `editing` the one being worked in.
    ///
    /// The pairing everything that reads a drawing reads it through — see
    /// [`Models`]. Here because a caller holding a document and a build is
    /// holding both halves already, and naming the type to put them together
    /// would be ceremony.
    pub(crate) fn models<'a>(&'a self, build: &'a Build, editing: Option<FeatureId>) -> Models<'a> {
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
    /// Named here rather than reached through a borrowed drawing, so that every
    /// way a document changes is a call on the document. An undo is the path
    /// that most wants watching — it is the one that can make geometry stop
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
            Feature::Plane(_) | Feature::Extrude { .. } | Feature::Revolve { .. } => {
                build.revised()
            }
        }
        self.rebuilt(build);
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
    /// One of exactly four ways a document changes, and the only one anybody
    /// *asks* for. [`Document::restore`] is what the history puts back, and
    /// [`Document::pulled`] and [`Document::put_again`] are how it undoes and
    /// redoes a step being added — which this call is what performs, since a
    /// creation is asked for like anything else and only its *recording* is
    /// special. Everything else a document hands out is `&self`.
    ///
    /// `build` is the caller's. Solving is what an edit to a drawing *is*,
    /// and a solve wants room to work in — and leaves a report behind — that is
    /// worth keeping across a drag and worth nothing in a saved file. So both
    /// belong to whoever is doing the editing, and the document borrows them
    /// for the length of the call. An edit that could happen without one in
    /// hand would be an edit that left its report stale.
    ///
    /// Hands back what it did to the shape of the recipe — see [`Shaped`], which
    /// is the whole of what an edit cannot be asked about afterwards.
    pub(crate) fn apply(&mut self, build: &mut Build, change: Change) -> Shaped {
        // Read before the walk below, which takes the change apart.
        let about = change.about();
        let mut shaped = Shaped::Same;
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
            Change::Place {
                sketch,
                constraint,
                at,
            } => self.sketching(sketch).place(build, constraint, at),
            Change::Delete { sketch, entity } => self.sketching(sketch).remove(build, entity),
            Change::Tidy { sketch } => self.sketching(sketch).remove_duplicates(build),
            // Nothing to solve. The sketches on this plane are in its own
            // coordinates and say exactly what they said before; all that has
            // changed is where they land, which nothing keeps a copy of.
            Change::MovePlane { plane, to } => {
                self.timeline.offset(plane, to);
                build.revised();
            }
            // Nothing to solve either. How far a solid stands off its region is
            // a number the step holds; the region it is grown from has not
            // moved, and neither has anything the drawing says.
            Change::Carry { extrude, to } => {
                self.timeline.carry(extrude, to);
                build.revised();
            }
            // The one change that puts a step on the end. It arrives already
            // named, a profile of several regions being a list an intent
            // carries rather than positions it resolves a pass later — see
            // [`Change::Extrude`].
            Change::Extrude {
                profile,
                distance,
                operation,
            } => {
                shaped = Shaped::Made(self.timeline.add(Feature::Extrude {
                    profile,
                    distance,
                    operation,
                }));
                build.revised();
            }
            // The same, spun about a line of its own drawing rather than
            // carried off the plane — see [`Change::Revolve`].
            Change::Revolve {
                profile,
                axis,
                sector,
                operation,
            } => {
                shaped = Shaped::Made(self.timeline.add(Feature::Revolve {
                    profile,
                    axis,
                    sector,
                    operation,
                }));
                build.revised();
            }
            // The other step-adder, and the simpler of the two: a sketch is
            // born empty, so there is nothing to name and nothing to resolve
            // against the drawing it is starting on.
            //
            // Solved all the same, by the remodel below. An empty sketch has no
            // geometry to settle and no region to enclose, but everything that
            // reads one reads what the last solve made of it, and a sketch with
            // no report is one nothing can draw.
            Change::AddSketch { on } => {
                let at = self.timeline.add(Feature::Sketch {
                    on,
                    sketch: Sketch::default(),
                });
                // **Settled at once, the way a sketch read from a file is.** A
                // sketch arrives needing a solve whether it arrived with
                // coordinates or with none, because what everything downstream
                // reads is what the last solve *made* of one — and a build has
                // no entry at all for a sketch nothing has solved, so reading
                // one is a panic rather than an empty drawing. Empty is the
                // easiest case of the same call and not a case of its own.
                self.sketching(at).opened(build);
                shaped = Shaped::Made(at);
                build.revised();
            }
            // **The step named, and everything standing on it.** Which those
            // are is worked out here rather than named by the intent, because a
            // replayed pass naming them would be naming steps the first pass had
            // already taken — see [`Change::DeleteStep`].
            //
            // Nothing is solved. Every sketch that is left says exactly what it
            // said; what has changed is only which of them there are, and where
            // the ones downstream of a removed plane now land.
            Change::DeleteStep { step } => {
                // **Refused rather than asserted.** A world plane is a step
                // somebody can point at and press Delete on, so this is a state
                // to answer for rather than a caller's mistake — and the answer
                // is nothing done, which is what an empty [`Shaped::Took`] says.
                // Nothing is revised either: a document that did not move has no
                // reason to be drawn again.
                shaped = Shaped::Took(if self.timeline.removable(step) {
                    // A buffer of its own, this being the one reader that asks
                    // once per gesture — see [`Timeline::doomed`], whose other
                    // reader asks every frame and keeps one.
                    let mut doomed = Vec::new();
                    self.timeline.doomed(step, &mut doomed);
                    let pulled = self.pulled(build, &doomed);
                    build.revised();
                    // After the revision, which is what wipes the last report.
                    build.took_steps(pulled.len());
                    pulled
                } else {
                    Vec::new()
                });
            }
            // Nothing is solved and nothing is rebuilt *differently*: every step
            // resolves what it stands on by reference rather than by position,
            // so the model comes out identical however the recipe is ordered.
            // The rebuild below runs all the same, because the day a solid can
            // be built on another that stops being true, and a reorder that
            // skipped it would be a model quietly disagreeing with its recipe.
            Change::Reorder { step, to } => {
                shaped = Shaped::Moved {
                    from: self.timeline.position_of(step),
                    to,
                };
                self.timeline.shift(step, to);
                build.revised();
            }
            // Nothing is solved and nothing is rebuilt: every step says exactly
            // what it said, and which regions the extrudes resolve to is worked
            // out for all of them whatever the bar. What changes is only how
            // much of the answer is *read* — see
            // [`Models::at`](crate::model::Models).
            //
            // Revised all the same, unlike the camera below it: what is drawn
            // has changed, so a picture laid out before this is out of date.
            Change::RollTo { through } => {
                self.timeline.roll_to(through);
                build.revised();
            }
            Change::Orbit { yaw, pitch } => self.camera.orbit(yaw, pitch),
            Change::Aim { yaw, pitch } => self.camera.aim(yaw, pitch),
            Change::Dolly { factor } => self.camera.dolly(factor),
            Change::Pan { by } => self.camera.pan(by),
            Change::Project(projection) => self.camera.projection = projection,
        }
        // After the change rather than as part of one, because it is a fact
        // about every step and not about the one that moved — a line drawn in
        // one sketch can take away what an extrude two steps later was grown
        // from.
        //
        // A change about no step is the camera's, and the camera cannot reach an
        // arrangement: it writes one field of this and nothing the drawing says.
        // Skipping it is what keeps an orbit off this path, which the document
        // is careful about elsewhere for the same reason — see [`Edits`], on why
        // turning the camera must not move the revision.
        match about {
            About::Makes | About::Removes | About::Moves { .. } | About::Rewrites { .. } => {
                self.rebuilt(build)
            }
            About::Nothing => {}
        }
        self.edits = self.edits.next();
        shaped
    }
}

/// What applying a change did to the *shape* of the recipe.
///
/// **What the history cannot work out for itself**, and the only thing an edit
/// hands back. A rewrite is a value the history can read on either side of the
/// call; a step made and steps taken away are not — one has no name until it
/// lands, and the others have no place to go back to once they are gone.
///
/// Three arms because [`About`] has three, and this carries what each of them is
/// short of. That they line up is checked at the two call sites in
/// [`History`](crate::history::History), which ask for the arm the change
/// promised.
#[derive(Debug)]
pub(crate) enum Shaped {
    /// No step came or went: a value rewritten, or the camera turned.
    Same,
    /// A step made, under the name the document minted for it.
    Made(FeatureId),
    /// Steps taken out, in the order they sat — so putting them back is walking
    /// this forwards, and taking them out again is walking it back.
    ///
    /// Empty where the document refused: a world plane is not a step anybody may
    /// take out, and a refusal is nothing done rather than an error to report.
    Took(Vec<Uprooted>),
    /// A step moved, and both places it has been.
    ///
    /// Both, though the change named one of them: [`About`] is what the history
    /// reads a change by, and it says a step *moved* without saying where to —
    /// so a history reaching into `Change::Reorder` for the other half would be
    /// matching on a kind it had already been told about. One channel for the
    /// whole answer instead.
    Moved { from: usize, to: usize },
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
/// test or a bench aiming one without a pointer to aim it with. The shape and
/// both its gates are argued at [`CatCad::internals`](crate::internals).
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use crate::document::Document;
    #[cfg(test)]
    use crate::document::Edits;
    use crate::drawing::Drawing;
    use crate::timeline::FeatureId;
    use aperture::Camera;
    use glam::Vec3;

    impl Document {
        /// The sketch at `at`, which a fixture knows is there.
        ///
        /// [`Document::drawing_at`] with the answer unwrapped. That one is an
        /// `Option` because a handle to the sketch being edited outlives the
        /// edits that can take it away — see
        /// [`Models::new`](crate::model::Models) — and a fixture naming a step
        /// it just made is in no such position. One helper rather than the same
        /// `expect` written at forty call sites, and the message says which
        /// assumption broke.
        pub(crate) fn drawn(&self, at: FeatureId) -> Drawing<'_> {
            self.timeline.drawn(at)
        }

        /// The first sketch the timeline holds, which is the one every fixture
        /// here is about.
        ///
        /// Not [`Document::opening`], though today they answer the same step:
        /// that one says where a *session* starts and is an `Option` because a
        /// document may be opened on no sketch at all. What a fixture wants is
        /// the sketch, and a fixture that unwrapped the other would be writing
        /// down an assumption about the session in a test that is not about one.
        pub(crate) fn first_sketch(&self) -> FeatureId {
            self.timeline.first_sketch()
        }

        /// Every step, in the order they are built.
        ///
        /// What a *structural* edit is about, and the whole of what a test can
        /// hold on to across one: which steps there are and what order they run
        /// in. A document compared by what it draws would pass a delete that put
        /// the right steps back in the wrong places, because most of them land
        /// in the same world either way.
        ///
        /// Narrower than the mod around it, like [`Edits::stepped`] above: no
        /// harness links this, and only the unit tests about structural edits
        /// ask it.
        #[cfg(test)]
        pub(crate) fn recipe(&self) -> Vec<FeatureId> {
            self.timeline.steps().map(|(at, _)| at).collect()
        }
    }

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

        /// The far end of the demo's arm, which is the freest thing it draws
        /// and so the one worth taking hold of.
        ///
        /// A fact about the *fixture* rather than about a document, and here
        /// because every harness that drives a drag wants it and each was
        /// writing it out itself. **The named sketch's last point, not the
        /// scene's last marker**: the arm's points are added last of its own
        /// sketch, where the scene draws every sketch and its last marker
        /// belongs to whichever the document drew last. Read off the scene, this
        /// named a point three sketches over, and the bench's dragging step
        /// spent a release measuring a gesture that never solved.
        pub(crate) fn wrist(&self, sketch: FeatureId) -> Vec3 {
            let drawing = self.drawn(sketch);
            let (_, wrist) = drawing
                .sketch()
                .points()
                .last()
                .expect("the demo draws points");
            drawing.plane().point(wrist.position).as_vec3()
        }

        /// A spot on `sketch`'s plane with nothing drawn near it — where a tool
        /// has room to put something down.
        ///
        /// The other half of the same fixture, and narrower than [`wrist`]
        /// beside it: what a *click on nothing* should produce is a question
        /// only the unit tests ask.
        ///
        /// A sketch coordinate rather than a screen one, so the answer is known
        /// by hand. The demo's rectangle starts at sketch x = 0 and its slab
        /// reaches to x = −2, so a unit and a half to the left of the frame is
        /// on the slab, on screen, and the better part of a hundred pixels clear
        /// of the nearest stroke.
        ///
        /// [`wrist`]: Document::wrist
        #[cfg(test)]
        pub(crate) fn empty_spot(&self, sketch: FeatureId) -> Vec3 {
            self.drawn(sketch)
                .plane()
                .point(glam::DVec2::new(-1.5, 2.5))
                .as_vec3()
        }
    }
}
