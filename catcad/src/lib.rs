//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

#[cfg(feature = "bench")]
mod bench;
mod demo;
mod document;
mod drawing;
mod history;
mod intent;
pub mod named;
mod overlay;
mod paint;
mod scene_view;

/// The one call `tests/alloc.rs` makes. The driver itself stays in `src/`,
/// where it can reach what it measures.
#[cfg(feature = "bench")]
pub use bench::alloc_bench;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use aperture::{Camera, Renderer};
use palantir::{App, Configure, HostHandle, Panel, Shortcut, Sizing, Ui, WindowToken};
use silverpoint::{SolveReport, Solver};

use crate::document::Document;
use crate::history::History;
use crate::intent::{Intent, Intents};
use crate::named::Named;
use crate::scene_view::SceneView;

/// Take back the last step, and put it back.
///
/// Palantir normalises the command modifier at the input boundary, so one
/// binding is Ctrl on Windows and Linux and Cmd on macOS. Modifiers are matched
/// exactly, which is what keeps `Ctrl+Z` from firing on `Ctrl+Shift+Z`.
const UNDO: Shortcut = Shortcut::ctrl('Z');
const REDO: Shortcut = Shortcut::ctrl_shift('Z');

/// One view of one scene, with the controls and the solve's verdict laid over
/// it.
#[derive(Debug)]
pub struct CatCad {
    /// Everything a session would have to write down to be opened again: the
    /// sketch, the solids beside it, and the camera looking at them.
    document: Document,
    /// How the document came to say what it says. Beside the document rather
    /// than in it: what is in one is what saving writes down, and the way here
    /// belongs to this run of the program.
    history: History,
    /// What this frame's gestures asked for, standing between the view that
    /// raised them and the document they land on. Kept across frames for its
    /// room rather than its contents, which are cleared before each one.
    intents: Intents,
    /// The room an edit's solve works in.
    ///
    /// A tool rather than anything the app *is* — kept for exactly the reason
    /// the inbox above is, that a drag would otherwise ask the heap for the
    /// same buffers sixty times a second. Lent to whatever is editing for the
    /// length of the call, which is why it is neither in the document that
    /// would be saved nor in the history of what has been done to it.
    solver: Solver,
    /// What draws that, and what the pointer over it is in the middle of. Owns
    /// nothing the document would be saved without.
    view: SceneView,
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
        // The one solver, made before anything that needs one. Opening a
        // document is a solve, so it is wanted here as much as it is per frame.
        let mut solver = Solver::default();
        let mut document = demo::document(&mut solver);
        // Laid out before it is aimed at, and measured off the view rather than
        // off the document. What has to fit on screen is what will be *drawn*,
        // and how far that reaches is aperture's to say, not this crate's: a
        // ring reaches its radius along each world axis only in so far as its
        // plane does not lean away from it, and a stroke's width and a marker's
        // glyph reach nowhere at all, being screen-sized. A document measuring
        // itself would be a second copy of all of that, free to drift.
        let mut view = SceneView::new(&document);
        if let Some(bounds) = view.bounds() {
            document.camera_mut().frame(bounds);
        }
        // Aiming the camera is not something the view was watching for, so it
        // is handed on: nothing has been painted yet, and this is what the
        // first paint will be painted through.
        view.settle(&document);
        Self {
            document,
            history: History::default(),
            intents: Intents::default(),
            solver,
            view,
        }
    }

    /// The renderer behind the view, so a harness can reach the scene without a
    /// pointer to drive it with.
    pub fn renderer(&self) -> &Rc<RefCell<Renderer>> {
        self.view.renderer()
    }

    /// Where the document is looked at from, for a harness that wants to aim it
    /// without a pointer.
    ///
    /// The document's own rather than the renderer's: the renderer is handed a
    /// copy at the end of every frame, so anything written into that copy is
    /// gone by the time a frame is drawn through it.
    pub fn camera_mut(&mut self) -> &mut Camera {
        self.document.camera_mut()
    }

    /// Show everything this frame draws, and collect what it asks for.
    ///
    /// Reads the document and writes only the inbox. Three sources of intent —
    /// the keyboard, the view, and the overlay's own controls — and none of
    /// them is allowed to act on what it asks.
    fn ask(&mut self, ui: &mut Ui) {
        // Polled unconditionally rather than short-circuited: reading a chord
        // is also what subscribes it for the wake that delivers the next one,
        // so one left unread on the frame the other fired would stop waking a
        // frame of its own.
        if ui.key_pressed(UNDO) {
            self.intents.push(Intent::Undo);
        }
        if ui.key_pressed(REDO) {
            self.intents.push(Intent::Redo);
        }
        self.view.ask(ui, &self.document, &mut self.intents);
        // Formatted straight into the pass's own text arena — no `String` is
        // built on the way, and the handle is lowered by the same pass that
        // minted it, which is the only pass it is good for.
        let status = ui.fmt(format_args!("{}", self.status()));
        overlay::ask(
            ui,
            status,
            self.document.camera().projection,
            &mut self.intents,
        );
    }

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn status(&self) -> Status {
        let drawing = self.document.drawing();
        Status {
            report: drawing.report(),
            degrees_of_freedom: drawing.freedoms().degrees_of_freedom(),
            redundant_equations: drawing.freedoms().redundant_equations(),
            hovered: self.view.hovered(),
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
struct Status {
    report: SolveReport,
    /// Read off the freedoms rather than the report, because they are what the
    /// sketch can still do — where the report is only how the last run went.
    degrees_of_freedom: usize,
    redundant_equations: usize,
    hovered: Option<Named>,
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.report.converged {
            "solved"
        } else {
            "unsolved"
        };
        write!(
            f,
            "{state} · {} dof · {} redundant · {} iterations",
            self.degrees_of_freedom, self.redundant_equations, self.report.iterations,
        )?;
        match self.hovered {
            Some(named) => write!(f, " · {}", named.noun()),
            None => Ok(()),
        }
    }
}

impl App for CatCad {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        // Emptied once a *pass*, not once a frame. A frame that settles records
        // twice, and palantir drains the input queues between the two — so the
        // second pass is a fresh reading of what is still latched, and it has to
        // start from an empty inbox rather than adding to the first's. Each pass
        // asks, applies and settles whole, which is what makes the pair
        // harmless: a drag names where it wants to be rather than how far to
        // travel, and an orbit measures against the total the last pass already
        // took, so re-asking on the second pass turns nothing.
        self.intents.clear();
        Panel::zstack()
            .auto_id()
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                // Ask, apply, settle — the whole of a frame, and the order the
                // rest of the crate is built around: nothing writes the document
                // until everything has finished reading it.
                self.ask(ui);
                self.history
                    .apply(&mut self.document, &mut self.solver, &self.intents);
                self.view.settle(&self.document);
            });
    }
}

#[cfg(test)]
mod tests;
