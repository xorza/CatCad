//! One gate per thing the pointer can be doing.

use crate::raised::Raised;
use catcad::CatCad;
use common::AllocTester;
use glam::{Vec2, Vec3};

/// Where the pointer sits for the still gate — off the drawing and off the
/// overlay, so nothing is hovered and the status line stays at its shortest.
///
/// Along the top edge between the two surfaces pinned there, which is the one
/// stretch of the view that is neither. The gate asserts that nothing in the
/// *drawing* is hovered; a chip under the pointer would pass that and still
/// measure a frame with a tooltip opening in it.
const PARKED: Vec2 = Vec2::new(800.0, 6.0);

/// How wide the hovering gate's sweep runs, in pixels, centred on the wrist —
/// and so how many frames it takes to walk one, the step being a pixel.
///
/// Wide enough to leave the arm, so what the window measures is *hovering*
/// rather than one answer held for the whole of it: the near half crosses a
/// point, the edges meeting at it and the region behind them, and the far half
/// crosses nothing. A sweep that stayed on any one of those would measure the
/// cheapest or the dearest case and report it as the middle.
const SWEEP: usize = 120;

/// Frames with the pointer parked off the drawing.
#[test]
fn a_still_frame_allocates_nothing() {
    let mut raised = Raised::new();
    raised.harness.move_to(PARKED);
    raised.frame();
    assert!(
        !raised.app.hovering_anything(),
        "the parked pointer is over something, so this is a second hovering gate",
    );
    AllocTester::new().run(|| raised.frame());
}

/// Frames with a control easing between states under the pointer.
///
/// **The one gate that measures the overlay rather than the drawing.** A chip
/// eases its fill and its ink toward what it should be wearing, and palantir
/// keeps that against the widget's id — a row created when the target moves and
/// drained when it arrives. Walking between two chips keeps a row live on every
/// frame, which is the case that could reach the heap and the case no other gate
/// here touches.
///
/// The walk cycles between two chips, so it warms in whole cycles rather than
/// through the probe: a probe can settle inside one cycle, before its widest
/// frame has happened at all.
#[test]
fn a_frame_with_a_control_easing_allocates_nothing() {
    let mut raised = Raised::new();
    let (from, to) = (
        raised.at(CatCad::tool_chip("Point")),
        raised.at(CatCad::tool_chip("Circle")),
    );
    // Both ends walked before any of it is measured, so the rows and the typed
    // map palantir keeps them in are asked for outside the window.
    for at in [from, to, from] {
        raised.harness.move_to(at);
        raised.frame();
    }
    // The gate reaches the frame it names, which a number alone cannot say: a
    // walk that landed between the chips would go on reporting zero about
    // frames with no control easing in them at all.
    assert!(
        raised.hovers(CatCad::tool_chip("Point")),
        "the walk missed the chip at {from:?}, so nothing is easing",
    );
    let mut step = 0usize;
    AllocTester::new().warmup(16).run(|| {
        step += 1;
        raised
            .harness
            .move_to(if step.is_multiple_of(2) { from } else { to });
        raised.frame();
    });
}

/// Frames with the pointer walking across the drawing, which is what the app
/// does whenever someone is using it.
///
/// Warmed in whole sweeps rather than through the probe, the sweep being a
/// cycle: its near half hovers geometry and its far half hovers nothing, and a
/// probe settling in the far half would never have met the near one.
#[test]
fn a_hovering_frame_allocates_nothing() {
    let mut raised = Raised::new();
    let middle = raised.cursor_on(raised.app.wrist());
    let across = |step: usize| Vec2::new(step as f32 - 0.5 * SWEEP as f32, 0.0);

    // Both halves of the sweep, walked before any of it is measured: a window
    // that only ever hovered one thing would be reporting that thing's cost as
    // hovering's.
    let mut over = 0;
    let mut clear = 0;
    for step in 0..SWEEP {
        raised.harness.move_to(middle + across(step));
        raised.frame();
        if raised.app.hovering_anything() {
            over += 1;
        } else {
            clear += 1;
        }
    }
    assert!(
        over > 0 && clear > 0,
        "the sweep found {over} frames over something and {clear} over nothing, \
         so it measures one of the two rather than the mix",
    );

    let mut step = 0usize;
    AllocTester::new().warmup(SWEEP).run(|| {
        step += 1;
        raised.harness.move_to(middle + across(step % SWEEP));
        raised.frame();
    });
}

