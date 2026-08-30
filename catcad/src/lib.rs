//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

mod build;
mod control;
mod demo;
mod dialog;
mod document;
mod drawing;
mod filing;
mod history;
mod hud;
mod intent;
mod lens;
mod look;
mod model;
mod paint;
mod part;
mod preview;
mod profile;
mod prompt;
mod scene_view;
mod selection;
mod session;
mod status;
mod timeline;
mod tool;
mod wording;

use std::path::PathBuf;

use palantir::{
    App, Configure, HostHandle, Key, KeyFilter, Mods, Panel, Shortcut, Sizing, Ui, WindowToken,
};

use crate::build::Build;
use crate::document::Document;
use crate::filing::Filing;
use crate::history::History;
use crate::hud::{Hud, Shown};
use crate::intent::change::Change;
use crate::intent::{Choice, Errand, Intent, Intents, Step};
use crate::look::Theme;
use crate::look::icons::Icons;
use crate::part::Part;
use crate::scene_view::SceneView;
use crate::session::Session;
use crate::status::{Solved, Status};
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
/// Move the picked step one place earlier or later in the recipe.
///
/// **The chord and not a drag on the row**, deliberately. A drag is the gesture
/// a tree wants, and it wants live feedback with it — a row that follows the
/// pointer, or a gap opening where it would land — because a drag without one is
/// a gesture you make blind. That is worth building when the order *does*
/// something: today every step resolves what it stands on by reference, so a
/// reorder changes what the tree shows and what the file writes and nothing
/// else. A press moves the row by one and shows it, which is the whole gesture
/// and needs nothing held between frames.
///
/// Ctrl rather than bare arrows, which the view will want for nudging geometry.
/// Build the recipe only as far as the picked step, and build the whole of it
/// again.
///
/// A pair rather than one chord that toggles, because rolling *forward* is not
/// undoing a roll back: what it wants is the end of the recipe whatever the bar
/// currently rests on, and a toggle would have to remember where it had been.
///
/// The same shape as the reorder chords beside them and for the same reason —
/// the bar is a thing to drag, and a drag wants live feedback. See
/// [`REORDER_UP`].
const ROLL_TO: Shortcut = Shortcut::ctrl('R');
const ROLL_FORWARD: Shortcut = Shortcut::ctrl_shift('R');
const REORDER_UP: Shortcut = Shortcut::new(Mods::CTRL, Key::ArrowUp);
const REORDER_DOWN: Shortcut = Shortcut::new(Mods::CTRL, Key::ArrowDown);
const NEW: Shortcut = Shortcut::ctrl('N');
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
    /// What floats over the view: the five surfaces, one per edge and corner.
    hud: Hud,
    /// The artwork the overlay draws with, taken up afresh each frame and held
    /// across the one that took it up.
    ///
    /// Held rather than made and dropped inside the frame, because the record
    /// pass writes shapes *naming* the set and the paint that reads them runs
    /// at submit — after recording has returned. A set dropped in between
    /// unloads the rasters the frame is about to draw.
    ///
    /// `None` until the first frame: taking one up wants a [`Ui`], and
    /// [`CatCad::build`] has none.
    icons: Option<Icons>,
    /// Every colour, weight and metric the application draws with, and the
    /// palantir theme derived from it.
    theme: Theme,
    /// Where the document came from, whether it has been written since, and the
    /// question being asked about it. Beside the document rather than in it,
    /// like the history and the session: where a drawing lives is not something
    /// the drawing says.
    filing: Filing,
}

impl CatCad {
    /// What `WinitHost` calls. The host's arguments say nothing this app
    /// needs, so the work is all in `CatCad::dressed` — which is also what
    /// lets the offscreen harness raise the same app without a window, in
    /// colours of its own.
    pub fn new(_ui: &mut Ui, _handle: HostHandle<Self>) -> Self {
        Self::build()
    }

    /// The app without a host, in the colours it ships in.
    pub fn build() -> Self {
        Self::dressed(Theme::default())
    }

