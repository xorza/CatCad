//! CatCad application entry point.

mod sketch_plane;

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Mesh, Object, Projection, Renderer, Scene};
use glam::{DVec2, Mat4, Vec2, Vec3};
use palantir::{
    Align, App, Background, Button, Configure, GpuPaint, GpuView, HostHandle, Panel, Sense, Sizing,
    Text, Ui, WindowToken, WinitHost, WinitHostError,
};
use silverpoint::{Constraint, Sketch, SolveReport, Solver};

use crate::sketch_plane::SketchPlane;

/// Radians of orbit per logical pixel of drag.
const ORBIT_RATE: f32 = 0.008;

/// Distance multiplier per wheel notch.
const ZOOM_RATE: f32 = 1.12;

#[derive(Debug)]
struct CatCad {
    view: Rc<RefCell<Renderer>>,
    /// Drag deltas arrive as cumulative travel, so the previous total is
    /// subtracted to recover this frame's movement.
    drag_travel: Vec2,
    /// What the startup solve made of the sketch now in the scene. Nothing
    /// edits it yet, so re-solving per frame would only recompute the same
    /// answer.
    report: SolveReport,
}

impl CatCad {
    /// What `WinitHost` calls. The host's arguments say nothing this app
    /// needs, so the work is all in [`CatCad::build`] — which is also what
    /// lets the offscreen harness raise the same app without a window.
    fn new(_ui: &mut Ui, _handle: HostHandle<Self>) -> Self {
        Self::build()
    }

    fn build() -> Self {
        let mut sketch = demo_sketch();
        let report = Solver::default().solve(&mut sketch);

        // The sketch and the solids share one world: the drawing lies on the
        // ground plane and the boxes stand on it, so orbiting the view moves
        // both together.
        let plane = SketchPlane::GROUND;
        let mut scene = Scene {
            curves: plane.curves(&sketch),
            ..Default::default()
        };
        // The ground the drawing lies on, and the reason the drawing carries a
        // depth bias at all: the slab's top face *is* the sketch plane, so the
        // two are exactly coplanar and something has to decide which reads.
        scene.objects.push(Object {
            mesh: Mesh::cube(1.0),
            // A unit cube spans ±0.5, so dropping the slab by half its
            // thickness after scaling lands its top face on y = 0.
            transform: Mat4::from_translation(Vec3::new(4.0, -0.5, -2.5))
                * Mat4::from_scale(Vec3::new(12.0, 1.0, 9.0)),
            color: Vec3::new(0.30, 0.30, 0.34),
            z_offset: 0,
        });
        for (size, at, color) in [
            (2.0, DVec2::new(2.0, 3.6), Vec3::new(0.55, 0.58, 0.62)),
            (0.8, DVec2::new(6.2, 1.1), Vec3::new(0.85, 0.35, 0.20)),
            (1.2, DVec2::new(6.6, 3.9), Vec3::new(0.25, 0.45, 0.75)),
        ] {
            // Half a cube up puts it on the plane rather than through it.
            let base = plane.point(at) + Vec3::Y * size * 0.5;
            scene
                .objects
                .push(Object::new(Mesh::cube(size)).at(base).colored(color));
        }
        if let Some(bounds) = scene.bounds() {
            scene.camera.frame(bounds);
        }

        Self {
            view: Rc::new(RefCell::new(Renderer::new(scene))),
            drag_travel: Vec2::ZERO,
            report,
        }
    }

    /// A sketch is only as useful as it is determined, so the report reads
    /// over the drawing rather than into a log.
    fn solve_summary(&self) -> String {
        let state = if self.report.converged {
            "solved"
        } else {
            "unsolved"
        };
        format!(
            "{state} · {} dof · {} redundant · {} iterations",
            self.report.degrees_of_freedom, self.report.redundant_equations, self.report.iterations,
        )
    }

