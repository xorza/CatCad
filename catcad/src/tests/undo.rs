//! What Ctrl+Z takes back, and what Ctrl+Shift+Z puts back.

use crate::tests::harness::Raised;
use crate::tool::Tool;
use glam::{DVec2, Vec2};
use palantir::Key;

use crate::hud::internals::POINT_BUTTON;

/// Ctrl+Z through the whole application: a real drag with the pointer, taken
/// back with the keyboard.
///
/// The one place the key bindings exist is `CatCad::record`, so the one way to
/// test them is to record real frames. What it pins beyond the history's own
/// tests is the wiring — that the chord is read at all, that reading it wakes a
/// frame, and that what it raises reaches the document before that frame is
/// drawn.
#[test]
fn ctrl_z_takes_back_a_drag_made_with_the_pointer() {
    let mut raised = Raised::new();

    let at_rest = raised.markers();
    let world = raised.app.wrist();
    let cursor = raised.cursor_on(world);

    // Press, travel past palantir's four-pixel latch, release.
    raised.harness.move_to(cursor);
    raised.frame();
    raised.harness.press_at(cursor);
    raised.frame();
    raised.harness.drag_to(cursor + Vec2::new(40.0, 25.0));
    raised.frame();
    raised.harness.release();
    raised.frame();
    let dragged = raised.markers();
    assert_ne!(dragged, at_rest, "the pointer moved nothing");

    // Now the keyboard. The chord has to wake a frame of its own: nothing else
    // is happening, and an undo that waited for an unrelated event would sit
    // unapplied on screen.
    let woken = raised.ctrl(Key::Char('Z'));
    assert!(
        woken.requests_repaint,
        "Ctrl+Z left the frame asleep, so the undo would not be drawn"
    );
    raised.frame();
    assert_eq!(
        raised.markers(),
        at_rest,
        "Ctrl+Z did not take the drag back"
    );

    // And Ctrl+Shift+Z puts it back. The modifiers are matched exactly, so the
    // two chords cannot be confused for one another.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        dragged,
        "Ctrl+Shift+Z did not put it back"
    );

    // With nothing left to put back, the chord changes nothing.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(raised.markers(), dragged);
}

/// The whole of the point tool, through the real application: pressed on the
/// toolbar, clicked into the viewport, and taken back with the keyboard.
///
/// The undo is the half worth the frames. Taking back a drag puts geometry
/// where it was; taking back a *creation* has to make geometry that exists stop
/// existing, which a snapshot of the solver's parameter vector could not
/// express — it names parameters by position, so one taken before the point was
/// added names the wrong ones after it. What this pins is that the whole path
/// agrees on that: the sketch comes back the width it was, the freedoms are
/// counted again over what is left, and the picture on screen is relaid out.
#[test]
fn the_toolbar_places_a_point_and_ctrl_z_takes_it_back() {
    let mut raised = Raised::new();
    let at_rest = raised.markers();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "the app opened with a tool in hand"
    );

    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Point,
        "the toolbar did not arm the point tool"
    );

    // Empty plane, because a click on something already drawn puts the tool
    // down instead of drawing over it. The spot lies on the sketch plane, so
    // the ray through the pixel showing it meets the plane exactly there, and
    // where the new point belongs is known rather than read back off the thing
    // that placed it.
    let empty = raised.app.empty_spot();
    let cursor = raised.cursor_on(empty);

    raised.harness.click_at(cursor);
    raised.frame();
    let placed = raised.markers();
    assert_eq!(placed.len(), at_rest.len() + 1, "the click placed nothing");
    assert!(
        placed.iter().any(|at| at.abs_diff_eq(empty, 1e-3)),
        "nothing was placed under the cursor at {empty:?}, only {placed:?}"
    );
    // A free point is two more things the drawing can decide, and the status
    // line is where that shows — so the freedoms were measured again over the
    // sketch as it now stands rather than carried over from before.
    assert!(
        raised
            .app
            .status()
            .to_string()
            .starts_with("solved · 7 dof · 0 redundant"),
        "the demo's five degrees of freedom did not become seven: {}",
        raised.app.status()
    );

    // Taken back: the point is gone, and so are the freedoms it brought.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        at_rest,
        "Ctrl+Z did not take the point back"
    );
    assert!(
        raised
            .app
            .status()
            .to_string()
            .starts_with("solved · 5 dof · 0 redundant"),
        "the drawing kept the freedoms of a point it no longer holds: {}",
        raised.app.status()
    );

    // And put back, which is the harder direction: the redo has to widen a
    // sketch that has since been narrowed.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        placed,
        "Ctrl+Shift+Z did not put the point back"
    );

    // Three ways to put a tool down, all landing on the same field through the
    // same inbox. Escape first, from wherever the pointer happens to be.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "Escape did not put the tool down"
    );

    // The right button over the drawing, which is the gesture a modeller
    // reaches for first.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);
    let held = raised.markers();
    raised.harness.right_click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "the right button left it in hand"
    );
    // And it is really down, not merely drawn as down: the click that follows
    // places nothing.
    raised.harness.click_at(cursor);
    raised.frame();
    assert_eq!(
        raised.markers(),
        held,
        "a cancelled tool went on placing points"
    );

    // And its own button again, because pressing the tool in hand puts it down.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(
        raised.app.session.tool(),
        Tool::Pointer,
        "pressing the armed tool re-armed it rather than putting it down"
    );
}

