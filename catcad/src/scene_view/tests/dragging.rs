//! Taking hold of what the drawing drew, and what moves when the pointer
//! does.

use crate::demo;
use crate::drawing::Grip;
use crate::scene_view::aimed::Aimed;
use crate::scene_view::tests::harness::{RaisedView, open_markers, unmoved};
use aperture::Motion;
use glam::{DVec2, Vec2};

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
    let mut raised = RaisedView::new();
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
    let mut raised = RaisedView::new();
    raised.frame();
    let at_rest = raised.markers();
    assert!(
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .expect("a fixture opens the sketch it names")
            .outcome()
            .converged(),
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
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .open()
            .expect("a fixture opens the sketch it names")
            .outcome()
            .converged(),
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
        unmoved(&now[..5], &at_rest[..5]),
        "dragging the linkage moved the rectangle: {:?} against {:?}",
        &now[..5],
        &at_rest[..5]
    );
}

/// Every kind of grip is reachable through the real pick path, not only
/// constructible.
///
/// What a press lands on has to carry the `HitAt` that tells a slide from a
/// resize, and only a real hit carries one — the drawing's own tests build
/// those by hand, so this is what says the two agree.
#[test]
fn the_view_can_take_hold_of_a_point_an_edge_and_a_rim() {
    let mut raised = RaisedView::new();
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

/// A datum keeps the point it was grabbed by under the pointer, from either
/// side of the model.
///
/// The claim a drag on a plane is worth anything for, and the one the plumbing
/// underneath cannot make on its own. A travel line answers where the cursor
/// falls along it *as it looks*, so which of the parallel lines is asked
/// decides at what depth that answer is read — and depth is what perspective
/// scales by. Taken through the base plane's origin, the drag tracked the
/// cursor at the origin's depth while the corner in hand sat at another, so the
/// plane ran ahead of the pointer from one side and lagged it from the other.
///
/// Mirrored viewpoints rather than one, because the fault is a *ratio* of two
/// depths: it vanishes wherever the two happen to agree, and reverses as the
/// view swings past. One angle would have caught it only by luck.
#[test]
fn a_datum_keeps_the_point_it_was_grabbed_by_under_the_cursor() {
    for yaw in [-0.7f32, 0.7] {
        let mut raised = RaisedView::new();
        raised.document.camera_mut().yaw = yaw;
        raised.frame();
        let cursor = raised
            .over_datum()
            .unwrap_or_else(|| panic!("yaw {yaw}: no cursor found the datum"));
        let shelf = raised.shelf_plane();
        // Where the press lands on the plane, which is the point the drag has
        // hold of and the one that has to stay under the pointer.
        let grabbed = Motion::Plane {
            origin: shelf.origin.as_vec3(),
            normal: shelf.normal().as_vec3(),
        }
        .resolve(&Aimed::at(cursor).aim(raised.lens()))
        .unwrap_or_else(|| panic!("yaw {yaw}: the press missed the plane"));

        let step = Vec2::new(0.0, 45.0);
        raised.harness.press_at(cursor);
        raised.frame();
        raised.harness.drag_to(cursor + step);
        raised.frame();

        let moved = raised.shelf_plane();
        let travelled = (moved.origin - shelf.origin).dot(shelf.normal());
        assert!(
            travelled.abs() > 0.1,
            "yaw {yaw}: the drag carried the plane nowhere"
        );

        // Where that grabbed point now looks, against where the pointer now is.
        // Only along the axis: a pointer may wander across a line all it likes,
        // and a line drag is right to ignore that half.
        let normal = shelf.normal().as_vec3();
        let carried = grabbed + normal * travelled as f32;
        let axis = (raised.cursor_on(grabbed + normal) - raised.cursor_on(grabbed)).normalize();
        let adrift = (raised.cursor_on(carried) - (cursor + step)).dot(axis);
        assert!(
            adrift.abs() < 4.0,
            "yaw {yaw}: forty-five pixels of pointer left the grabbed point \
             {adrift} px adrift along its own axis"
        );
    }
}

/// **Dragging a solid's far end carries it, and moves nothing that was drawn.**
///
/// The gesture that makes an extrude parametric to the hand rather than only to
/// a number: the far cap travels along the normal of the plane its region was
/// drawn on, and the drawing underneath says exactly what it said before —
/// which is what a solid being *derived* means.
///
/// Which way it went rather than merely that it went, for the reason the datum
/// drag below says: a sign flipped anywhere between the ray and the distance
/// would carry the solid the other way and still pass an assertion that only
/// said it had changed.
#[test]
fn dragging_a_solids_far_end_carries_it_and_leaves_the_drawing_alone() {
    let mut raised = RaisedView::new();
    raised.frame();
    let cursor = raised
        .over_cap()
        .expect("no cursor found the far end of the demo's solid");

    let drawn = open_markers(&raised);
    let reach = |raised: &RaisedView| {
        raised
            .document
            .models(&raised.build, raised.session.editing())
            .solids()
            .next()
            .expect("the demo grows a solid")
            .1
            .distance()
    };
    let camera = raised.camera();
    let before = reach(&raised);

    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(0.0, 45.0));
    raised.frame();

    // Down the screen, on a view that opens looking down at the model, is down
    // the ground's own normal — so the solid comes back towards the plane it
    // stands on rather than growing away from it.
    let after = reach(&raised);
    assert!(
        after < before,
        "dragging down grew the solid, from {before} to {after}"
    );
    assert_eq!(
        raised.camera(),
        camera,
        "taking hold of the cap turned the view instead of carrying it"
    );
    assert_eq!(
        open_markers(&raised),
        drawn,
        "carrying the solid moved the drawing it was grown from"
    );
}

