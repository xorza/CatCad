//! Editing what is already drawn: tidying it, growing a solid off it, and
//! dragging past the edge of the view.

use crate::hud::internals::{LINE_BUTTON, POINT_BUTTON};
use crate::prompt::{Asking, Prompt};
use crate::tests::harness::Raised;
use glam::{DVec2, Vec2, Vec3};
use palantir::Key;

use crate::hud::internals::{EXTRUDE_BUTTON, TIDY_BUTTON};
use crate::intent::Choice;
use crate::tool::Tool;

/// The clean-up button takes out geometry a deletion left behind, and leaves
/// the drawing it was pressed on otherwise alone.
///
/// The end of the wiring the sketch's own tests start: a press reaches the
/// document as [`Change::Tidy`] and lands on the drawing. What makes a spare
/// here is the realistic route to one — an edge deleted out from under a join
/// leaves its corner point tied to a neighbour and holding up nothing, which is
/// exactly the litter the command exists for.
#[test]
fn the_clean_up_button_clears_what_a_deletion_left_behind() {
    let mut raised = Raised::new();
    let at_rest = raised.drawing().sketch().points().count();
    let edges = raised.drawing().sketch().segments().count();

    let plane = raised.drawing().plane();
    let corner = [
        plane.point(DVec2::new(-1.5, 1.0)).as_vec3(),
        plane.point(DVec2::new(-1.5, 3.5)).as_vec3(),
        plane.point(DVec2::new(-4.0, 3.5)).as_vec3(),
    ];
    let at = corner.map(|world| raised.cursor_on(world));

    // Two edges meeting at a corner: four points and the coincidence tying the
    // middle pair.
    raised.harness.click_at(LINE_BUTTON);
    raised.frame();
    for spot in [at[0], at[1], at[1], at[2]] {
        raised.harness.click_at(spot);
        raised.frame();
    }
    assert_eq!(raised.drawing().sketch().points().count(), at_rest + 4);

    // Pressed on that, the command finds nothing: every one of those points
    // ends an edge.
    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    assert_eq!(
        raised.drawing().sketch().points().count(),
        at_rest + 4,
        "a cleanup ate a corner that was holding an edge up"
    );
    // And says so, rather than answering a press with nothing.
    assert!(
        raised
            .app
            .status()
            .to_string()
            .ends_with(" · nothing to clean up"),
        "the status line read {}",
        raised.app.status()
    );

    // Now take the second edge away. Its far end is left over but duplicates
    // nothing, and its corner end is left over *and* still tied to the first
    // edge's — so one of the two goes and the other does not.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    let midpoint = raised.cursor_on(corner[1].midpoint(corner[2]));
    raised.harness.click_at(midpoint);
    raised.frame();
    raised.harness.key(Key::Delete);
    raised.frame();
    let sketch = raised.drawing().sketch();
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "the edge was not deleted"
    );
    assert_eq!(sketch.points().count(), at_rest + 4, "its ends stayed");

    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    let sketch = raised.drawing().sketch();
    assert_eq!(
        sketch.points().count(),
        at_rest + 3,
        "the orphaned corner was not cleared"
    );
    assert_eq!(
        sketch.segments().count(),
        edges + 1,
        "the surviving edge went too"
    );
    assert!(
        raised
            .app
            .status()
            .to_string()
            .ends_with(" · removed 1 point"),
        "the status line read {}",
        raised.app.status()
    );

    // And pressing it again finds nothing, which is what makes it safe to lean
    // on — and the line goes back to saying so.
    raised.harness.click_at(TIDY_BUTTON);
    raised.frame();
    assert_eq!(raised.drawing().sketch().points().count(), at_rest + 3);
    assert!(
        raised
            .app
            .status()
            .to_string()
            .ends_with(" · nothing to clean up")
    );

    // A later edit takes the note away: it described the last thing done, and
    // it no longer is.
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    let empty = raised.cursor_on(plane.point(DVec2::new(-6.0, 1.0)).as_vec3());
    raised.harness.click_at(empty);
    raised.frame();
    let line = raised.app.status().to_string();
    assert!(
        !line.contains("clean up") && !line.contains("removed"),
        "a stale cleanup note outlived the edit after it: {line}"
    );
}

/// **A drag that outruns the view keeps hold of what it grabbed.**
///
/// The pointer leaving the viewport is not the user letting go, and a drag that
/// stopped there would strand geometry wherever the edge happened to be — worst
/// on a small window, where every long pull crosses one.
///
/// What it pins is a distinction two readings of the same cursor turn on. The
/// press, the click and the hover take the cursor **filtered** by `hovered`, so
/// the overlay's own controls do not light what is behind them; what resolves
/// against a plane takes it **bare**, and palantir keeps answering
/// `pointer_local` off the widget precisely so that it can. The two are one
/// `Option<Aimed>` apiece and nothing but this says which call wants which —
/// see [`aimed::landing`](crate::scene_view::aimed).
///
/// Two legs rather than one, and the second further out, so what is asserted is
/// that the drag went on *tracking* after the pointer left rather than landing
/// one more frame and stopping.
#[test]
fn a_drag_that_leaves_the_view_goes_on_moving_what_it_holds() {
    let mut raised = Raised::new();

    let world = raised.app.wrist();
    let cursor = raised.cursor_on(world);
    raised.harness.move_to(cursor);
    raised.frame();
    let before = raised.markers();
    raised.harness.press_at(cursor);
    raised.frame();

    // Inside the view, which is the leg that works either way.
    raised.harness.drag_to(cursor + Vec2::new(60.0, 0.0));
    raised.frame();
    let inside = raised.markers();
    assert_ne!(
        inside, before,
        "the drag moved nothing while still on the view"
    );

    // Off the left edge by a clear margin — `HARNESS_SIZE` is 800 across, so a negative
    // x is outside it however the view was arranged.
    raised.harness.drag_to(Vec2::new(-200.0, cursor.y));
    raised.frame();
    let outside = raised.markers();
    assert_ne!(
        outside, inside,
        "the drag stopped the moment the pointer left the view"
    );

    // And it went on the way it was pulled rather than merely twitching once.
    // The farthest any marker has come from where it started, because the drag
    // reaches the wrist through the constraints and what travels most is not
    // decided here — what matters is that the drawing kept going.
    let travelled = |now: &[Vec3]| {
        now.iter()
            .zip(&before)
            .map(|(now, was)| now.distance(*was))
            .fold(0.0, f32::max)
    };
    assert!(
        travelled(&outside) > travelled(&inside),
        "the drawing ended {} from where it started having been {} at the edge",
        travelled(&outside),
        travelled(&inside),
    );

    raised.harness.release();
    raised.frame();
}

