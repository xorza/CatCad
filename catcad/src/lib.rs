//! A parametric CAD application: constrained sketches, solved, and drawn.
//!
//! The binary beside this is a `WinitHost` call and nothing else, so the whole
//! of the app is reachable from a test — which is what lets the visual suite
//! raise the real thing rather than a stand-in for it.

#[cfg(feature = "bench")]
mod bench;
mod demo;
pub mod named;
mod overlay;
mod scene_view;
pub mod sketch_plane;

/// The one call `benches/alloc.rs` makes. The driver itself stays in `src/`,
/// where it can reach what it measures.
#[cfg(feature = "bench")]
pub use bench::alloc_bench;

use std::cell::RefCell;
use std::rc::Rc;

use aperture::Renderer;
use palantir::{App, Configure, HostHandle, Panel, Sizing, Ui, WindowToken};
use silverpoint::SolveReport;

use crate::demo::Demo;
use crate::scene_view::SceneView;

/// One view of one scene, with the controls and the solve's verdict laid over
/// it.
#[derive(Debug)]
pub struct CatCad {
    view: SceneView,
    /// What the startup solve made of the sketch now in the scene. Nothing
    /// edits it yet, so re-solving per frame would only recompute the same
    /// answer.
    report: SolveReport,
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
            view: SceneView::new(demo.scene, demo.names),
            report: demo.report,
        }
    }

    /// The renderer behind the view, so a harness can reach the scene and the
    /// camera without a pointer to drive them with.
    pub fn renderer(&self) -> &Rc<RefCell<Renderer>> {
        self.view.renderer()
    }

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn status(&self) -> String {
        let state = if self.report.converged {
            "solved"
        } else {
            "unsolved"
        };
        let under = match self.view.hovered() {
            Some(named) => format!(" · {}", named.noun()),
            None => String::new(),
        };
        format!(
            "{state} · {} dof · {} redundant · {} iterations{under}",
            self.report.degrees_of_freedom, self.report.redundant_equations, self.report.iterations,
        )
    }
}

impl App for CatCad {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        Panel::zstack()
            .auto_id()
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                self.view.show(ui);
                let asked = overlay::show(ui, &self.status(), self.view.projection());
                self.view.set_projection(asked);
            });
    }
}

#[cfg(test)]
mod tests;
