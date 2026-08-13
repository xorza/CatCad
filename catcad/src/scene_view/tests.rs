use super::*;
use crate::demo;
use palantir::internals::UiHarness;

const SIZE: UVec2 = UVec2::new(800, 600);

/// The demo, as the application raises it.
#[derive(Debug)]
struct Raised {
    document: Document,
    view: SceneView,
    harness: UiHarness,
}

impl Raised {
    fn new() -> Self {
        let mut document = demo::document();
        let mut scene = document.raise();
        document.frame(&scene);
        scene.camera = document.camera();
        Self {
            document,
            view: SceneView::new(scene),
            harness: UiHarness::new(SIZE),
        }
    }

    /// One frame, in the order the application records one: the view first, and
    /// then the camera it settled on handed to the renderer.
    fn frame(&mut self) {
        let Self {
            document,
            view,
            harness,
        } = self;
        harness.frame(|ui| {
            view.show(ui, document);
            view.aim(document);
        });
    }

    /// A cursor position that lands on something the drawing will let go of.
    fn over_draggable(&self) -> Option<Vec2> {
        self.sweep(|grip| grip.is_some())
    }

    /// A cursor position that lands on something it will not — the demo pins a
    /// point, and pressing one has to orbit like any other miss.
    fn over_pinned(&self) -> Option<Vec2> {
        self.sweep(|grip| grip.is_none())
    }

    /// A cursor position that lands on a grip of the given kind.
    fn over(&self, want: fn(Grip) -> bool) -> Option<Vec2> {
        self.sweep(move |grip| grip.is_some_and(want))
    }

    /// The first cursor of a coarse sweep whose hit satisfies `keep`, asked of
    /// the very scene the view picks against.
    fn sweep(&self, keep: impl Fn(Option<Grip>) -> bool) -> Option<Vec2> {
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
                    .is_some_and(|hit| keep(self.document.drawing().grip(&hit)))
            })
    }

    fn camera(&self) -> aperture::Camera {
        self.document.camera()
    }

    /// Where a world position lands on screen — the cursor that aims at it.
    fn cursor_on(&self, world: Vec3) -> Vec2 {
        let viewport = Viewport::new(SIZE);
        let clip = self.camera().view_proj(viewport.aspect()) * world.extend(1.0);
        viewport.pixel_from_clip(clip)
    }

    /// The far end of the demo's arm, which is the freest thing it draws. The
    /// arm's points are added last, so the wrist is drawn last of all.
    fn wrist(&self) -> Vec3 {
        *self.markers().last().expect("the demo draws markers")
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

/// Pressing on something and moving takes it with the pointer, and leaves the
/// camera alone.
///
/// What this pins is the wiring — press, resolve, edit, redraw, release — and
/// not which geometry ends up where; that is the drawing's own business and
/// tested against a fixture there. It aims at the arm rather than sweeping for
/// the first grip, because the demo's frame is fully determined and a drag on
/// determined geometry is refused outright.
#[test]
fn dragging_a_point_moves_it_and_not_the_camera() {
    let mut raised = Raised::new();
    raised.frame();
    let cursor = raised.cursor_on(raised.wrist());

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

/// Dragging determined geometry moves nothing, and leaves nothing behind for
/// a later drag to undo.
///
/// The whole of a reported bug: dragging a rectangle corner deformed the
/// rectangle, because the solver answers an impossible request with a
/// least-squares compromise. That compromise was held together only by what
/// the drag pinned, so dragging *anything else* afterwards let go of it and
/// the rectangle sprang back — deform under one drag, snap on the next. Both
/// halves are checked here, in the order that produced them.
#[test]
fn a_drag_the_constraints_forbid_moves_nothing_and_leaves_nothing_behind() {
    let mut raised = Raised::new();
    raised.frame();
    let at_rest = raised.markers();
    assert!(
        raised.document.drawing().report().converged,
        "the demo has to open solved for this to mean anything"
    );

    // A rectangle corner: determined by its constraints, so there is nowhere
    // for it to go. Not the fixed one — this is refused for being impossible,
    // not for being pinned.
    let corner = raised.cursor_on(at_rest[2]);
    raised.harness.move_to(corner);
    raised.frame();
    raised.harness.press_at(corner);
    raised.frame();
    raised.harness.drag_to(corner + Vec2::new(60.0, 40.0));
    raised.frame();

    assert_eq!(
        raised.markers(),
        at_rest,
        "a drag the constraints forbid deformed the drawing"
    );
    assert!(
        raised.document.drawing().report().converged,
        "a refused drag left the drawing unsolved"
    );
    raised.harness.release();
    raised.frame();

    // Now drag the arm, which does have somewhere to go. Nothing the first
    // drag touched may spring back, because the first drag touched nothing.
    let wrist = raised.cursor_on(raised.wrist());
    raised.harness.move_to(wrist);
    raised.frame();
    raised.harness.press_at(wrist);
    raised.frame();
    raised.harness.drag_to(wrist + Vec2::new(30.0, 20.0));
    raised.frame();

    let now = raised.markers();
    assert_ne!(now, at_rest, "the arm would not move either");
    // The rectangle and the circle's hub — everything the arm is not — stand
    // where they did. Within a tolerance, because a real solve ran: the corners
    // come back to the same answer through different arithmetic, and land a few
    // parts in 10^15 apart doing it.
    assert!(
        settled(&now[..5], &at_rest[..5]),
        "dragging the linkage moved the rectangle: {:?} against {:?}",
        &now[..5],
        &at_rest[..5]
    );
}

/// Whether two sets of positions agree to far below anything drawable.
fn settled(now: &[Vec3], was: &[Vec3]) -> bool {
    now.len() == was.len() && now.iter().zip(was).all(|(a, b)| a.abs_diff_eq(*b, 1e-6))
}

/// Every kind of grip is reachable through the real pick path, not only
/// constructible.
///
/// What a press lands on has to carry the `HitAt` that tells a slide from a
/// resize, and only a real hit carries one — the drawing's own tests build
/// those by hand, so this is what says the two agree.
#[test]
fn the_view_can_take_hold_of_a_point_an_edge_and_a_rim() {
    let mut raised = Raised::new();
    raised.frame();

    assert!(
        raised.over(|grip| matches!(grip, Grip::Point(_))).is_some(),
        "no cursor found a point to move"
    );
    // The rectangle's fixed corner rules out the two edges that meet it, so
    // this finds one of the others — or the linkage's own.
    assert!(
        raised
            .over(|grip| matches!(grip, Grip::Segment { .. }))
            .is_some(),
        "no cursor found an edge to slide"
    );
    assert!(
        raised.over(|grip| matches!(grip, Grip::Rim(_))).is_some(),
        "no cursor found a rim to resize"
    );
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
