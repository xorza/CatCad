//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

#[cfg(feature = "bench")]
mod bench;
mod build;
mod demo;
mod dialog;
mod document;
mod drawing;
mod filing;
mod history;
mod hud;
mod intent;
mod model;
mod names;
mod paint;
mod part;
mod preview;
mod profile;
mod prompt;
mod scene_view;
mod selection;
mod session;
mod timeline;
mod tool;

/// The one call `tests/alloc.rs` makes. The driver itself stays in `src/`,
/// where it can reach what it measures.
#[cfg(feature = "bench")]
pub use bench::alloc_bench;

use std::fmt;
use std::path::PathBuf;

use palantir::{
    App, Configure, HostHandle, Key, KeyFilter, Panel, Shortcut, Sizing, Ui, WindowToken,
};
use silverpoint::{Entity, Removed};

use crate::build::Build;
use crate::document::Document;
use crate::filing::Filing;
use crate::history::History;
use crate::hud::{Hud, Shown};
use crate::intent::{Change, Choice, Errand, Intent, Intents, Step};
use crate::part::Part;
use crate::prompt::{Asking, Stands};
use crate::scene_view::SceneView;
use crate::session::Session;
use crate::tool::Tool;

/// Take back the last step, and put it back.
///
/// Palantir normalises the command modifier at the input boundary, so one
/// binding is Ctrl on Windows and Linux and Cmd on macOS. Modifiers are matched
/// exactly, which is what keeps `Ctrl+Z` from firing on `Ctrl+Shift+Z`.
const UNDO: Shortcut = Shortcut::ctrl('Z');
const REDO: Shortcut = Shortcut::ctrl_shift('Z');

/// Take what is picked out out of the drawing.
///
/// A bare key rather than a chord, which is what every modeller binds it to.
///
/// Safe to bind bare, and nothing here is what makes it safe. `Delete` is of
/// the *edit* class, so a focused field claims it and this poll answers `false`
/// on its own — there is no gate to remember. Every binding above is an
/// accelerator, which no field takes, so all of them go on working while one is
/// open.
const DELETE: Shortcut = Shortcut::key(Key::Delete);

/// Put the document away, and fetch one back.
///
/// The three every modeller binds, in the places every modeller binds them.
/// Save As is Save with Shift, which is why the two are matched exactly rather
/// than by modifier subset — see [`UNDO`].
const SAVE: Shortcut = Shortcut::ctrl('S');
const SAVE_AS: Shortcut = Shortcut::ctrl_shift('S');
const OPEN: Shortcut = Shortcut::ctrl('O');

/// One view of one scene, with the controls and the solve's verdict laid over
/// it.
#[derive(Debug)]
pub struct CatCad {
    /// Everything a session would have to write down to be opened again: the
    /// sketch, and the camera looking at it.
    document: Document,
    /// How the document came to say what it says. Beside the document rather
    /// than in it: what is in one is what saving writes down, and the way here
    /// belongs to this run of the program.
    history: History,
    /// What this frame's gestures asked for, standing between the view that
    /// raised them and the document they land on. Kept across frames for its
    /// room rather than its contents, which are cleared before each one.
    intents: Intents,
    /// The room an edit's solve works in, and everything that solve leaves
    /// behind: its report, the drawing's faces, and the buffers those were
    /// worked out in.
    ///
    /// A tool rather than anything the app *is* — kept for exactly the reason
    /// the inbox above is, that a drag would otherwise ask the heap for the
    /// same buffers sixty times a second. Beside the document rather than in
    /// it, because all of it follows from the sketch by running the solver over
    /// it again: saving writes none of it and opening rebuilds it. Lent to
    /// whatever is editing, which is what keeps it in step — an edit that could
    /// happen without this in hand would be one that left it stale.
    build: Build,
    /// What draws that, and what the pointer over it is in the middle of. Owns
    /// nothing the document would be saved without.
    view: SceneView,
    /// What is in hand and what is picked out. Beside the document rather than
    /// in it, like the history above: what a session is drawing *with* is not
    /// part of what it has drawn.
    session: Session,
    /// What floats over the view: the tool bar, and the readout in the
    /// corner.
    hud: Hud,
    /// Where the document came from, whether it has been written since, and the
    /// question being asked about it. Beside the document rather than in it,
    /// like the history and the session: where a drawing lives is not something
    /// the drawing says.
    filing: Filing,
}