/// **A region picked out grows a solid, and Ctrl+Z takes the whole step back.**
///
/// The path a user actually has: click a region, press Extrude, and a step
/// appears on the end of the document. Which is the first thing anyone can do
/// that *adds* a step rather than rewriting one, and so the first thing the
/// history had to learn to record — a step that was not there has no earlier
/// value to put back, so undoing one takes the step away again.
///
/// Both halves are asked, because either alone is a trap. A creation that
/// nothing records is a step the user cannot take back; an undo that put the
/// value back rather than the step would leave a solid behind grown from
/// nothing.
#[test]
fn extruding_a_region_grows_a_solid_and_ctrl_z_takes_the_step_back() {
    let mut raised = Raised::new();

    // The demo opens with one, grown off the hub.
    assert_eq!(raised.solids(), 1);

    // The frame is region 0 of the open sketch — the rectangle with the hub cut
    // out of it, which is not the region the demo already grew from.
    let frame_region = raised
        .models()
        .open()
        .expect("a fixture opens the sketch it names")
        .region(0);
    raised.choose(Choice::Select(Some(frame_region)));
    raised.frame();

    // The bar shows the button only while a region is picked, so where it lands
    // is found rather than guessed: it is the leftmost thing on the bottom bar.
    raised.harness.click_at(EXTRUDE_BUTTON);
    raised.frame();
    // The button *asks* rather than builds: the solid is on screen at no depth
    // at all, drawn from the form's own reading, and the timeline has not heard
    // of it. A cancel here would leave nothing behind to take back.
    assert!(
        matches!(
            raised.app.session.prompt().map(Prompt::about),
            Some(Asking::Extrude { .. })
        ),
        "pressing Extrude opened no form: {}",
        raised.app.status()
    );
    assert_eq!(
        raised.solids(),
        1,
        "pressing Extrude reached the document before the depth was settled"
    );

    // The depth typed, and Enter to settle it. One step, carrying the depth it
    // was given rather than a zero that was then carried.
    raised.harness.type_text("2");
    raised.frame();
    raised.harness.key(Key::Enter);
    raised.frame();
    assert!(
        raised.app.session.prompt().is_none(),
        "Enter left the form open"
    );
    assert_eq!(
        raised.solids(),
        2,
        "committing the form did not grow a solid: {}",
        raised.app.status()
    );

    raised.ctrl(Key::Char('Z'));
    raised.frame();
    assert_eq!(
        raised.solids(),
        1,
        "Ctrl+Z left the solid behind, so the creation went unrecorded"
    );

    // And back again, which is the half that says the step returns rather than
    // a fresh one taking its place.
    raised.ctrl_shift(Key::Char('Z'));
    raised.frame();
    assert_eq!(raised.solids(), 2, "redo did not put the step back");
}

/// Escape backs out of one thing at a time: the tool first, then the sketch it
/// was drawing in.
///
/// Two steps rather than one, because they are two things to be out of. A key
/// that put the tool down *and* closed the drawing would be a key you could not
/// use without losing your place — and one that only ever did the first would
/// leave no way back out at all.
///
/// What closing takes with it is everything the session holds *about* the
/// drawing, and nothing else: the tool goes because it draws in the sketch you
/// are in, and what is picked out stays because a selection may name parts of
/// any sketch and of none.
#[test]
fn escape_puts_down_the_tool_before_it_closes_the_sketch() {
    let mut raised = Raised::new();
    let sketch = raised.app.editing();
    raised.harness.click_at(POINT_BUTTON);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Point);

    // First press: the tool alone. The sketch is still open, which is the whole
    // of the claim — a tool put down is not a drawing left.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(raised.app.session.tool(), Tool::Pointer);
    assert_eq!(
        raised.app.session.editing(),
        Some(sketch),
        "putting the tool down closed the sketch under it"
    );

    // Second press: the sketch. And the readout says so rather than reporting a
    // solve nobody asked for.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(raised.app.session.editing(), None);
    assert!(
        raised
            .app
            .status()
            .to_string()
            .starts_with("no sketch open"),
        "the readout still reports a solve: {}",
        raised.app.status()
    );

    // Closing again closes nothing again — every intent names where it wants to
    // end up, so a replayed pass lands on the same answer.
    raised.harness.key(Key::Escape);
    raised.frame();
    assert_eq!(raised.app.session.editing(), None);

    // And back in by clicking something, which is the one gesture that says
    // which sketch you mean because it is the one that says which thing.
    raised.enter_first_sketch();
    raised.frame();
    assert_eq!(raised.app.session.editing(), Some(sketch));
}
