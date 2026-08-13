use super::*;
use crate::demo::Demo;
use palantir::internals::UiHarness;

fn demo_view() -> SceneView {
    let demo = Demo::build();
    SceneView::new(demo.scene, demo.names)
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
    const SIZE: UVec2 = UVec2::new(800, 600);

    let mut harness = UiHarness::new(SIZE);
    let mut view = demo_view();
    // Arranges the view, so there is something for the pointer to be over.
    harness.frame(|ui| view.show(ui));

    // A pixel that lands on the drawing, asked of the very scene the view will
    // pick against — so what this measures is the wiring, not the geometry.
    let (cursor, tag) = {
        let renderer = view.renderer().borrow();
        let viewport = Viewport::new(SIZE);
        (0..SIZE.y)
            .step_by(8)
            .flat_map(|y| {
                (0..SIZE.x)
                    .step_by(8)
                    .map(move |x| Vec2::new(x as f32, y as f32))
            })
            .find_map(|cursor| {
                let hit = renderer.scene().nearest(cursor, viewport, HOVER_REACH)?;
                Some((cursor, hit.tag))
            })
            .expect("the demo drawing covers some pixel of an 800×600 view")
    };

    // Entering the view changes the hover target, which wakes a frame by
    // itself — so the one that proves anything is the next, wholly inside.
    harness.move_to(cursor);
    harness.frame(|ui| view.show(ui));
    let delta = harness.move_to(cursor + Vec2::splat(2.0));
    assert!(
        delta.requests_repaint,
        "a move inside the view left the frame asleep, so the highlight would go stale"
    );

    // And the frame that move asks for is the one that lights the primitive.
    harness.move_to(cursor);
    harness.frame(|ui| view.show(ui));
    assert_eq!(
        view.hovered(),
        view.names.get(tag),
        "the pick reported {tag:?} but the view hovered {:?}",
        view.hovered()
    );
    assert!(
        view.hovered().is_some(),
        "aimed at the drawing and lit nothing"
    );

    // Off the drawing entirely, nothing stays lit.
    harness.move_to(Vec2::new(SIZE.x as f32 - 1.0, SIZE.y as f32 - 1.0));
    harness.frame(|ui| view.show(ui));
    assert_eq!(view.hovered(), None);
}

/// The projection is the view's to hold, and reading it back has to answer
/// with what was set — the overlay's toggle is a round trip through these two.
#[test]
fn the_projection_round_trips_through_the_view() {
    let mut view = demo_view();
    let first = view.projection();

    view.set_projection(first.toggled());
    assert_eq!(view.projection(), first.toggled());
    assert_ne!(view.projection(), first, "toggling changed nothing");

    view.set_projection(first);
    assert_eq!(view.projection(), first);
}
