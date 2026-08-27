//! The app on a view, and the frames a gate drives it through.

use aperture::Viewport;
use catcad::CatCad;
use glam::{UVec2, Vec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{App, ResponseState, WidgetId, WindowToken};
use std::hint::black_box;

/// The surface every gate records at. Large enough that layout does real work
/// rather than collapsing everything to nothing.
const SURFACE: UVec2 = UVec2::new(1600, 1000);

/// The app raised on a `SURFACE`-sized view, with one frame behind it.
///
/// One frame in, because everything below aims at something the app has drawn:
/// the camera is settled on the way in, and a cursor worked out before that
/// would aim through a camera the app has since replaced.
///
/// A gate takes its own rather than carrying the last one's on, so what each
/// measures is a steady frame of one kind — a window that inherited a latched
/// drag would be measuring two things and reporting one number.
#[derive(Debug)]
pub(crate) struct Raised {
    pub(crate) app: CatCad,
    pub(crate) harness: UiHarness,
}

impl Raised {
    pub(crate) fn new() -> Self {
        let mut raised = Self {
            app: CatCad::build(),
            harness: UiHarness::new(SURFACE),
        };
        // Every gate measures a frame that is *drawing* something, and a
        // document is opened on no sketch.
        raised.app.enter_first_sketch();
        raised.frame();
        raised
    }

    /// One frame, recorded the way the host records one.
    pub(crate) fn frame(&mut self) {
        let Self { app, harness } = self;
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    }

    /// Where a control on the overlay ended up, measured off the frame that
    /// drew it.
    ///
    /// A widget's rect is the layout engine's answer and arrives a frame late,
    /// so this records a frame and reads the *previous* one's placement — which
    /// is why the app is raised with one behind it.
    pub(crate) fn at(&mut self, id: WidgetId) -> Vec2 {
        self.response(id)
            .rect
            .expect("the overlay drew the control asked for")
            .center()
    }

    /// Whether the overlay reports `id` as under the pointer.
    pub(crate) fn hovers(&mut self, id: WidgetId) -> bool {
        self.response(id).hovered
    }

    /// What the overlay last reported about `id`, read out of a fresh frame.
    ///
    /// A rect and a hover are both the layout engine's answer and both arrive a
    /// frame late, so both are read the same way: record a frame, and take what
    /// the *previous* one left.
    fn response(&mut self, id: WidgetId) -> ResponseState {
        let Self { app, harness } = self;
        harness.frame_value(|ui| {
            app.record(WindowToken(0), ui);
            ui.response_for(id)
        })
    }

    /// The cursor that aims at `world`, through the camera the app is looking
    /// with.
    pub(crate) fn cursor_on(&mut self, world: Vec3) -> Vec2 {
        self.app
            .camera_mut()
            .screen_of(world, Viewport::new(SURFACE))
            .expect("the gate aims at what it draws")
    }
}