impl CatCad {
    /// What `WinitHost` calls. The host's arguments say nothing this app
    /// needs, so the work is all in [`CatCad::build`] — which is also what
    /// lets the offscreen harness raise the same app without a window.
    pub fn new(_ui: &mut Ui, _handle: HostHandle<Self>) -> Self {
        Self::build()
    }

    /// The app without a host, which is what the visual suite raises.
    pub fn build() -> Self {
        // The one build, made before anything that needs one. Opening a
        // document is a solve, so it is wanted here as much as it is per frame.
        let mut build = Build::default();
        let mut document = demo::document(&mut build);
        // Laid out before it is aimed at, and measured off the view rather than
        // off the document. What has to fit on screen is what will be *drawn*,
        // and how far that reaches is aperture's to say, not this crate's: a
        // ring reaches its radius along each world axis only in so far as its
        // plane does not lean away from it, and a stroke's width and a marker's
        // glyph reach nowhere at all, being screen-sized. A document measuring
        // itself would be a second copy of all of that, free to drift.
        // Opened in its first sketch, so a tool has somewhere to draw before
        // anything has been picked out.
        let session = Session::new(document.opening());
        let mut view = SceneView::new(&document, &build, session.editing());
        if let Some(extent) = view.extent() {
            document.frame(extent);
        }
        // Aiming the camera is not something the view was watching for, so it
        // is handed on here rather than left to the first frame. Not for the
        // sake of the painting — a recorded frame settles before its paint
        // command runs, so the first one would hand it on in time either way —
        // but so that what `build` returns already agrees with itself, and a
        // caller can measure the view it was given without recording a frame to
        // make the answer true.
        view.settle(&document, &build, &session);
        Self {
            document,
            history: History::default(),
            intents: Intents::default(),
            build,
            view,
            session,
            hud: Hud::default(),
            filing: Filing::default(),
        }
    }

    /// Take everything this frame's input asked for, before anything is drawn.
    ///
    /// **Records nothing.** The keyboard is polled where its chords
    /// belong — at the root, which is the scope that owns them — and the pointer
    /// over the drawing is polled by the view's own id rather than taken off a
    /// widget that has not been recorded yet. What that costs is a stable id;
    /// what it buys is that everything drawn below is drawn from a document this
    /// frame's gestures have already reached.
    fn poll(&mut self, ui: &mut Ui) {
        // Polled unconditionally rather than short-circuited: reading a chord
        // is also what subscribes it for the wake that delivers the next one,
        // so one left unread on the frame another fired would stop waking a
        // frame of its own.
        if ui.key_pressed(UNDO) {
            self.intents.push(Step::Undo);
        }
        if ui.key_pressed(REDO) {
            self.intents.push(Step::Redo);
        }
        if ui.key_pressed(SAVE) {
            self.intents.push(Errand::Save);
        }
        if ui.key_pressed(SAVE_AS) {
            self.intents.push(Errand::SaveAs);
        }
        if ui.key_pressed(OPEN) {
            self.intents.push(Errand::Open);
        }
        // Escape puts down whatever is in hand wherever the pointer happens to
        // be. The view answers for the right button over the drawing, which is
        // the same cancel by the gesture a modeller reaches for first, and the
        // bar for a second press of a tool's own button — three ways to ask for
        // the same thing, and none of them does it.
        if ui.escape_pressed() {
            self.intents.push(Choice::Hold(Tool::Pointer));
        }
        // Everything picked out, each named rather than "the selection": an
        // intent says where it wants to end up, and a replayed pass reading the
        // selection again would find it already gone. Landing twice is harmless
        // — the second removal finds nothing to remove.
        if ui.key_pressed(DELETE) {
            // Entities only. Deleting a face would mean deleting whatever
            // draws it, which is a different command and not this one — so a
            // face picked out alongside an edge lets the edge go and stays.
            for part in self.session.selection().picked() {
                if let (Some(sketch), Some(entity)) = (part.sketch(), part.entity()) {
                    self.intents.push(Change::Delete { sketch, entity });
                }
            }
        }
        self.view
            .poll(ui, &self.document, &self.session, &mut self.intents);
    }

