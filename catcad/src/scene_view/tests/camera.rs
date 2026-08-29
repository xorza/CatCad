//! Turning, panning and zooming the view, and what that must not edit.

use crate::hud::cube::Gizmo;
use crate::internals::HARNESS_SIZE;
use crate::look::Theme;
use crate::scene_view::pointing::ZOOM_RATE;
use crate::scene_view::tests::harness::{RaisedView, open_markers};
use glam::Vec2;
use palantir::PointerButton;

/// Dragging a datum slides it along the line it is offset on, carrying what is
/// drawn on it and touching neither the open sketch nor the camera.
///
/// The middle button slides the picture, and takes hold of nothing on the way.
///
/// Its own gesture in the plain sense and not a [`Gesture`]: there is nothing
/// under the cursor for a press to decide about, so it lives beside the wheel
/// rather than beside the grab. Which is the half worth pinning — a pan that
/// went through the press would grab whatever it started over, and a pan
/// wanting to start over the drawing is the whole point of having one.
#[test]
fn the_middle_button_pans_the_view_and_grabs_nothing() {
    let mut raised = RaisedView::new();
    raised.frame();
    let before = raised.camera();
    let drawn = open_markers(&raised);

    // From the middle of the view, which is over the drawing: the left button
    // on this very pixel takes hold of geometry, so a pan that reached the
    // grab would be caught here and nowhere else.
    let centre = HARNESS_SIZE.as_vec2() * 0.5;
    let step = Vec2::new(60.0, -35.0);
    raised
        .harness
        .press_button_at(PointerButton::Middle, centre);
    raised.frame();
    raised.harness.drag_to(centre + step);
    raised.frame();

    // What stood at the orbit target — the one depth a pan is measured at — has
    // travelled with the pointer, by the pointer's own travel and no rate of
    // its own. Which way as much as how far: a sign dropped anywhere between
    // the drag and the camera slides the picture against the hand.
    let carried = raised.cursor_on(before.target);
    assert!(
        (carried - (centre + step)).length() < 1.0,
        "the pointer travelled {step:?} and the picture went to {carried:?}, \
         not {:?}",
        centre + step
    );

    // And nothing else moved. The camera was panned rather than turned or
    // pulled, and the drawing under the press was not taken hold of — either
    // would mean the middle button had fallen through to the left one's path.
    let after = raised.camera();
    assert_eq!(after.yaw, before.yaw, "a pan turned the camera");
    assert_eq!(after.pitch, before.pitch, "a pan tilted the camera");
    assert_eq!(after.distance, before.distance, "a pan zoomed the camera");
    assert_eq!(open_markers(&raised), drawn, "a pan dragged the drawing");
}

/// Two fingers travelling slide the view by exactly what they travelled, and
/// change nothing else about it.
///
/// The whole of what a pan promises is that what you put your fingers on stays
/// under them, so the check is where a fixed world point lands on screen rather
/// than what the camera's numbers came out as. Palantir hands a trackpad's
/// travel over as a scroll in logical pixels — the same delta a page would be
/// scrolled by — and the viewport moving over the scene is what turns that into
/// a camera step.
#[test]
fn two_fingers_travelling_pan_the_view_by_what_they_travelled() {
    let mut raised = RaisedView::new();
    raised.frame();
    let centre = Vec2::new(400.0, 300.0);
    raised.harness.move_to(centre);
    raised.frame();

    // The target projects to the middle of the viewport, which is where the
    // camera is by definition pointed.
    let anchor = raised.camera().target;
    assert!((raised.cursor_on(anchor) - centre).length() < 0.5);
    let before = raised.camera();
    let markers = raised.markers();

    // Fingers going left and up: the viewport travels the other way over the
    // scene, which is what a scroll delta already says.
    let travelled = Vec2::new(-90.0, 40.0);
    raised.harness.scroll_pixels_at(centre, travelled);
    raised.frame();

    let landed = raised.cursor_on(anchor);
    assert!(
        (landed - (centre - travelled)).length() < 0.5,
        "a pan of {travelled:?} left the target at {landed:?}, not {:?}",
        centre - travelled
    );
    assert_ne!(raised.camera().target, before.target, "nothing panned");
    assert_eq!(
        (
            raised.camera().distance,
            raised.camera().yaw,
            raised.camera().pitch
        ),
        (before.distance, before.yaw, before.pitch),
        "a pan turned or approached the scene as well as sliding it"
    );
    assert_eq!(raised.markers(), markers, "panning edited the drawing");

    // And it lands once however many passes the frame recorded. A scroll is
    // drained between them, so a pan that arrives as a step rather than as a
    // destination is still applied exactly as far as it was asked for — which
    // is what the pixel check above would catch doubling.
    raised.frame();
    assert!((raised.cursor_on(anchor) - (centre - travelled)).length() < 0.5);
}