    /// The same app, wearing `theme`.
    ///
    /// The seam the visual suite raises the app through, so that what a golden
    /// is a claim about is not a table generated upstream of this crate. The
    /// argument is at `Palette::probe`, which is the table it raises.
    fn dressed(theme: Theme) -> Self {
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
        //
        // The sheets standing for the planes are no part of it, and are left out
        // where a scene says how far it reaches rather than here — furniture is
        // sized to what it stands around, so a camera framed on it would frame
        // the room. See [`Scene::extent`](aperture::Scene).
        // Opened in no sketch at all, which is how every document opens — see
        // [`Document::opening`]. What the demo shows before anything is clicked
        // is its solid and the planes it was built on.
        let session = Session::new(document.opening());
        let mut view = SceneView::new(&document, &build, &theme, session.editing());
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
        // The overlay before the settle, which wants what the overlay knows:
        // nothing yet, the gizmo having neither arranged nor been pointed at.
        let hud = Hud::default();
        view.settle(&document, &build, &theme, &session, hud.gizmo());
        Self {
            document,
            history: History::default(),
            intents: Intents::default(),
            build,
            view,
            session,
            hud,
            icons: None,
            theme,
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
        if ui.key_pressed(NEW) {
            self.intents.push(Errand::New);
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
        // Escape backs out of one thing at a time, innermost first: whatever
        // is in hand, and then the sketch it was drawing in. Two steps rather
        // than one, because they are two things to be out of — a tool put down
        // by a key that also closed the drawing would be a key you could not
        // use without losing your place.
        //
        // The view answers for the right button over the drawing, which is the
        // same cancel by the gesture a modeller reaches for first, and the bar
        // for a second press of a tool's own button — three ways to ask for the
        // first step, and none of them does it here.
        if ui.escape_pressed() {
            self.intents.push(match self.session.tool() {
                Tool::Pointer => Intent::from(Choice::Close),
                _ => Choice::Hold(Tool::Pointer).into(),
            });
        }
        // Whatever is picked out, so this reaches a step nothing else can: the
        // bar is the one thing that answers for a step *below* it, and a step
        // below the bar is not built and so is not drawn.
        if ui.key_pressed(ROLL_FORWARD) {
            self.intents.push(Change::RollTo { through: None });
        }
        // One step and no more, because a move puts *a* step somewhere: two at
        // once would be two moves whose second is measured against what the
        // first left, which is a thing to ask for twice if it is wanted.
        if let [Part::Step(step)] = *self.session.selection().picked() {
            if ui.key_pressed(ROLL_TO) {
                self.intents.push(Change::RollTo {
                    through: Some(step),
                });
            }
            for (chord, by) in [(REORDER_UP, -1), (REORDER_DOWN, 1)] {
                // Clamped here rather than refused there, so the document is
                // never asked for a position a step may not take — see
                // [`Change::Reorder`]. At the end of its run the key does
                // nothing, which is not the same as a step that moved nowhere:
                // that one would be an undo to press and watch do nothing.
                if ui.key_pressed(chord)
                    && let Some(to) = self.document.nudged(step, by)
                {
                    self.intents.push(Change::Reorder { step, to });
                }
            }
        }
        // Everything picked out, each named rather than "the selection": an
        // intent says where it wants to end up, and a replayed pass reading the
        // selection again would find it already gone. Landing twice is harmless
        // — the second removal finds nothing to remove.
        if ui.key_pressed(DELETE) {
            for part in self.session.selection().picked() {
                match part {
                    // **One key, and what it takes follows from what is
                    // picked.** A plane is a step of the recipe rather than
                    // anything drawn, so what goes with it is every step built
                    // on it — see [`Change::DeleteStep`]. A world plane is
                    // refused, and the refusal is the document's: the key says
                    // what was asked for, and what may be answered is decided
                    // where the answer is.
                    Part::Step(step) => self.intents.push(Change::DeleteStep { step: *step }),
                    // Entities otherwise. Deleting a face would mean deleting
                    // whatever draws it, which is a different command and not
                    // this one — so a face picked out alongside an edge lets the
                    // edge go and stays.
                    part => {
                        if let (Some(sketch), Some(entity)) = (part.sketch(), part.entity()) {
                            self.intents.push(Change::Delete { sketch, entity });
                        }
                    }
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
        // Taken up here rather than at construction, and afresh every frame: a
        // set is registered against the host that will draw it — see
        // [`Icons::load`]. Before the form as well as before the overlay,
        // because both of them draw chips.
        //
        // Parked on the application and drawn through a clone, which is a
        // reference count: what the record pass writes names the set, and the
        // paint that reads it runs after recording has returned.
        let icons = self.icons.insert(Icons::load(ui)).clone();
        self.view.draw(ui);
        self.ask(ui, &icons);
        // Formatted straight into the pass's own text arena — no `String` is
        // built on the way, and the handle is lowered by the same pass that
        // minted it, which is the only pass it is good for.
        let reported = self.status();
        let solved = reported.solved;
        let rest = ui.fmt(format_args!("{}", reported.rest()));
        // Palantir's own widgets resolve against whatever palette they are
        // handed, so the derived theme is installed before anything records.
        // Built on the frame it is first wanted and handed over as a reference
        // count after that — see [`Theme::dressed`].
        ui.set_theme(self.theme.dressed().palantir.clone());
        self.hud.show(
            ui,
            Shown {
                icons: &icons,
                theme: &self.theme,
                tool: self.session.tool(),
                rest,
                solved,
                camera: self.document.camera(),
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
    fn ask(&mut self, ui: &mut Ui, icons: &Icons) {
        // How the drawing is being looked at, which is what placing a form
        // against it is answered in. `None` until the view has arranged, and
        // until then there is nowhere on screen for a form to stand.
        let Some(lens) = self.view.lens(self.document.camera()) else {
            return;
        };
        // Read before the form is taken, because both come off the session: one
        // asks which sketch is open and the other borrows the form itself.
        let models = self.document.models(&self.build, self.session.editing());
        let Some(prompt) = self.session.prompt_mut() else {
            return;
        };
        // Where the form stands, asked of the view — which holds both halves of
        // that answer, the room the camera is read in and the layout the marks
        // were placed in.
        //
        // Nowhere to stand is a frame the form is not shown for rather than a
        // form that closes — see [`SceneView::stands`].
        let Some(stands) = self.view.stands(prompt.about(), models, lens) else {
            return;
        };
        prompt.show(ui, &self.theme, icons, stands, models, &mut self.intents);
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
        let made = self
            .history
            .apply(&mut self.document, &mut self.build, &self.intents);
        // A sketch this frame made is the sketch this frame opens, and this is
        // the one thing a frame decides that the inbox cannot carry: the handle
        // did not exist when the asking was read. See [`Session::entered`].
        self.session.entered(
            made,
            self.document.models(&self.build, self.session.editing()),
        );
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
                Errand::New => self.start_new(),
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
                // The one errand that reads the *view*: how far the scene
                // reaches is the picture's to answer, and walking it is why
                // this is asked for rather than offered every frame.
                Errand::Fit => {
                    if let Some(extent) = self.view.extent() {
                        self.document.frame(extent);
                    }
                }
            }
        }
    }

    /// Throw everything away and start on an empty document.
    ///
    /// The same replacement [`CatCad::read`] performs, minus the reading: a
    /// different document is a different session and a different history, and
    /// this one has never been anywhere, so it has no path to save back to
    /// either. Nothing is guarded and nothing is asked — see [`Errand::New`].
    ///
    /// The camera comes back to its default with the rest, because the camera is
    /// the *document's* — see [`Document`] — so a new one arrives with a new one.
    /// Nothing aims it afterwards, unlike [`CatCad::build`]: framing needs an
    /// extent and an empty document has none, its three planes being drawn at
    /// the origin at a fixed size on screen rather than reaching anywhere. Which
    /// is the answer that wanted no code — the default already looks at the
    /// origin, and the origin is where the three cross.
    fn start_new(&mut self) {
        self.document = Document::empty(&mut self.build);
        self.session = Session::new(self.document.opening());
        self.history = History::default();
        // Back to a document that has never been anywhere, which is what
        // `Save` then asks about — see [`Filing`](crate::filing::Filing).
        self.filing = Filing::default();
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
        Status {
            solved: models.open().map(|model| {
                let outcome = model.outcome();
                Solved {
                    converged: outcome.converged(),
                    iterations: outcome.iterations(),
                    degrees_of_freedom: outcome.degrees_of_freedom(),
                    redundant_constraints: outcome.redundant_constraints(),
                }
            }),
            lost: models.lost(),
            unmerged: models.unmerged(),
            hovered: self.view.hovered(),
            reported: self.build.reported(),
            unsaved: self.filing.unsaved(self.document.edits()),
            filed: self.filing.report(),
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
                self.view.settle(
                    &self.document,
                    &self.build,
                    &self.theme,
                    &self.session,
                    self.hud.gizmo(),
                );
            });
    }
}

/// What a harness reaches past the app for.
///
/// **Where the whole shape is argued**, because it repeats at every layer below
/// and the argument should not. Everything here is a way of standing outside a
/// frame: the app itself never wants any of it, since everything it draws it
/// draws from the document it has just written, through a view that lays itself
/// out. A caller that wants to aim the camera or read the scene by hand is one
/// driving the app without a pointer, which is a test or a bench and nothing
/// else — so none of it is part of what this crate publishes to a program that
/// merely runs it.
///
/// **Two gates, and the same two wherever this appears.** What an integration
/// test or a bench reaches has to survive `cfg(test)` being off in the library
/// it links, so it rides the `internals` feature; what only this crate's own
/// unit tests want is gated on `test` alone, so a build that turns the feature
/// on carries no method nothing outside can call. The second is spelt as a
/// `looking` module inside the first.
///
/// **And it is layered, because the fields are.** A harness holds a [`CatCad`];
/// the renderer it wants is a private field of a private field of one. So an
/// answer is forwarded outward rather than the fields opened up, and what each
/// layer adds is reach and nothing else — which is why a reach-in is argued
/// where the thing it reaches *lives*, and every layer above says only what it
/// forwards to.
///
/// **Which is why nothing here links inward.** These methods are `pub`, so a
/// harness that turns the feature on reads their documentation with only the
/// published surface in front of it — and every layer they forward to is
/// private. A link into that tree is dead for the one reader it is written for,
/// and rustdoc refuses it outright. So what a reach-in forwards to is *named*,
/// in backticks, and the argument stays where the thing lives.
///
/// `cargo doc --document-private-items --all-features` is what catches a link
/// that forgets. Without the feature this module is not compiled at all, so the
/// plain doc build says nothing about it.
#[cfg(any(test, feature = "internals"))]
pub(crate) mod internals {
    use std::cell::{Ref, RefCell, RefMut};
    use std::rc::Rc;

    use aperture::{Camera, Pane, Renderer};

    use crate::CatCad;
    use crate::hud;
    #[cfg(test)]
    use crate::intent::Intent;
    use crate::intent::{Choice, Intents, Opening};
    use crate::look::Theme;
    use crate::look::palette::Palette;
    use crate::tool::Tool;

    /// The window every harness in this crate lays its frames out in.
    ///
    /// Stated once because a screen coordinate only means anything against it:
    /// the two harnesses sweep the view for a cursor and read what a projection
    /// put where, and a size each would be two frames of reference wearing one
    /// name. Wide enough for the whole demo at the angle it opens at.
    #[cfg(test)]
    pub(crate) const HARNESS_SIZE: glam::UVec2 = glam::UVec2::new(800, 600);

    impl CatCad {
        /// The app in the visual suite's own colours, which is what that suite
        /// raises.
        ///
        /// **Not [`CatCad::build`], and the difference is the whole point** —
        /// argued at `Palette::probe`, which is the table this dresses the app
        /// in.
        ///
        /// The three colour readings below answer for this app and not for the
        /// shipped one, so a suite that reached past this for its frames would
        /// be measuring one palette against another.
        pub fn probe() -> Self {
            Self::dressed(Theme::probe())
        }

        /// The width every sketch stroke is authored at, in logical pixels.
        ///
        /// Off this application's own theme, so what a test holds a stroke to is
        /// the number the drawing in front of it was drawn with — a second
        /// application under a second theme answers its own.
        ///
        /// On the type rather than beside it, because the module this sits in is
        /// `pub(crate)` and a free function in one reaches nobody.
        pub fn edge_width(&self) -> f32 {
            self.theme.drawing.edge
        }

        /// The sRGB bytes the drawing's background is painted in, in a frame
        /// [`CatCad::probe`] raised.
        ///
        /// Off the table for the reason [`CatCad::edge_width`] is off the
        /// theme: a frame sweep that counted a colour written out beside it
        /// would be a sweep the palette can walk away from. Everything the
        /// overlay paints flat lands on the target as these very bytes, so what
        /// a sweep asks is equality and not a window.
        ///
        /// **The suite's table and not the shipped one**, which is why the
        /// answer is bound to `CatCad::probe` rather than to the application. A
        /// sweep run over a frame the shipped palette painted would be asking
        /// one table about another.
        ///
        /// Three calls rather than one taking which colour it wants, because
        /// naming a role would mean a type an integration test can reach and
        /// this module is not one — see the note above. Associated rather than
        /// taken off an instance, because a sweep runs per pixel and the
        /// application it swept has been dropped by then.
        pub fn ground_srgb() -> [u8; 3] {
            Palette::probe().ground.srgb()
        }

        /// The sRGB bytes a pinned point is painted in.
        pub fn pinned_srgb() -> [u8; 3] {
            Palette::probe().pinned.srgb()
        }

        /// The sRGB bytes geometry the constraints have not pinned is painted
        /// in.
        pub fn free_srgb() -> [u8; 3] {
            Palette::probe().free.srgb()
        }

        /// The renderer behind the view, so a harness can reach the scene
        /// without a pointer to drive it with.
        pub fn renderer(&self) -> &Rc<RefCell<Renderer>> {
            self.view.renderer()
        }

        /// The pane the drawing is in, which is what a harness reaching the
        /// scene almost always wants — see `SceneView::pane` one layer in.
        pub fn pane(&self) -> Ref<'_, Pane> {
            self.view.pane()
        }

        /// The same to write into, for a harness staging a frame by rewriting
        /// what the application drew.
        pub fn pane_mut(&self) -> RefMut<'_, Pane> {
            self.view.pane_mut()
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

        /// What the app is modelling, as the drawing it is being worked in.
        ///
        /// The one reading the app takes of itself before it asks the session
        /// or the history anything, and so the one a test setting either up has
        /// to take too. Written out at the call site it is a three-part chain
        /// over two private fields, which is a paragraph wherever a test wants
        /// one line.
        #[cfg(test)]
        pub(crate) fn models(&self) -> crate::model::Models<'_> {
            self.document.models(&self.build, self.session.editing())
        }

        /// Whether the pointer is over the thing `tag` names.
        ///
        /// Published for the visual suite, which is the only place a mark can
        /// be hovered at all: a run is pickable only once a frame has laid it
        /// out, and laying one out wants a device.
        pub fn hovering(&self, tag: aperture::Tag) -> bool {
            self.view.hovering(tag)
        }

        /// The sketch the session has open, which every fixture below builds
        /// on.
        ///
        /// An `expect` rather than an answer, because a fixture that has raised
        /// the demo *is* in a sketch: this is a harness saying what it has set
        /// up, not code guarding against a state. See
        /// [`Session::editing`](crate::session::Session), which is where the
        /// state itself is answered for.
        pub(crate) fn editing(&self) -> crate::timeline::FeatureId {
            self.session
                .editing()
                .expect("a raised document opens in a sketch")
        }

        /// Open the document's first sketch — see
        /// `Session::enter_first_sketch`.
        ///
        /// The session's, plus the one thing that is the *app's*: the picture
        /// caught up with it, which a frame would do a phase later. A harness
        /// that painted without recording would otherwise paint the document as
        /// it stood before any of this — laid out on no sketch, every plane
        /// showing.
        pub fn enter_first_sketch(&mut self) {
            self.session.enter_first_sketch(&self.document, &self.build);
            self.view.settle(
                &self.document,
                &self.build,
                &self.theme,
                &self.session,
                self.hud.gizmo(),
            );
        }

        /// Open the form that decides the depth of a solid grown off `region`
        /// of the sketch the session has open.
        ///
        /// Through the inbox rather than by writing the session, the way the
        /// relation bar's own Extrude chip asks. Here because a gate measuring
        /// the frames a depth is typed over has to be *in* one, and what opens
        /// a form is this crate's own vocabulary rather than a harness's.
        ///
        /// Nothing reaches the timeline: a form open on a region is a solid on
        /// screen and a step the document has not heard of — see
        /// `Opening::Extrude`.
        pub fn ask_for_a_depth(&mut self, region: usize) {
            let sketch = self.editing();
            let mut intents = Intents::default();
            let Self {
                session,
                document,
                build,
                ..
            } = self;
            let models = document.models(build, session.editing());
            let profile = models
                .at(sketch)
                .expect("a harness asks for a depth off a sketch it opened")
                .profile(&[region]);
            intents.push(Choice::Ask(Some(Opening::Extrude { profile })));
            session.apply(models, &intents);
        }

        /// Put `to` in the open form's depth field, the way a drag on the
        /// arrow carrying the solid does.
        ///
        /// Through the inbox, because that is how the arrow writes it: a drag
        /// sends one `Choice::Set` a frame and the
        /// form decides what the number comes to. So a gate driving this
        /// drives the frames a depth is decided over without having to find
        /// the arrow in the picture first.
        pub fn set_a_depth(&mut self, to: f64) {
            let mut intents = Intents::default();
            intents.push(Choice::Set { nth: 0, to });
            let Self {
                session,
                document,
                build,
                ..
            } = self;
            session.apply(document.models(build, session.editing()), &intents);
        }

        /// What the open form's depth field reads as, or `None` where no form
        /// is deciding one.
        ///
        /// What a gate asserts to say it reached the frame it names: a depth
        /// that stopped moving is a preview that stopped being rebuilt, and a
        /// gate on a number would go on reporting zero about it.
        pub fn deciding_a_depth(&self) -> Option<f64> {
            self.session.prompt()?.shows(0)
        }

        /// Ask the session for `choice`, the way the bar or a gesture asks.
        ///
        /// Through the inbox rather than by writing the session, because that is
        /// the only way the application ever changes one.
        #[cfg(test)]
        pub(crate) fn choose(&mut self, choice: impl Into<Intent>) {
            let mut intents = Intents::default();
            intents.push(choice);
            // The fields are named one at a time rather than through
            // [`CatCad::models`], which borrows the whole app and so cannot be
            // handed to the session it belongs to.
            let Self {
                session,
                document,
                build,
                ..
            } = self;
            session.apply(document.models(build, session.editing()), &intents);
        }

        /// The far end of the demo's arm, which is the freest thing it draws
        /// and so the one worth taking hold of.
        ///
        /// Forwarded rather than answered — the document is what knows, and its
        /// own gated half is where the argument for this lives. Here because
        /// what the answer reads is two private fields of one app: the
        /// document, and which sketch the session has open.
        pub fn wrist(&self) -> glam::Vec3 {
            self.document.wrist(self.editing())
        }

        /// Where a tool has room to put something down, forwarded the same way
        /// as [`CatCad::wrist`].
        #[cfg(test)]
        pub fn empty_spot(&self) -> glam::Vec3 {
            self.document.empty_spot(self.editing())
        }

        /// Whether anything in the drawing is under the pointer.
        ///
        /// The answer rather than what is hovered, for the reason every forward
        /// here is one: what a `Part` names is this crate's own vocabulary, and
        /// a harness asking whether a frame hovers *something* has no use for
        /// it.
        pub fn hovering_anything(&self) -> bool {
            self.view.hovered().is_some()
        }

        /// Whether the line tool is the one in hand.
        pub fn drawing_a_line(&self) -> bool {
            matches!(self.session.tool(), Tool::Line { .. })
        }

        /// Whether the tool in hand has one end of something down already.
        ///
        /// What says a rubber band is being drawn: the first click landed, and
        /// the second has not.
        pub fn tool_begun(&self) -> bool {
            self.session.tool().started().is_some()
        }

        /// One tool on the rail, by the label it is named and captioned with.
        ///
        /// Associated rather than free, for the reason
        /// [`CatCad::ground_srgb`] is: the module this sits in is `pub(crate)`,
        /// so a free function in one reaches nobody.
        pub fn tool_chip(label: &str) -> palantir::WidgetId {
            hud::internals::tool(label)
        }
    }
}

#[cfg(test)]
mod tests;