    /// Draw the frame, from a document this frame's input has already moved.
    ///
    /// **In stacking order**, which is recording order: the drawing, then the
    /// field standing over a dimension in it, then the overlay — so a control on
    /// the bar wins a press the field could also claim.
    ///
    /// The widgets here raise intents of their own, and those are the ones the
    /// pointer could not be polled for: what a field made of a keystroke is
    /// known only once it has run. They land in the second apply, so a commit
    /// still reaches the document on the frame it was typed.
    fn draw(&mut self, ui: &mut Ui) {
        self.view.draw(ui);
        self.ask(ui);
        // Formatted straight into the pass's own text arena — no `String` is
        // built on the way, and the handle is lowered by the same pass that
        // minted it, which is the only pass it is good for.
        let status = ui.fmt(format_args!("{}", self.status()));
        self.hud.show(
            ui,
            Shown {
                tool: self.session.tool(),
                status,
                projection: self.document.camera().projection,
                models: self.document.models(&self.build, self.session.editing()),
                selection: self.session.selection(),
            },
            &mut self.intents,
        );
    }

    /// Show the form open against the drawing, where one is open.
    ///
    /// Placed by the *view*, because where geometry lands on screen is a
    /// question about the camera and the viewport and the view owns both — and
    /// answered against the drawing this frame's edits have not yet reached,
    /// like everything else in the asking half.
    ///
    /// Nothing is gated on the form being open. Its fields are focusable
    /// widgets, so palantir routes presses and keystrokes to them the way it
    /// routes them to a button on the bar; there is no arbitration for this to
    /// do and no keyboard for it to drain.
    fn ask(&mut self, ui: &mut Ui) {
        let Some(prompt) = self.session.prompt() else {
            return;
        };
        let Some(viewport) = self.view.viewport() else {
            return;
        };
        let camera = self.document.camera();
        let models = self.document.models(&self.build, self.session.editing());
        // Where each kind of form stands, worked out before the form is
        // borrowed *mutably* to be shown — the drawing is reached through the
        // session and the form is part of it.
        let stands = match prompt.about() {
            Asking::Dimension { part } => {
                let part = *part;
                // Where the mark *would* be drawn, which is where the field
                // stands instead — see [`paint::redraw`], which leaves out the
                // mark of whatever is being typed into. Read off the layout
                // rather than worked out again, so the field lands on the lane
                // the drawing gave the mark and not on the one an unstacked
                // anchor would have.
                let found = models.iter().find_map(|model| match model.entity(part) {
                    Some(Entity::Constraint(id)) => Some((model.drawing(), id)),
                    _ => None,
                });
                // Never missing, on the same terms `Session::prune` guarantees:
                // a form open over a dimension an undo took away is closed
                // before the frame that would draw it.
                let (drawing, id) =
                    found.expect("a form is open over a dimension the drawing no longer holds");
                // Unplaced is a different thing from gone, and only the second
                // is a broken promise. A drawing places the marks of the sketch
                // it is *in*, so a form outliving a click that opened another
                // sketch has a dimension the layout knows nothing about — and
                // that is a frame the form is not shown for rather than one it
                // is closed by, exactly as a form swung off screen is. See
                // [`prompt::footprint`].
                self.view
                    .placed(id)
                    .and_then(|placed| {
                        // The middle of the mark's own box rather than the point
                        // it hangs off, and worked out by the drawing: the box
                        // rises up the *run's* frame, which is the sketch
                        // plane's — see [`paint::mark_centre`].
                        let middle = paint::mark_centre(placed.mark, drawing, &camera, viewport);
                        camera.screen_of(middle, viewport)
                    })
                    .map(Stands::Over)
            }
            // Beside the centre already placed, which is all there is of the
            // circle until a radius says otherwise.
            Asking::Circle { sketch, center } => models
                .at(*sketch)
                .and_then(|model| {
                    // The whole rim rather than the centre and the pointer
                    // between them: a circle reaches its radius in *every*
                    // direction, so a box drawn to wherever the band happens to
                    // have got to is a quarter of one — and a form standing
                    // clear of that quarter stands on the rest.
                    let middle = model.drawing().at(*center);
                    let radius = self.view.band_rim().map_or(0.0, |to| middle.distance(to));
                    crate::prompt::footprint(model.rim_around(middle, radius), &camera, viewport)
                })
                .map(Stands::Beside),
            // Beside the circle it is about, which is the rim projected: a
            // form standing over the middle of one would cover the very
            // geometry the number is describing.
            Asking::Radius { sketch, circle } => {
                let rim = models.at(*sketch).map(|model| model.rim_of(*circle));
                rim.and_then(|rim| crate::prompt::footprint(rim, &camera, viewport))
                    .map(Stands::Beside)
            }
            // Resolved here rather than remembered, for the reason the form
            // holds a name at all: the arrangement it was read from is not the
            // one it is being drawn against.
            Asking::Extrude { profile } => profile
                .face_of(models)
                .and_then(|region| {
                    self.view
                        .region_footprint(models, profile.sketch(), region, &camera, viewport)
                })
                .map(Stands::Beside),
        };
        // Nowhere to stand is a frame the form is not shown for, not a form
        // that closes: the camera turning back brings it round again.
        let Some(stands) = stands else {
            return;
        };
        let Some(prompt) = self.session.prompt_mut() else {
            unreachable!("the form was open a moment ago");
        };
        prompt.show(ui, stands, models, &mut self.intents);
    }