/// The wheel and the pinch zoom by what they were given, agree about which way
/// is closer, and move the view no other way.
///
/// Both gestures on one fixture because the one thing they have to agree on is
/// what closer *means*, and two tests cannot assert an agreement. A pinch says
/// it outright — fingers apart is a bigger picture — and the wheel's number is
/// a scroll offset, positive being a scroll down, which is the direction that
/// takes the eye out. Both directions of both, because a zoom that only ever
/// grew would pass a test that watched one end of one of them.
#[test]
fn the_wheel_and_the_pinch_zoom_the_same_way_round() {
    let mut raised = RaisedView::new();
    raised.frame();
    let centre = Vec2::new(400.0, 300.0);
    raised.harness.move_to(centre);
    raised.frame();
    let before = raised.camera();

    // A notch down is one whole ZOOM_RATE further off.
    raised.harness.scroll_lines_at(centre, Vec2::new(0.0, 1.0));
    raised.frame();
    let out = raised.camera();
    assert!(
        (out.distance - before.distance * ZOOM_RATE).abs() < before.distance * 1e-5,
        "a notch down left the eye at {} from {}",
        out.distance,
        before.distance
    );
    assert_eq!(
        (out.target, out.yaw, out.pitch),
        (before.target, before.yaw, before.pitch),
        "the wheel moved the view as well as zooming it"
    );

    // And two notches back up is two rates in from there, which is one rate
    // nearer than it started.
    raised.harness.scroll_lines_at(centre, Vec2::new(0.0, -2.0));
    raised.frame();
    let up = raised.camera().distance;
    assert!(
        (up - before.distance / ZOOM_RATE).abs() < before.distance * 1e-5,
        "two notches up from {} left the eye at {up}",
        out.distance
    );
    assert!(up < before.distance, "scrolling up did not come closer");

    // Fingers apart asks for a bigger picture, which is a shorter orbit — the
    // same direction scrolling up went.
    raised.harness.pinch_at(centre, 1.25);
    raised.frame();
    let closer = raised.camera();
    assert!(
        (closer.distance - up / 1.25).abs() < before.distance * 1e-5,
        "a 1.25 pinch left the eye at {} from {up}",
        closer.distance
    );
    assert!(
        closer.distance < up,
        "the pinch and the wheel disagree about which way is closer"
    );
    assert_eq!(
        (closer.target, closer.yaw, closer.pitch),
        (before.target, before.yaw, before.pitch),
        "a pinch moved the view as well as zooming it"
    );

    // And the way back out, by the reciprocal, is the distance it pinched from.
    raised.harness.pinch_at(centre, 0.8);
    raised.frame();
    assert!(
        (raised.camera().distance - up).abs() < before.distance * 1e-5,
        "{:?} did not undo the pinch",
        raised.camera()
    );
}

/// Pressing where the drawing is not turns the camera, which is the only way
/// the view can be looked around — so a drag has to fall back to it rather
/// than swallow the gesture.
#[test]
fn dragging_off_the_drawing_orbits_and_edits_nothing() {
    let mut raised = RaisedView::new();
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
    let mut raised = RaisedView::new();
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

    assert_ne!(
        raised.camera(),
        camera,
        "a press on something that does not move has to orbit"
    );
    assert_eq!(raised.markers(), before, "a pinned point was dragged");
}

/// The camera the document holds is the one the renderer paints through.
///
/// The round trip the projection toggle makes: the overlay reads the document,
/// writes back what was asked for, and the frame has to be drawn through it.
/// Settling hands the renderer a copy every frame, which is what closes that
/// loop — a copy refreshed only on change is what would leave it open.
#[test]
fn settling_aims_the_renderer_through_the_documents_own_camera() {
    let mut raised = RaisedView::new();
    raised.frame();
    assert_eq!(raised.view.pane().camera, raised.camera());

    // Turn the camera the way a gesture would, and the renderer follows.
    raised.document.camera_mut().orbit(0.4, 0.2);
    let turned = raised.camera();
    assert_ne!(
        raised.view.pane().camera,
        turned,
        "nothing to prove otherwise"
    );
    raised.view.settle(
        &raised.document,
        &raised.build,
        &Theme::default(),
        &raised.session,
        Gizmo::NOWHERE,
    );
    assert_eq!(raised.view.pane().camera, turned);

    // The projection rides along with it, which is the toggle's whole path.
    let was = raised.camera().projection;
    raised.document.camera_mut().projection = was.toggled();
    raised.view.settle(
        &raised.document,
        &raised.build,
        &Theme::default(),
        &raised.session,
        Gizmo::NOWHERE,
    );
    let now = raised.view.pane().camera.projection;
    assert_eq!(now, was.toggled());
    assert_ne!(now, was);
}