    /// The 3D viewport, orbited and zoomed by the pointer over it.
    fn viewport(&mut self, ui: &mut Ui) {
        let paint: Rc<RefCell<dyn GpuPaint>> = self.view.clone();
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
                self.view
                    .borrow_mut()
                    .camera_mut()
                    .orbit(-step.x * ORBIT_RATE, step.y * ORBIT_RATE);
            }
            None => self.drag_travel = Vec2::ZERO,
        }

        let notches = response.scroll.lines.y;
        if notches != 0.0 {
            self.view
                .borrow_mut()
                .camera_mut()
                .dolly(ZOOM_RATE.powf(-notches));
        }
    }

    /// What floats over the viewport, pinned to its top-left corner.
    fn overlay(&mut self, ui: &mut Ui) {
        Panel::vstack()
            .auto_id()
            // Chrome would put a slab of theme colour over the drawing; the
            // overlay is meant to sit *on* the view, not box it off.
            .background(Background::NONE)
            .size((Sizing::HUG, Sizing::HUG))
            .align(Align::TOP_LEFT)
            .padding(12.0)
            .gap(8.0)
            .show(ui, |ui| {
                self.projection_toggle(ui);
                Text::new(self.solve_summary()).auto_id().show(ui);
            });
    }

    /// Flips the camera between the two projections.
    ///
    /// Labelled with the projection it is on rather than the one it would
    /// switch to: the button has to answer "which am I looking at?" every
    /// frame, and only answers "what happens if I press this?" once.
    fn projection_toggle(&mut self, ui: &mut Ui) {
        let projection = self.view.borrow().camera().projection;
        let label = match projection {
            Projection::Perspective => "Perspective",
            Projection::Orthographic => "Orthographic",
        };
        if Button::new().auto_id().label(label).show(ui).left.clicked() {
            self.view.borrow_mut().camera_mut().projection = projection.toggled();
        }
    }
}

impl App for CatCad {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
        // One view of one scene, with the controls and the solve's verdict
        // laid over it.
        Panel::zstack()
            .auto_id()
            .size((Sizing::FILL, Sizing::FILL))
            .show(ui, |ui| {
                self.viewport(ui);
                self.overlay(ui);
            });
    }
}

/// A rectangle anchored at the origin with a circle at its centre. Every
/// position below is a guess deliberately off the answer: what puts the
/// geometry where it belongs is the solve, not the coordinates.
fn demo_sketch() -> Sketch {
    const WIDTH: f64 = 8.0;
    const HEIGHT: f64 = 5.0;

    let mut sketch = Sketch::default();
    let corner = [
        sketch.add_point(DVec2::ZERO),
        sketch.add_point(DVec2::new(7.4, 0.6)),
        sketch.add_point(DVec2::new(8.6, 4.2)),
        sketch.add_point(DVec2::new(-0.5, 5.3)),
    ];
    // Without an anchor the rectangle is still free to slide and turn, and
    // the report would say so: three degrees of freedom left over.
    sketch.fix(corner[0]);
    for pair in [[0, 1], [1, 2], [2, 3], [3, 0]] {
        sketch.add_segment(corner[pair[0]], corner[pair[1]]);
    }
    sketch.add_constraint(Constraint::Horizontal {
        a: corner[0],
        b: corner[1],
    });
    sketch.add_constraint(Constraint::Vertical {
        a: corner[1],
        b: corner[2],
    });
    sketch.add_constraint(Constraint::Horizontal {
        a: corner[2],
        b: corner[3],
    });
    sketch.add_constraint(Constraint::Vertical {
        a: corner[3],
        b: corner[0],
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[0],
        b: corner[1],
        distance: WIDTH,
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[1],
        b: corner[2],
        distance: HEIGHT,
    });

    let hub = sketch.add_point(DVec2::new(3.6, 2.1));
    let hole = sketch.add_circle(hub, 0.9);
    // Both bottom corners sit half a diagonal from the centre. That leaves a
    // mirrored solution below the edge, which the guess above declines.
    let to_centre = (WIDTH * WIDTH + HEIGHT * HEIGHT).sqrt() * 0.5;
    sketch.add_constraint(Constraint::Distance {
        a: corner[0],
        b: hub,
        distance: to_centre,
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[1],
        b: hub,
        distance: to_centre,
    });
    sketch.add_constraint(Constraint::Radius {
        circle: hole,
        radius: 1.5,
    });
    sketch
}

fn main() -> Result<(), WinitHostError> {
    WinitHost::builder(WindowToken(0))
        .title("CatCad")
        .build(CatCad::new)?
        .run()
}

#[cfg(test)]
mod tests;