/// The gesture the plane's offset is edited by, and the one that has to work
/// from *outside* the sketch it moves: the demo opens on the ground, and the
/// datum being dragged is what the other sketch sits on. Every other press is
/// refused unless it lands in the sketch being worked in, so a plane taking one
/// is the whole of what this pins — along with the travel being an offset rather
/// than a place, which is the only thing a plane has to say.
#[test]
fn dragging_a_datum_slides_it_and_leaves_the_open_sketch_alone() {
    let mut raised = RaisedView::new();
    raised.frame();
    let cursor = raised
        .over_datum()
        .expect("no cursor found the datum to move");

    let drawn = open_markers(&raised);
    let shelf = raised.shelf_plane();
    assert_eq!(
        shelf.origin.y,
        demo::SHELF,
        "the shelf opens off the ground"
    );
    let camera = raised.camera();

    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(0.0, 45.0));
    raised.frame();

    let moved = raised.shelf_plane();
    // Down the screen, on a view that opens looking down at the model, is down
    // the ground's normal. Which way it went rather than merely that it went:
    // a sign flipped anywhere between the ray and the offset — in `travel`, in
    // `offset_at`, in the grab's own subtraction — sends the plane the other
    // way and would pass an assertion that only said it had moved.
    assert!(
        moved.origin.y < shelf.origin.y,
        "dragging down carried the plane up, to {}",
        moved.origin.y
    );
    // And along that normal and nothing else. A drag resolved against a plane
    // rather than a line would have carried it sideways too, and one that wrote
    // a place rather than an offset could have tipped it.
    assert_eq!(moved.origin.x, 0.0);
    assert_eq!(moved.origin.z, 0.0);
    assert_eq!(moved.normal(), shelf.normal());

    // And the sketch being worked in is untouched. It lies on the ground, which
    // this plane is measured *off* rather than the other way round — so a press
    // that had been taken for a grip on geometry, or a travel written as a place
    // in the open sketch, would show here.
    assert_eq!(
        open_markers(&raised),
        drawn,
        "moving a plane moved the sketch that is open"
    );
    assert_eq!(
        raised.camera(),
        camera,
        "a drag on a datum turned the camera"
    );
}

/// Taking hold of something picks it out, and the pointer goes on naming it
/// rather than whatever it is dragged across.
///
/// Two halves of one idea: mid-drag the pointer has already acted. What it
/// would act on if you pressed is no longer a question worth answering, so the
/// readout keeps naming the thing in hand — and geometry the cursor happens to
/// cross on its way is not offered as a choice that is not on offer.
///
/// The drag runs across the rest of the drawing rather than off into empty
/// space, so there is something for a stale hover to have found. Before this,
/// that is exactly what it found.
#[test]
fn a_drag_keeps_naming_what_it_holds_rather_than_what_it_passes_over() {
    let mut raised = RaisedView::new();
    raised.frame();
    let wrist = raised.cursor_on(raised.wrist());
    raised.harness.move_to(wrist);
    raised.frame();
    let held = raised.view.hovered().expect("the cursor is on the wrist");
    // A corner of the demo's frame, which is something else entirely — and what
    // a hover that followed the cursor would have latched onto.
    let corner = raised.cursor_on(
        raised
            .drawing()
            .plane()
            .point(DVec2::new(8.0, 5.0))
            .as_vec3(),
    );

    raised.harness.press_at(wrist);
    raised.frame();
    raised.harness.drag_to(wrist + Vec2::new(30.0, 20.0));
    raised.frame();

    assert_eq!(
        raised.session.selection().picked(),
        [held],
        "the drag did not pick out what it took hold of"
    );
    assert_eq!(
        raised.view.hovered(),
        Some(held),
        "the pointer stopped naming what it had hold of"
    );

    // Dragged on across the drawing, over geometry belonging to something else.
    // Neither answer moves: this is where the readout used to follow the cursor
    // onto whatever it was passing.
    for at in [raised.cursor_on(raised.empty_spot()), corner] {
        raised.harness.drag_to(at);
        raised.frame();
        assert_eq!(
            raised.view.hovered(),
            Some(held),
            "the pointer named something it was only passing over"
        );
        assert_eq!(raised.session.selection().picked(), [held]);
    }

    // And it answers for the cursor again once the button is up.
    raised.harness.release();
    raised.frame();
    raised.harness.move_to(corner);
    raised.frame();
    let after = raised.view.hovered();
    assert!(
        after.is_some() && after != Some(held),
        "after the drag the pointer reported {after:?} rather than what it now sits on"
    );
}