/// Frames with a point actually being taken somewhere.
///
/// The press is landed before the window opens, so what is measured is the
/// middle of a gesture rather than the start of one.
#[test]
fn a_dragging_frame_allocates_nothing() {
    let mut raised = Raised::new();
    let grabbed = raised.cursor_on(raised.app.wrist());
    raised.harness.move_to(grabbed);
    raised.frame();
    raised.harness.press_at(grabbed);
    raised.frame();

    // One drag before the window, both to latch the gesture and to say that it
    // *writes*: a press that took hold of nothing goes on reporting zero for a
    // frame that never reaches the solver.
    let was = raised.app.wrist();
    raised.harness.drag_to(grabbed + Vec2::new(41.0, 24.0));
    raised.frame();
    assert!(
        raised.app.wrist().distance(was) > 0.0,
        "the drag moved nothing, so this measures a gesture that never solves",
    );

    let mut step = 0usize;
    AllocTester::new().warmup(16).run(|| {
        step += 1;
        // Well past palantir's four-pixel latch and never twice in the same
        // place, so the drag stays live and the geometry keeps moving — a drag
        // asked for where it already is settles to a no-op, which is not the
        // frame this is meant to be measuring.
        raised
            .harness
            .drag_to(grabbed + Vec2::new(40.0 + (step % 16) as f32, 24.0));
        raised.frame();
    });
}

/// Frames with a line half drawn, its rubber band following the cursor.
///
/// The band is the one thing the view draws that the document did not write,
/// and it is rewritten every frame the pointer moves.
#[test]
fn a_banding_frame_allocates_nothing() {
    let mut raised = Raised::new();
    let line = raised.at(CatCad::tool_chip("Line"));
    raised.harness.click_at(line);
    raised.frame();
    assert!(
        raised.app.drawing_a_line(),
        "the click at {line:?} missed the chip, so no line tool is in hand",
    );

    // Clear of the arm, so the first click starts a line rather than putting
    // the tool down on something already drawn.
    let empty = raised.app.wrist() + Vec3::new(0.0, 0.0, 3.0);
    let start = raised.cursor_on(empty);
    raised.harness.click_at(start);
    raised.frame();
    assert!(
        raised.app.tool_begun(),
        "no line was begun, so there is no band here to measure",
    );

    let mut step = 0usize;
    AllocTester::new().warmup(16).run(|| {
        step += 1;
        raised
            .harness
            .move_to(start + Vec2::new(40.0 + (step % 16) as f32, 24.0));
        raised.frame();
    });
}

/// Frames with a depth being decided, which is the dearest thing the app draws
/// on any of them.
///
/// A form open on a region raises the tool and puts it together with the model
/// on every frame the depth moves — a whole kernel boolean where every other
/// gate here measures a redraw. Everything it needs is held across frames: the
/// builder, the boolean, the body the tool is raised into and the body the
/// answer is written to, all on the layout beside the mesher. See
/// [`Growing::body`](catcad::internals) — the claim is that a depth typed a
/// digit at a time reaches the heap once, and nothing else checked it.
///
/// Driven through the inbox rather than by dragging the arrow, because that is
/// what the arrow does: one `Set` a frame, and the form decides what the number
/// comes to.
///
/// **The one gate here with a budget, and the two blocks are not the drawing's.**
/// A form open on screen is a `TextEdit` with a placeholder, and a placeholder
/// takes a `Cow<'static, str>` where the string is the form's — so it is cloned
/// once a frame, which `Prompt::beside` argues at the line that does it. Stated
/// exactly rather than loosely: everything the depth itself costs is under it,
/// so a body that stopped being refilled in place reads as three and fails.
#[test]
fn a_frame_deciding_a_depth_allocates_nothing() {
    let mut raised = Raised::new();
    raised.app.ask_for_a_depth(0);
    raised.frame();
    assert_eq!(
        raised.app.deciding_a_depth(),
        Some(0.0),
        "no form is deciding a depth, so this measures an ordinary redraw",
    );

    // One depth before the window, both to build the first body outside it and
    // to say that the number *moves*: a form that stopped taking one would go
    // on reporting zero for a frame that never reaches the kernel.
    raised.app.set_a_depth(1.0);
    raised.frame();
    assert_eq!(
        raised.app.deciding_a_depth(),
        Some(1.0),
        "the form did not take the depth, so nothing is being rebuilt",
    );

    let mut step = 0usize;
    AllocTester::new().warmup(16).budget(2).run(|| {
        step += 1;
        // Never twice the same, so the solid is rebuilt on every frame rather
        // than the layout finding nothing moved and leaving the batch alone.
        raised.app.set_a_depth(1.0 + (step % 16) as f64 / 16.0);
        raised.frame();
    });
}
