use super::*;
use crate::demo::Demo;
use palantir::internals::UiHarness;

const SIZE: UVec2 = UVec2::new(800, 600);

/// The demo, as the application raises it.
#[derive(Debug)]
struct Raised {
    drawing: Drawing,
    view: SceneView,
    harness: UiHarness,
}

impl Raised {
    fn new() -> Self {
        let demo = Demo::build();
        Self {
            drawing: demo.drawing,
            view: SceneView::new(demo.scene),
            harness: UiHarness::new(SIZE),
        }
    }

    fn frame(&mut self) {
        let Self {
            drawing,
            view,
            harness,
        } = self;
        harness.frame(|ui| view.show(ui, drawing));
    }

    /// A cursor position that lands on something the drawing will let go of.
    fn over_draggable(&self) -> Option<Vec2> {
        self.sweep(|motion| motion.is_some())
    }

    /// A cursor position that lands on something it will not — the demo pins a
    /// point, and pressing one has to orbit like any other miss.
    fn over_pinned(&self) -> Option<Vec2> {
        self.sweep(|motion| motion.is_none())
    }

    /// The first cursor of a coarse sweep whose hit satisfies `keep`, asked of
    /// the very scene the view picks against.
    fn sweep(&self, keep: impl Fn(Option<Motion>) -> bool) -> Option<Vec2> {
        let renderer = self.view.renderer().borrow();
        let viewport = Viewport::new(SIZE);
        (0..SIZE.y)
            .step_by(4)
            .flat_map(|y| {
                (0..SIZE.x)
                    .step_by(4)
                    .map(move |x| Vec2::new(x as f32, y as f32))
            })
            .find(|&cursor| {
                renderer
                    .scene()
                    .nearest(cursor, viewport, HOVER_REACH)
                    .and_then(|hit| self.drawing.resolve(hit.tag))
                    .is_some_and(|entity| keep(self.drawing.motion_of(entity)))
            })
    }

    fn camera(&self) -> aperture::Camera {
        *self.view.renderer().borrow().camera()
    }

    /// Where every marker in the scene sits, in the order they are drawn.
    fn markers(&self) -> Vec<Vec3> {
        self.view
            .renderer()
            .borrow()
            .scene()
            .points
            .iter()
            .map(|point| point.position)
            .collect()
    }
}

/// The pointer moving *within* the view has to wake a frame, and what it lands
/// on has to reach `hovered`.
///
/// Palantir drops a `PointerMoved` that crosses no widget boundary and latches
/// no press, so a view filling the window sees none of them unless it watches
/// for them — and a highlight computed on the way in then sits stale on screen
/// until an unrelated event forces a frame. That is the whole of what this
/// pins: the move inside, not the one that enters.
#[test]
fn a_move_inside_the_view_wakes_a_frame_and_lights_what_it_lands_on() {
    let mut raised = Raised::new();
    // Arranges the view, so there is something for the pointer to be over.
    raised.frame();

    let cursor = raised
        .over_draggable()
        .expect("the demo draws something to grab");

    // Entering the view changes the hover target, which wakes a frame by
    // itself — so the one that proves anything is the next, wholly inside.
    raised.harness.move_to(cursor);
    raised.frame();
    let delta = raised.harness.move_to(cursor + Vec2::splat(2.0));
    assert!(
        delta.requests_repaint,
        "a move inside the view left the frame asleep, so the highlight would go stale"
    );

    // And the frame that move asks for is the one that lights the primitive.
    raised.harness.move_to(cursor);
    raised.frame();
    assert!(
        raised.view.hovered().is_some(),
        "aimed at the drawing and lit nothing"
    );

    // Off the drawing entirely, nothing stays lit.
    raised
        .harness
        .move_to(Vec2::new(SIZE.x as f32 - 1.0, SIZE.y as f32 - 1.0));
    raised.frame();
    assert_eq!(raised.view.hovered(), None);
}

/// Pressing on something draggable and moving takes it with the pointer, and
/// leaves the camera alone.
///
/// What this pins is the wiring — press, resolve, edit, redraw, release — and
/// not which geometry ends up where. The sweep takes the first draggable point
/// in raster order, so what it grabs is whatever the demo happens to draw
/// there; where a drag *puts* things, and what follows it, is the drawing's
/// own business and tested against a fixture there.
#[test]
fn dragging_a_point_moves_it_and_not_the_camera() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised
        .over_draggable()
        .expect("the demo draws a draggable point");

    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    let camera = raised.camera();

    // Past palantir's four-pixel latch, so the drag is live rather than a
    // press that has not travelled.
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    raised.frame();

    assert_ne!(raised.markers(), before, "the drag moved nothing");
    assert_eq!(
        raised.camera(),
        camera,
        "a drag on the drawing turned the camera"
    );

    // Released, the pointer moves over the drawing without moving it — a
    // plain move rather than a drag, since there is no longer a press for one
    // to latch to.
    raised.harness.release();
    raised.frame();
    let settled = raised.markers();
    raised.harness.move_to(cursor + Vec2::new(80.0, 25.0));
    raised.frame();
    assert_eq!(raised.markers(), settled, "the drag outlived its release");
}

/// Pressing where the drawing is not turns the camera, which is the only way
/// the view can be looked around — so a drag has to fall back to it rather
/// than swallow the gesture.
#[test]
fn dragging_off_the_drawing_orbits_and_edits_nothing() {
    let mut raised = Raised::new();
    raised.frame();

    // A corner the demo's geometry comes nowhere near.
    let empty = Vec2::new(4.0, 4.0);
    raised.harness.move_to(empty);
    raised.frame();
    let before = raised.markers();
    let camera = raised.camera();

    raised.harness.press_at(empty);
    raised.frame();
    raised.harness.drag_to(empty + Vec2::new(60.0, 10.0));
    raised.frame();

    assert_ne!(raised.camera(), camera, "the drag did not orbit");
    assert_eq!(raised.markers(), before, "orbiting edited the drawing");
}

/// A point the drawing pins is not draggable, so pressing it orbits like any
/// other miss.
#[test]
fn pressing_a_pinned_point_orbits_rather_than_dragging_it() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised
        .over_pinned()
        .expect("the demo pins a point and draws it");

    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    let camera = raised.camera();

    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(50.0, 0.0));
    raised.frame();

    assert_ne!(raised.camera(), camera, "a press on scenery has to orbit");
    assert_eq!(raised.markers(), before, "a pinned point was dragged");
}

/// The projection is the view's to hold, and reading it back has to answer
/// with what was set — the overlay's toggle is a round trip through these two.
#[test]
fn the_projection_round_trips_through_the_view() {
    let mut raised = Raised::new();
    let first = raised.view.projection();

    raised.view.set_projection(first.toggled());
    assert_eq!(raised.view.projection(), first.toggled());
    assert_ne!(raised.view.projection(), first, "toggling changed nothing");

    raised.view.set_projection(first);
    assert_eq!(raised.view.projection(), first);
}
