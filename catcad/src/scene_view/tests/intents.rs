//! What a gesture hands the document, which is an intent and never an edit.

use crate::intent::change::Change;
use crate::intent::{Choice, Intent, Intents};
use crate::part::Part;
use crate::scene_view::click::{Click, clicked};
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

/// A second click on a plane starts a sketch on it; a second click on anything
/// else starts nothing.
///
/// **The gesture that makes a world plane worth having.** Without it the three a
/// document comes with are decoration and an empty document is a dead end — see
/// [`Change::AddSketch`].
///
/// Driven through [`clicked`] rather than the pointer, and that seam is the same
/// one every other double-click test takes: what a click *found* is resolved
/// against a painted frame, and these harnesses record without a GPU. What is
/// asked here is the half that needs no measuring — given that the click landed
/// on a plane, what does it come to.
///
/// The plane a *sketch* is drawn on as well as one nothing stands on, because
/// the two read differently and only one of them is obvious: a second click on
/// a plane already carrying a sketch starts another rather than opening the one
/// that is there. Opening what is there is a different command with no answer
/// where a plane carries three.
#[test]
fn a_second_click_on_a_plane_starts_a_sketch_and_on_anything_else_starts_none() {
    let raised = RaisedView::new();
    let sketch = raised.editing();
    let drawing = raised.document.drawn(sketch);
    let (point, _) = drawing
        .sketch()
        .points()
        .next()
        .expect("the demo draws points");

    let asked = |double: bool, under: Option<Part>| {
        let mut intents = Intents::default();
        clicked(
            Click {
                double,
                adding: false,
                under,
                // Nothing resolved on the plane, so no tool has anywhere to
                // build — which keeps this about the double-click alone.
                at: None,
            },
            &raised.document,
            &raised.session,
            &mut intents,
        );
        intents.iter().collect::<Vec<Intent>>()
    };
    let starts = |asked: &[Intent], on| {
        asked.iter().any(
            |intent| matches!(intent, Intent::Change(Change::AddSketch { on: at }) if *at == on),
        )
    };

    // The plane the open sketch is drawn on, so this is also the case that says
    // a plane already carrying a sketch gets another rather than a refusal.
    let carrying = raised
        .document
        .models(&raised.build, Some(sketch))
        .open_plane()
        .expect("a fixture opens the sketch it names");
    let twice = asked(true, Some(Part::Step(carrying)));
    assert!(
        starts(&twice, carrying),
        "a second click on a plane asked for {twice:?}"
    );
    // And it puts away whatever form was open, exactly as any other click does:
    // the sketch it was about is not the sketch you are now in.
    assert!(
        twice
            .iter()
            .any(|intent| matches!(intent, Intent::Choice(Choice::Ask(None)))),
        "starting a sketch left a form standing over the one you left"
    );

    // One click on the same plane picks it out and starts nothing — which is
    // what makes the bar's Sketch button the same answer asked a second way.
    let once = asked(false, Some(Part::Step(carrying)));
    assert!(
        !starts(&once, carrying),
        "one click on a plane started a sketch: {once:?}"
    );
    assert!(
        once.iter().any(|intent| matches!(
            intent,
            Intent::Choice(Choice::Select(Some(Part::Step(at)))) if *at == carrying
        )),
        "one click on a plane did not pick it out: {once:?}"
    );

    // Nothing else has a sketch to start. A point is geometry drawn *in* one,
    // and empty space names no plane at all.
    for under in [
        Some(Part::Entity {
            sketch,
            entity: point.into(),
        }),
        None,
    ] {
        let asked = asked(true, under);
        assert!(
            !asked
                .iter()
                .any(|intent| matches!(intent, Intent::Change(Change::AddSketch { .. }))),
            "a second click on {under:?} asked for {asked:?}"
        );
    }
}
