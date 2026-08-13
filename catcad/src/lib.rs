//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

#[cfg(feature = "bench")]
mod bench;
mod demo;
mod drawing;
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

use aperture::Renderer;
use palantir::{App, Configure, HostHandle, Panel, Sizing, Ui, WindowToken};
use silverpoint::SolveReport;

use crate::demo::Demo;
use crate::drawing::Drawing;
use crate::named::Named;
use crate::scene_view::SceneView;

/// One view of one scene, with the controls and the solve's verdict laid over
/// it.
#[derive(Debug)]
pub struct CatCad {
    /// The model: the sketch, where it lies, and the solver that keeps it
    /// satisfied as the pointer edits it.
    drawing: Drawing,
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
        let demo = Demo::build();
        Self {
            drawing: demo.drawing,
            view: SceneView::new(demo.scene),
        }
    }

    /// The renderer behind the view, so a harness can reach the scene and the
    /// camera without a pointer to drive them with.
    pub fn renderer(&self) -> &Rc<RefCell<Renderer>> {
        self.view.renderer()
    }

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn status(&self) -> Status {
        Status {
            report: self.drawing.report(),
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
        Panel::zstack()
            .auto_id()
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                self.view.show(ui, &mut self.drawing);
                // Formatted straight into the pass's own text arena — no
                // `String` is built on the way, and the handle is lowered by
                // the same pass that minted it, which is the only pass it is
                // good for.
                let status = ui.fmt(format_args!("{}", self.status()));
                let asked = overlay::show(ui, status, self.view.projection());
                self.view.set_projection(asked);
            });
    }
}

#[cfg(test)]
mod tests;
