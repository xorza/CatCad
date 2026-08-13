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
mod scene_view;
pub mod sketch_plane;

/// The one call `tests/alloc.rs` makes. The driver itself stays in `src/`,
/// where it can reach what it measures.
#[cfg(feature = "bench")]
pub use bench::alloc_bench;

use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

use aperture::{Camera, Renderer};
use palantir::{App, Configure, HostHandle, Panel, Shortcut, Sizing, Ui, WindowToken};
use silverpoint::SolveReport;

use crate::document::Document;
use crate::history::History;
use crate::intent::{Intent, Intents};
use crate::named::{Named, Names};
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
        let mut document = demo::document();
        // Raised before it is framed, because what has to fit on screen is what
        // the document turns into and not the document itself.
        let mut names = Names::default();
        let scene = document.raise(&mut names);
        document.frame(&scene);
        let mut view = SceneView::new(scene, names);
        // Settled once here, so the view's account of what it has drawn agrees
        // with the scene it was handed. Without it the view would be holding a
        // laid-out drawing while believing it had laid nothing out, and would
        // lay it out again on the first frame — harmless, but it would mean the
        // app was never quite consistent until it had drawn once.
        view.settle(&document);
        Self {
            document,
            history: History::default(),
            intents: Intents::default(),
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

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn status(&self) -> Status {
        Status {
            report: self.document.drawing().report(),
            hovered: self.view.hovered(),
        }
    }
}

/// What the status line says: the solve's verdict, and what the pointer is
/// over.
///
/// A `Display` rather than a `String`, for two reasons. Its only caller writes
/// it straight into the record pass's own text arena, and a line rebuilt every
/// frame out of a report that changes only on a solve should not cost an
/// allocation to say so. And a value that can be written to any formatter is
/// one a test can read without raising a `Ui` to do it.
#[derive(Debug)]
struct Status {
    report: SolveReport,
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
            self.report.degrees_of_freedom, self.report.redundant_equations, self.report.iterations,
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
                // Polled unconditionally rather than short-circuited: reading a
                // chord is also what subscribes it for the wake that delivers
                // the next one, so one left unread on the frame the other fired
                // would stop waking a frame of its own.
                if ui.key_pressed(UNDO) {
                    self.intents.push(Intent::Undo);
                }
                if ui.key_pressed(REDO) {
                    self.intents.push(Intent::Redo);
                }
                self.view.show(ui, &self.document, &mut self.intents);
                // Formatted straight into the pass's own text arena — no
                // `String` is built on the way, and the handle is lowered by
                // the same pass that minted it, which is the only pass it is
                // good for.
                let status = ui.fmt(format_args!("{}", self.status()));
                let asked = overlay::show(ui, status, self.document.camera().projection);
                if asked != self.document.camera().projection {
                    self.intents.push(Intent::Project(asked));
                }
                // Everything above only asked. This is where a frame's asking
                // becomes a change, and where what that changed is drawn.
                self.history.apply(&mut self.document, &self.intents);
                self.view.settle(&self.document);
            });
    }
}

#[cfg(test)]
mod tests;
