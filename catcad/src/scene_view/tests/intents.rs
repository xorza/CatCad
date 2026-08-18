//! What a gesture hands the document, which is an intent and never an edit.

use crate::intent::Intent;
use crate::intent::change::Change;
use crate::scene_view::tests::harness::RaisedView;
use glam::Vec2;

/// The view asks; it does not act.
///
/// The whole of what the pipeline buys, and the thing every later feature leans
/// on: a gesture arrives as an intent and the document is untouched until that
/// intent is applied. A view that edited on its way past would leave nothing to
/// record, so there would be nothing for an undo to take back — and no single
/// place that could tell a document has changed since it was last saved.
///
/// The camera goes the same way as the drawing, which is the half easily got
/// wrong: it lives on the document too, so orbiting is as much an edit as
/// dragging is and cannot be allowed to happen inside the view.
#[test]
fn a_gesture_reaches_the_document_as_an_intent_rather_than_as_an_edit() {
    let mut raised = RaisedView::new();
    raised.frame();
    let cursor = raised.cursor_on(raised.wrist());
    raised.harness.move_to(cursor);
    raised.frame();

    let before = raised.asked_for();
    let camera = raised.camera();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));

    // The asking half alone. One drag was asked for, and nothing has moved.
    raised.ask();
    let asked: Vec<Intent> = raised.intents.iter().collect();
    assert!(
        matches!(asked[..], [Intent::Change(Change::Drag { .. })]),
        "a drag frame asked for {asked:?}"
    );
    assert_eq!(
        raised.asked_for(),
        before,
        "the view edited the drawing on its way past"
    );

    // Applying is what moves it, and what marks the drawing as needing to be
    // laid out again — which the settle says of itself rather than being told.
    let unlaid = raised.build.revision();
    raised
        .history
        .apply(&mut raised.document, &mut raised.build, &raised.intents);
    assert_ne!(
        raised.build.revision(),
        unlaid,
        "a drag left the drawing looking exactly as laid out as before"
    );
    assert_ne!(raised.asked_for(), before, "the applied drag moved nothing");

    // And the same of the camera: an orbit off the drawing is asked for, not
    // taken, though the camera is the document's as much as the sketch is.
    raised.harness.release();
    raised.frame();
    let empty = Vec2::new(4.0, 4.0);
    raised.harness.move_to(empty);
    raised.frame();
    raised.harness.press_at(empty);
    raised.frame();
    raised.harness.drag_to(empty + Vec2::new(60.0, 10.0));

    raised.ask();
    let asked: Vec<Intent> = raised.intents.iter().collect();
    assert!(
        matches!(asked[..], [Intent::Change(Change::Orbit { .. })]),
        "an orbit frame asked for {asked:?}"
    );
    assert_eq!(raised.camera(), camera, "the view turned the camera itself");
    // And it owes the drawing no redraw: where a thing is looked at from is
    // the document's, but it is not what is drawn. That an applied orbit does
    // turn the camera is `dragging_off_the_drawing_orbits_and_edits_nothing`,
    // which drives whole frames — an orbit is a delta against what the last
    // pass already took, so how far this one turns depends on which pass is
    // being read, and only the whole frame has a stable answer.
    let unlaid = raised.build.revision();
    raised
        .history
        .apply(&mut raised.document, &mut raised.build, &raised.intents);
    assert_eq!(
        raised.build.revision(),
        unlaid,
        "an orbit asked the drawing to be laid out again"
    );
}