    /// Land everything the frame asked for, on whichever of the three things a
    /// frame writes it belongs to.
    ///
    /// The session is written here rather than in the history, because none of
    /// what it holds is a step to take back: undoing a placed point should not
    /// put the tool that placed it back in your hand, nor disturb what is picked
    /// out around it. So the inbox is read twice over, once for what the session
    /// owns and once for what the document does — which costs a walk of a few
    /// entries, and means neither reader has to know what the other's intents
    /// mean.
    fn apply(&mut self) {
        // The models the *asking* was read against, which is what minting a
        // durable name out of a position needs — see
        // [`Asking::Extrude`](crate::prompt::Asking). Taken before the history
        // writes, for the same reason.
        self.session.apply(
            self.document.models(&self.build, self.session.editing()),
            &self.intents,
        );
        self.history
            .apply(&mut self.document, &mut self.build, &self.intents);
        // After the history, because an undo can take geometry the session was
        // still holding on to — see [`Session::prune`].
        self.session
            .prune(self.document.models(&self.build, self.session.editing()));
        // Last of all, because saving has to write down what this frame's
        // edits left rather than what they started from — and because opening
        // replaces the three things above, so anything landing after it would
        // be landing on a document that was never asked.
        self.run();
        // Emptied by the call that landed it, so no phase has to remember to.
        // A frame applies twice — see [`CatCad::record`] — and an inbox carried
        // into the second would land the first's asking again.
        self.intents.clear();
    }

