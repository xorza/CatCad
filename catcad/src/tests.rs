//! What the app decides before anything is drawn.

use aperture::Viewport;
use glam::{DVec2, UVec2, Vec2};
use palantir::internals::UiHarness;
use silverpoint::Solver;

use crate::{CatCad, HOVER_REACH, demo_sketch};

/// The demo is a fixture, so what it solves to is a fact the rest of the suite
/// leans on — the frames below all draw this rectangle — and the report has to
/// agree that nothing is left free.
#[test]
fn the_demo_sketch_solves_to_a_determined_rectangle() {
    let mut sketch = demo_sketch();
    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    assert_eq!(report.degrees_of_freedom, 0, "{report:?}");
    assert_eq!(report.redundant_equations, 0, "{report:?}");

    let corners: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
    let expected = [
        DVec2::ZERO,
        DVec2::new(8.0, 0.0),
        DVec2::new(8.0, 5.0),
        DVec2::new(0.0, 5.0),
        // The circle's centre: mid-width, mid-height.
        DVec2::new(4.0, 2.5),
    ];
    for (found, want) in corners.iter().zip(expected) {
        assert!((*found - want).length() < 1e-9, "{found:?} vs {want:?}");
    }
    assert_eq!(sketch.circles().next().unwrap().1.radius, 1.5);
}

/// The pointer moving *within* the viewport has to wake a frame, and what it
/// lands on has to reach `hovered`.
///
/// Palantir drops a `PointerMoved` that crosses no widget boundary and latches
/// no press, so a viewport filling the window sees none of them unless it
/// watches for them — and a highlight computed on the way in then sits stale
/// on screen until an unrelated event forces a frame. That is the whole of
/// what this pins: the move inside, not the one that enters.
#[test]
fn a_move_inside_the_viewport_wakes_a_frame_and_lights_what_it_lands_on() {
    const SIZE: UVec2 = UVec2::new(800, 600);

    let mut harness = UiHarness::new(SIZE);
    let mut app = CatCad::build();
    // Arranges the view, so there is something for the pointer to be over.
    harness.frame(|ui| app.viewport(ui));

    // A pixel that lands on the drawing, asked of the very scene the app will
    // pick against — so what this measures is the wiring, not the geometry.
    let (cursor, tag) = {
        let view = app.view.borrow();
        let viewport = Viewport::new(SIZE);
        (0..SIZE.y)
            .step_by(8)
            .flat_map(|y| {
                (0..SIZE.x)
                    .step_by(8)
                    .map(move |x| Vec2::new(x as f32, y as f32))
            })
            .find_map(|cursor| {
                let hit = *view.scene().pick(cursor, viewport, HOVER_REACH).first()?;
                Some((cursor, hit.tag))
            })
            .expect("the demo drawing covers some pixel of an 800×600 view")
    };

    // Entering the view changes the hover target, which wakes a frame by
    // itself — so the one that proves anything is the next, wholly inside.
    harness.move_to(cursor);
    harness.frame(|ui| app.viewport(ui));
    let delta = harness.move_to(cursor + Vec2::splat(2.0));
    assert!(
        delta.requests_repaint,
        "a move inside the viewport left the frame asleep, so the highlight would go stale"
    );

    // And the frame that move asks for is the one that lights the primitive.
    harness.move_to(cursor);
    harness.frame(|ui| app.viewport(ui));
    assert_eq!(
        app.hovered,
        app.names.get(tag),
        "the pick reported {tag:?} but the app hovered {:?}",
        app.hovered
    );
    assert!(
        app.hovered.is_some(),
        "aimed at the drawing and lit nothing"
    );

    // Off the drawing entirely, nothing stays lit.
    harness.move_to(Vec2::new(SIZE.x as f32 - 1.0, SIZE.y as f32 - 1.0));
    harness.frame(|ui| app.viewport(ui));
    assert_eq!(app.hovered, None);
}
