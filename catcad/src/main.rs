//! CatCad application entry point.

use std::cell::RefCell;
use std::rc::Rc;

use aperture::{Mesh, Object, Renderer, Scene};
use glam::{Vec2, Vec3};
use palantir::{
    App, Configure, GpuPaint, GpuView, HostHandle, Sense, Sizing, Ui, WindowToken, WinitHost,
    WinitHostError,
};

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
}

impl CatCad {
    fn new(_ui: &mut Ui, _handle: HostHandle<Self>) -> Self {
        let mut scene = Scene::default();
        scene
            .objects
            .push(Object::new(Mesh::cube(2.0)).colored(Vec3::new(0.55, 0.58, 0.62)));
        scene.objects.push(
            Object::new(Mesh::cube(0.8))
                .at(Vec3::new(1.8, -0.6, 0.4))
                .colored(Vec3::new(0.85, 0.35, 0.20)),
        );
        scene.objects.push(
            Object::new(Mesh::cube(1.2))
                .at(Vec3::new(-1.6, 0.9, -0.8))
                .colored(Vec3::new(0.25, 0.45, 0.75)),
        );

        Self {
            view: Rc::new(RefCell::new(Renderer::new(scene))),
            drag_travel: Vec2::ZERO,
        }
    }
}

impl App for CatCad {
    fn record(&mut self, _win: WindowToken, ui: &mut Ui) {
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
}

fn main() -> Result<(), WinitHostError> {
    WinitHost::builder(WindowToken(0))
        .title("CatCad")
        .build(CatCad::new)?
        .run()
}