    /// Run everything the frame asked of the application itself.
    ///
    /// The fourth group, and the only one that does not write a field beside
    /// the document — it writes the document, and the history and session with
    /// it. That is why this walks the inbox by index where its three peers
    /// iterate: an errand cannot be holding a borrow of the application while
    /// it replaces most of one. See [`Intents::at`].
    fn run(&mut self) {
        let mut nth = 0;
        while let Some(intent) = self.intents.at(nth) {
            nth += 1;
            let Intent::Errand(errand) = intent else {
                continue;
            };
            match errand {
                // Where it came from, and a dialog only if it came from
                // nowhere. That is what makes it the one worth binding: a
                // document that has a name is written without being asked
                // anything.
                Errand::Save => match self.filing.path() {
                    Some(path) => self.write(path.to_path_buf()),
                    None => self.save_as(),
                },
                Errand::SaveAs => self.save_as(),
                // A dismissed dialog is an answer, and the answer is no. It
                // says nothing about the document, so nothing here says
                // anything about it either — not even in the readout, where a
                // note about a dialog nobody used would push out whatever the
                // last thing that *did* happen was.
                Errand::Open => {
                    if let Some(path) = dialog::open(self.filing.path()) {
                        self.read(path);
                    }
                }
            }
        }
    }

    /// Ask where to put the document, and put it there.
    ///
    /// Both commands that need a dialog to save go through here — Save As, and
    /// the Save of a document that has never been anywhere — so the two cannot
    /// come to differ about what a Save As is.
    fn save_as(&mut self) {
        if let Some(path) = dialog::save(self.filing.path()) {
            self.write(path);
        }
    }

    /// Write the document to `path`, and note how it went.
    fn write(&mut self, path: PathBuf) {
        match self.document.save_to(&path) {
            Ok(()) => self.filing.wrote(path, self.document.edits()),
            // Where the document lives is left where it was. A write that
            // failed changed nothing, and a document that had been saved
            // before this one is still saved.
            Err(error) => self.filing.refused_write(&path, error),
        }
    }

    /// Open the document at `path`, or leave everything alone and say why.
    ///
    /// Everything this run has made of the document that was open goes with it,
    /// and the order is what makes a refusal harmless: nothing here is written
    /// until the file has been read, parsed, checked and solved — see
    /// [`Document::open`].
    fn read(&mut self, path: PathBuf) {
        let document = match Document::open(&mut self.build, &path) {
            Ok(document) => document,
            Err(error) => return self.filing.refused_read(&path, error),
        };
        self.document = document;
        // A different document is a different session and a different history.
        // Nothing that was in hand is in hand, nothing picked out still exists
        // to be picked out, and what was done to the document that was open
        // cannot be taken back off the one that replaced it.
        self.session = Session::new(self.document.opening());
        self.history = History::default();
        self.filing.opened(path, self.document.edits());
    }

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn status(&self) -> Status<'_> {
        let models = self.document.models(&self.build, self.session.editing());
        let model = models.open();
        let outcome = model.outcome();
        Status {
            converged: outcome.converged(),
            iterations: outcome.iterations(),
            degrees_of_freedom: outcome.degrees_of_freedom(),
            redundant_constraints: outcome.redundant_constraints(),
            lost: models.lost(),
            hovered: self.view.hovered(),
            cleaned: self.build.cleaned(),
            unsaved: self.filing.unsaved(self.document.edits()),
            filed: self.filing.report(),
        }
    }
}

