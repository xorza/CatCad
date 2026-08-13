//! The 3D view, and everything the pointer does to it.

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Highlight, Lit, Projection, Renderer, Scene, Viewport};
use glam::{UVec2, Vec2, Vec3};
use palantir::{Configure, GpuPaint, GpuView, PointerWake, Response, Sense, Sizing, Ui};

use crate::named::{Named, Names};

/// Radians of orbit per logical pixel of drag.
const ORBIT_RATE: f32 = 0.008;

/// Distance multiplier per wheel notch.
const ZOOM_RATE: f32 = 1.12;

/// How far from the cursor a thing may be and still count as under it, in
/// logical pixels. Wider than the strokes, because aiming is not precise and
/// a stroke a pixel and a half wide is not a target.
pub(crate) const HOVER_REACH: f32 = 6.0;

/// Kept in step with `sketch_plane`'s ladder: the highlight has to beat the
/// markers, which already beat the strokes.
const MARKER_LIFT_STEP: i32 = 2048;

/// What the thing under the cursor looks like. One step of lift above the
/// markers, which are already the top of the drawing's own ladder.
const HOVERED: Highlight = Highlight {
    color: Vec3::new(1.0, 0.85, 0.25),
    scale: 1.8,
    lift: MARKER_LIFT_STEP,
};

/// One scene, drawn into a viewport the pointer can orbit, zoom and aim at.
///
/// Owns everything the pointer's state is made of, so the app above it holds
/// a view rather than a renderer plus the three loose fields that were only
/// ever about pointing at one.
#[derive(Debug)]
pub(crate) struct SceneView {
    renderer: Rc<RefCell<Renderer>>,
    /// Drag deltas arrive as cumulative travel, so the previous total is
    /// subtracted to recover this frame's movement.
    drag_travel: Vec2,
    /// What each drawn primitive's tag stands for, built with the drawing.
    names: Names,
    /// The sketch entity under the pointer, if any.
    hovered: Option<Named>,
}

impl SceneView {
    /// A view of `scene`, whose tags `names` can report.
    pub(crate) fn new(scene: Scene, names: Names) -> Self {
        Self {
            renderer: Rc::new(RefCell::new(Renderer::new(scene))),
            drag_travel: Vec2::ZERO,
            names,
            hovered: None,
        }
    }

    /// The renderer being drawn, for a caller that wants to edit the scene or
    /// move the camera without going through a pointer.
    pub(crate) fn renderer(&self) -> &Rc<RefCell<Renderer>> {
        &self.renderer
    }

    /// The sketch entity under the pointer, if any.
    pub(crate) fn hovered(&self) -> Option<Named> {
        self.hovered
    }

    pub(crate) fn projection(&self) -> Projection {
        self.renderer.borrow().camera().projection
    }

    pub(crate) fn set_projection(&mut self, projection: Projection) {
        self.renderer.borrow_mut().camera_mut().projection = projection;
    }

    /// Show the view, and let the pointer over it orbit, zoom and light what
    /// it is aimed at.
    pub(crate) fn show(&mut self, ui: &mut Ui) {
        // A bare pointer move only wakes a frame for a widget that asked for
        // one: palantir skips a `PointerMoved` that crosses no boundary and
        // latches no press, and a viewport filling the window has no boundary
        // to cross. Without this the highlight below is computed once on the
        // way in and then sits stale until an unrelated event forces a frame.
        ui.watch_pointer(PointerWake::MOVE);

        let paint: Rc<RefCell<dyn GpuPaint>> = self.renderer.clone();
        let response = GpuView::new(paint)
            .auto_id()
            .sense(Sense::DRAG | Sense::SCROLL)
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui);

        match response.left.drag.delta() {
            Some(travel) => {
                let step = travel - self.drag_travel;
                self.drag_travel = travel;
                // Dragging right turns the model right, which means orbiting
                // the camera the other way.
                self.renderer
                    .borrow_mut()
                    .camera_mut()
                    .orbit(-step.x * ORBIT_RATE, step.y * ORBIT_RATE);
            }
            None => self.drag_travel = Vec2::ZERO,
        }

        self.hover(&response);

        let notches = response.scroll.lines.y;
        if notches != 0.0 {
            self.renderer
                .borrow_mut()
                .camera_mut()
                .dolly(ZOOM_RATE.powf(-notches));
        }
    }

    /// Light whatever the pointer is over.
    ///
    /// `pointer_local` is already what [`Scene::pick`] asks for — logical
    /// pixels from the widget's own top-left — so nothing is converted here.
    /// It is measured against `layout_rect` rather than the visible `rect`,
    /// and the viewport has to be built from the same one or the two would
    /// disagree the moment anything clipped the view.
    ///
    /// Only the nearest hit lights: a marker sits on the end of every edge
    /// that meets it, and lighting all of them would answer a question nobody
    /// asked. `pick` has already put the most specific first.
    fn hover(&mut self, response: &Response<'_>) {
        let under = response
            .pointer_local
            .zip(response.layout_rect)
            // `pointer_local` is the offset from this widget's corner wherever
            // the pointer is, including well off the widget — so asking
            // whether it is actually over the view is what stops the overlay's
            // own controls from lighting the drawing behind them.
            .filter(|_| response.hovered)
            .and_then(|(cursor, rect)| {
                let viewport = Viewport::new(UVec2::new(rect.size.w as u32, rect.size.h as u32));
                let renderer = self.renderer.borrow();
                let hit = renderer.scene().pick(cursor, viewport, HOVER_REACH);
                hit.first().map(|hit| hit.tag)
            });

        self.hovered = under.and_then(|tag| self.names.get(tag));
        self.renderer
            .borrow_mut()
            .highlight_only(under.map(|tag| Lit { tag, look: HOVERED }));
    }
}

#[cfg(test)]
mod tests;
