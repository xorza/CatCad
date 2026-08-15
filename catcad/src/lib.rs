//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

#[cfg(feature = "bench")]
mod bench;
mod build;
mod demo;
mod document;
mod drawing;
mod history;
mod hud;
mod intent;
mod model;
mod names;
mod paint;
mod part;
mod preview;
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

use palantir::{App, Configure, HostHandle, Key, Panel, Shortcut, Sizing, Ui, WindowToken};
use silverpoint::{Entity, Removed};

use crate::build::Build;
use crate::document::Document;
use crate::history::History;
use crate::hud::{Hud, Shown};
use crate::intent::{Change, Choice, Intents, Step};
use crate::part::Part;
use crate::scene_view::SceneView;
use crate::selection::Selection;
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
/// A bare key rather than a chord, which is what every modeller binds it to —
/// and safe to bind bare because nothing here takes typed text yet. That changes
/// the moment a dimension can be retyped, and this is the binding that will have
/// to answer for it.
const DELETE: Shortcut = Shortcut::key(Key::Delete);

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
        let mut view = SceneView::new(&document, &build);
        if let Some(bounds) = view.bounds() {
            document.frame(bounds);
        }
        // Aiming the camera is not something the view was watching for, so it
        // is handed on here rather than left to the first frame. Not for the
        // sake of the painting — a recorded frame settles before its paint
        // command runs, so the first one would hand it on in time either way —
        // but so that what `build` returns already agrees with itself, and a
        // caller can measure the view it was given without recording a frame to
        // make the answer true.
        view.settle(&document, &build, &Selection::default());
        Self {
            document,
            history: History::default(),
            intents: Intents::default(),
            build,
            view,
            session: Session::default(),
            hud: Hud::default(),
        }
    }

    /// Show everything this frame draws, and collect what it asks for.
    ///
    /// Reads the document and writes only the inbox and the tool in hand. Three
    /// sources of intent — the keyboard, the view, and the overlay's own
    /// controls — and none of them is allowed to act on what it asks.
    fn ask(&mut self, ui: &mut Ui) {
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
            for entity in self
                .session
                .selection()
                .picked()
                .iter()
                .filter_map(|part| part.entity())
            {
                self.intents.push(Change::Delete(entity));
            }
        }
        self.view
            .ask(ui, &self.document, self.session.tool(), &mut self.intents);
        // Formatted straight into the pass's own text arena — no `String` is
        // built on the way, and the handle is lowered by the same pass that
        // minted it, which is the only pass it is good for.
        let status = ui.fmt(format_args!("{}", self.status()));
        // Last, so what floats over the view is the topmost thing in the zstack
        // and takes its own presses rather than the view beneath it.
        self.hud.show(
            ui,
            Shown {
                tool: self.session.tool(),
                status,
                projection: self.document.camera().projection,
                drawing: self.document.drawing(),
                selection: self.session.selection(),
            },
            &mut self.intents,
        );
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
        self.session.apply(&self.intents);
        self.history
            .apply(&mut self.document, &mut self.build, &self.intents);
        // Last, because an undo can take geometry the session was still holding
        // on to — see [`Session::prune`].
        self.session.prune(self.document.model(&self.build));
    }

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn status(&self) -> Status {
        let model = self.document.model(&self.build);
        let outcome = model.outcome();
        Status {
            converged: outcome.converged(),
            iterations: outcome.iterations(),
            degrees_of_freedom: outcome.degrees_of_freedom(),
            redundant_constraints: outcome.redundant_constraints(),
            hovered: self.view.hovered(),
            cleaned: self.build.cleaned(),
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
    converged: bool,
    iterations: u32,
    /// What the sketch can still do, where the two above are only how the last
    /// run getting it there went.
    degrees_of_freedom: usize,
    redundant_constraints: usize,
    hovered: Option<Part>,
    /// What the last cleanup took out, where that was the last thing done.
    ///
    /// Three states rather than two counts: nothing to say, a cleanup that
    /// found nothing, and a cleanup that took something. The middle one is the
    /// reason this is an `Option` — a command that answers a press with silence
    /// reads as a command that did not work.
    cleaned: Option<Removed>,
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
        Part::Face { .. } => "face",
    }
}

impl fmt::Display for Status {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let state = if self.converged { "solved" } else { "unsolved" };
        write!(
            f,
            "{state} · {} dof · {} redundant · {} iterations",
            self.degrees_of_freedom, self.redundant_constraints, self.iterations,
        )?;
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
                self.apply();
                self.view
                    .settle(&self.document, &self.build, self.session.selection());
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
    }
}

#[cfg(test)]
mod tests;