/// What the status line says: the solve's verdict, and what the pointer is
/// over.
///
/// Its own fields rather than a borrowed [`Drawing`](crate::drawing::Drawing):
/// four values copied once a frame cost nothing, and a `Status` that could be
/// built out of nothing but numbers is one a test can check the wording of
/// without raising a document to do it.
///
/// A `Display` rather than a `String`, for two reasons. Its only caller writes
/// it straight into the record pass's own text arena, and a line rebuilt every
/// frame out of a report that changes only on a solve should not cost an
/// allocation to say so. And a value that can be written to any formatter is
/// one a test can read without raising a `Ui` to do it.
#[derive(Debug)]
struct Status<'a> {
    converged: bool,
    iterations: u32,
    /// What the sketch can still do, where the two above are only how the last
    /// run getting it there went.
    degrees_of_freedom: usize,
    redundant_constraints: usize,
    /// How many extrudes no longer know which region they are grown from.
    ///
    /// The one thing a document can say about a step *downstream* of the sketch
    /// being worked in, and the reason it is worth a line: a drawing whose
    /// regions have been cut up carries on looking exactly as it did, and the
    /// feature that has lost its footing says nothing until someone asks it to
    /// build. See [`Models::lost`](crate::model::Models::lost).
    lost: usize,
    hovered: Option<Part>,
    /// What the last cleanup took out, where that was the last thing done.
    ///
    /// Three states rather than two counts: nothing to say, a cleanup that
    /// found nothing, and a cleanup that took something. The middle one is the
    /// reason this is an `Option` — a command that answers a press with silence
    /// reads as a command that did not work.
    cleaned: Option<Removed>,
    /// Whether the document has been changed since it was last written.
    unsaved: bool,
    /// What the last thing asked of the filing came to, where anything has
    /// been. Already a sentence — see [`Filing::report`] — because there is
    /// nothing here that could word one: a path is not a number and this line
    /// is built sixty times a second.
    filed: Option<&'a str>,
}

/// What to call a part of the drawing where a person will read it.
///
/// Here rather than on [`Entity`], which is silverpoint's: what a thing is
/// called is this crate's business, and a segment reads as an *edge* — what the
/// drawing shows is the boundary of something, and "segment" is the solver's
/// word for it rather than the draughtsman's.
fn noun(part: Part) -> &'static str {
    match part {
        Part::Entity {
            entity: Entity::Point(_),
            ..
        } => "point",
        Part::Entity {
            entity: Entity::Segment(_),
            ..
        } => "edge",
        Part::Entity {
            entity: Entity::Circle(_),
            ..
        } => "circle",
        Part::Entity {
            entity: Entity::Constraint(_),
            ..
        } => "constraint",
        Part::Region { .. } => "region",
        Part::Plane(_) => "plane",
        // Which face of the solid is not said. It is the one under the cursor,
        // and "the far end of the extrude" is a sentence about the timeline
        // rather than about the thing being pointed at.
        Part::Solid { .. } => "face",
        Part::Growing => "depth",
    }
}

impl fmt::Display for Status<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.converged { "solved" } else { "unsolved" };
        write!(
            f,
            "{state} · {} dof · {} redundant · {} iterations",
            self.degrees_of_freedom, self.redundant_constraints, self.iterations,
        )?;
        if self.lost > 0 {
            // Named for what was lost rather than for the step that lost it: a
            // profile is what an extrude is grown from, and "1 extrude" would
            // read as though the extrude itself had gone.
            write!(f, " · {} profile", self.lost)?;
            if self.lost != 1 {
                f.write_str("s")?;
            }
            f.write_str(" lost")?;
        }
        if self.unsaved {
            f.write_str(" · unsaved")?;
        }
        if let Some(filed) = self.filed {
            write!(f, " · {filed}")?;
        }
        if let Some(entity) = self.hovered {
            write!(f, " · {}", noun(entity))?;
        }
        match self.cleaned {
            None => Ok(()),
            Some(cleaned) if cleaned.is_empty() => write!(f, " · nothing to clean up"),
            Some(cleaned) => {
                f.write_str(" · removed ")?;
                // In the drawing's words rather than the sketch's, like
                // everything else a person reads here — see [`noun`].
                let took = [
                    (cleaned.points, "point"),
                    (cleaned.segments, "edge"),
                    (cleaned.circles, "circle"),
                ];
                for (nth, (count, what)) in
                    took.into_iter().filter(|&(count, _)| count > 0).enumerate()
                {
                    if nth > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{count} {what}")?;
                    if count != 1 {
                        f.write_str("s")?;
                    }
                }
                Ok(())
            }
        }
    }
}