/// Taking back the step that created something drops it from the selection, so
/// the next thing created is not mistaken for it.
///
/// A handle is not made safe by the generation it carries here. That is what
/// refuses a handle to something *removed*, and nothing removes geometry — an
/// undo restores the sketch whole, arenas and generations alike, precisely so
/// that a handle held across a step still names what it named. The cost is that
/// the very next point added takes the handle the undone one had: measured, the
/// two are both `Id(9#0)`. So a selection that kept the first would light the
/// second, green and unasked for.
#[test]
fn undoing_a_creation_takes_what_it_created_out_of_the_selection() {
    let mut raised = Raised::new();
    let at_rest = raised.markers();

    // Place a point on empty plane, put the tool down, and pick the point out.
    let spot = raised.app.empty_spot();
    let first = raised.cursor_on(spot);
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    raised.harness.click_at(first);
    raised.frame();
    raised.harness.key(Key::Escape);
    raised.frame();
    raised.harness.click_at(first);
    raised.frame();
    assert_eq!(raised.markers().len(), at_rest.len() + 1);
    assert_eq!(
        raised.app.session.selection().count(),
        1,
        "the new point was not picked out"
    );

    // Take the creation back. The point goes, and so does the handle to it.
    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.markers(),
        at_rest,
        "Ctrl+Z did not take the point back"
    );
    assert_eq!(
        raised.app.session.selection().count(),
        0,
        "a handle to what the undo removed is still picked out"
    );

    // Now a different point, somewhere else — minted with the handle the undone
    // one had. Nobody picked it, so nothing is picked out.
    let elsewhere = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .plane()
        .point(DVec2::new(-1.5, 4.5))
        .as_vec3();
    let second = raised.cursor_on(elsewhere);
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    raised.harness.click_at(second);
    raised.frame();

    let now = raised.markers();
    assert_eq!(
        now.len(),
        at_rest.len() + 1,
        "the second click placed nothing"
    );
    assert!(
        now.iter().any(|at| at.abs_diff_eq(elsewhere, 1e-3)),
        "the second point did not land where it was asked for"
    );
    let newest = raised
        .app
        .document
        .drawing_at(raised.app.session.editing())
        .sketch()
        .points()
        .last()
        .expect("the sketch holds points")
        .0;
    let newest = raised.models().open().part(newest);
    assert!(
        !raised.app.session.selection().contains(newest),
        "a point nobody picked came up selected, on a handle left over from an undo"
    );
}