impl App for CatCad {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        Panel::zstack()
            .auto_id()
            // The application root, as far as the keyboard is concerned, and
            // that is what this declaration is for rather than any key it
            // claims for itself. A chord is granted to the innermost *scope*
            // whose filter takes its class, and a read is answered for by the
            // scope it was taken inside — so without a root there is nothing
            // outside the viewport's own scope for an accelerator to resolve
            // to, and `Ctrl+S` would be answered by whatever is being typed
            // into. `KeyFilter::ALL` because everything the application binds
            // is the application's until something nested says otherwise; the
            // viewport says so while a field is open, and the class split is
            // what keeps that from taking the accelerators with it.
            .input_scope(KeyFilter::ALL)
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                // **Read, apply, draw, apply, settle.** The order the rest of
                // the crate is built around, with the reading pulled clear of
                // the drawing: nothing writes the document until everything has
                // finished reading it, *and* nothing is drawn from a document
                // this frame's input has not reached yet. A field placed against
                // a camera the frame's own orbit had not turned trailed it by a
                // frame; so, more quietly, did every reading on the overlay.
                //
                // Two applies rather than one because the two kinds of asking
                // cannot share a phase. What the *pointer* asked can be polled
                // before anything records; what a *widget* made of a keystroke
                // is known only once it has run, and there is no polling that
                // ahead of it. So the widgets draw between them and their asking
                // lands in the second.
                //
                // The inbox is emptied by whichever apply lands it — see
                // [`CatCad::apply`] — so a phase always starts from an empty
                // one. That matters because a frame that settles records twice
                // and palantir drains the input queues between the two, making
                // the second pass a fresh reading of what is still latched. That
                // the pair is harmless is the inbox's own rule: an intent names
                // where it wants to end up, so a drag re-asked lands in the same
                // place and an orbit measures against a total the first pass
                // already took.
                self.poll(ui);
                self.apply();
                self.draw(ui);
                self.apply();
                self.view.settle(&self.document, &self.build, &self.session);
            });
    }
}

/// What a harness reaches past the app for.
///
/// Both of these are ways of standing outside a frame: the app itself never
/// wants either, because everything it draws it draws from the document it has
/// just written, through a view that lays itself out. A caller that wants to
/// aim the camera or edit the scene by hand is one driving the app without a
/// pointer, which is a test or a bench and nothing else — so neither is part of
/// what this crate publishes to a program that merely runs it.
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use std::cell::RefCell;
    use std::rc::Rc;

    use aperture::{Camera, Renderer};

    use crate::CatCad;

    impl CatCad {
        /// The width every sketch stroke is authored at, in logical pixels.
        ///
        /// Published so the visual suite can measure against the number the
        /// drawing was actually drawn with. It lives in `paint` and is that
        /// module's to choose; what a test needs is not a second opinion about
        /// the right width but the same one, and an integration test cannot see
        /// a `pub(crate)` const.
        ///
        /// On the type rather than beside it, because the module this sits in is
        /// `pub(crate)` and a free function in one reaches nobody.
        pub fn edge_width() -> f32 {
            crate::paint::EDGE_WIDTH
        }

        /// The renderer behind the view, so a harness can reach the scene
        /// without a pointer to drive it with.
        pub fn renderer(&self) -> &Rc<RefCell<Renderer>> {
            self.view.renderer()
        }

        /// Where the document is looked at from, for a harness that wants to
        /// aim it without a pointer.
        ///
        /// The document's own rather than the renderer's: the renderer is
        /// handed a copy at the end of every frame, so anything written into
        /// that copy is gone by the time a frame is drawn through it.
        pub fn camera_mut(&mut self) -> &mut Camera {
            self.document.camera_mut()
        }

        /// Whether the pointer is over the thing `tag` names.
        ///
        /// Published for the visual suite, which is the only place a mark can
        /// be hovered at all: a run is pickable only once a frame has laid it
        /// out, and laying one out wants a device.
        pub fn hovering(&self, tag: aperture::Tag) -> bool {
            self.view.hovering(tag)
        }
    }
}

#[cfg(test)]
mod tests;
