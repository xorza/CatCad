//! Per-frame allocation gates for the application's record pass, driven by
//! `dhat`.
//!
//! One bench of five steps, all recording real frames through `UiHarness`:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `record-still` | a frame with the pointer parked | strict zero |
//! | `record-lifting` | a frame with a control easing under the pointer | strict zero |
//! | `record-hovering` | a frame with the pointer moving over the drawing | strict zero |
//! | `record-dragging` | a frame taking a point somewhere | strict zero |
//! | `record-banding` | a frame with a line half drawn, band following | strict zero |
//!
//! All five are zero, and between them that is the whole of a frame:
//! recording is all this crate does per frame, and none of it reaches the heap.
//! The status line is formatted into the record pass's own text arena rather
//! than a `String`; `Scene::nearest` answers a hover without building a list;
//! the drawing is laid out over the primitives the renderer already holds
//! rather than into fresh ones; what a dragged frame takes down — the solver's
//! parameter vector, so a drag with nowhere to go can be handed back untouched,
//! and the history's two ends of what it is recording — all refill buffers that
//! have the room;
//! and what the curves enclose is worked out in an
//! [`Arrangement`](silverpoint::Arrangement) kept across frames, which refills
//! the list per corner of what leaves it, the list per loop, the list per curve
//! of where it is cut, and the fill per face rather than building each afresh.
//!
//! Four steps rather than one, because what separates them is what each thing
//! the pointer can be doing costs — and a regression in one and not the others
//! says immediately which part moved. Dragging is the only one that writes the
//! document, and so the only one that solves and snapshots; banding is the only
//! one that lays the drawing out for something the document does not hold, and
//! it is what catches a rubber band appended to a batch rather than written
//! into it.
//!
//! **Every step asserts the frame it reached is the frame it names**, which is
//! the one thing a gate on a number cannot check for itself: a step that stops
//! reaching its own frame goes on reporting zero, and reports it about a frame
//! nobody wanted measured. Three of the four had drifted that way at once — a
//! click on chrome the constant no longer pointed at, a press on a point in a
//! sketch the session was not editing, and a sweep that never left the region it
//! started on. All three read as passing.
//!
//! No GPU: `Ui` records and lays out without one, which is the half of a frame
//! this crate owns. What the renderer does with the result is gated in
//! `aperture`'s own bench, and what palantir does beneath both is gated in
//! palantir's.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

use aperture::Viewport;
use common::AllocBench;
use glam::{UVec2, Vec2, Vec3};
use palantir::internals::UiHarness;
use palantir::{App, ResponseState, WidgetId, WindowToken};
use std::hint::black_box;

use crate::CatCad;
use crate::hud::internals;
use crate::tool::Tool;

/// The surface every step records at. Large enough that layout does real work
/// rather than collapsing everything to nothing.
const SURFACE: UVec2 = UVec2::new(1600, 1000);

/// Where the pointer sits for the still step — off the drawing and off the
/// overlay, so nothing is hovered and the status line stays at its shortest.
///
/// Along the top edge between the two surfaces pinned there, which is the one
/// stretch of the view that is neither. The step asserts that nothing in the
/// *drawing* is hovered; a chip under the pointer would pass that and still
/// measure a frame with a tooltip opening in it.
const PARKED: Vec2 = Vec2::new(800.0, 6.0);

/// How wide the hovering step's sweep runs, in pixels, centred on the wrist —
/// and so how many frames it takes to walk one, the step being a pixel.
///
/// Wide enough to leave the arm, so what the window averages is *hovering*
/// rather than one answer held for the whole of it: the near half crosses a
/// point, the edges meeting at it and the region behind them, and the far half
/// crosses nothing. A sweep that stayed on any one of those would measure the
/// cheapest or the dearest case and report it as the middle — which is what the
/// fixed span this replaced did, sitting on one region for every frame of it.
const SWEEP: usize = 120;

/// The app raised on a `SURFACE`-sized view, with one frame behind it.
///
/// One frame in, because everything below aims at something the app has drawn:
/// the camera is settled on the way in, and a cursor worked out before that
/// would aim through a camera the app has since replaced.
///
/// A step takes its own rather than carrying the last one's on, so what each
/// measures is a steady frame of one kind — a window that inherited a latched
/// drag would be measuring two things and reporting one number.
#[derive(Debug)]
struct Raised {
    app: CatCad,
    harness: UiHarness,
}

impl Raised {
    fn new() -> Self {
        let mut raised = Self {
            app: CatCad::build(),
            harness: UiHarness::new(SURFACE),
        };
        // Every step below measures a frame that is *drawing* something, and a
        // document is opened on no sketch — see
        // [`Document::opening`](crate::document::Document).
        raised.app.enter_first_sketch();
        raised.frame();
        raised
    }

    /// One frame, recorded the way the host records one.
    fn frame(&mut self) {
        let Self { app, harness } = self;
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    }

    /// Where a control on the overlay ended up, measured off the frame that
    /// drew it.
    ///
    /// A widget's rect is the layout engine's answer and arrives a frame late,
    /// so this records a frame and reads the *previous* one's placement — which
    /// is why the app is raised with one behind it.
    fn at(&mut self, id: WidgetId) -> Vec2 {
        self.response(id)
            .rect
            .expect("the overlay drew the control asked for")
            .center()
    }

    /// Whether the overlay reports `id` as under the pointer.
    fn hovers(&mut self, id: WidgetId) -> bool {
        self.response(id).hovered
    }

    /// What the overlay last reported about `id`, read out of a fresh frame.
    ///
    /// A rect and a hover are both the layout engine's answer and both arrive a
    /// frame late, so both are read the same way: record a frame, and take what
    /// the *previous* one left.
    fn response(&mut self, id: WidgetId) -> ResponseState {
        let Self { app, harness } = self;
        harness.frame_value(|ui| {
            app.record(WindowToken(0), ui);
            ui.response_for(id)
        })
    }

    /// The cursor that aims at `world`, through the camera the app is looking
    /// with.
    fn cursor_on(&mut self, world: Vec3) -> Vec2 {
        self.app
            .camera_mut()
            .screen_of(world, Viewport::new(SURFACE))
            .expect("the bench aims at what it draws")
    }
}

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("catcad", "frame");
    still(&mut bench);
    lifting(&mut bench);
    hovering(&mut bench);
    dragging(&mut bench);
    banding(&mut bench);
    bench.finish();
}

/// Frames with the pointer parked off the drawing.
fn still(bench: &mut AllocBench) {
    let mut raised = Raised::new();
    raised.harness.move_to(PARKED);
    raised.frame();
    assert!(
        raised.app.view.hovered().is_none(),
        "the parked pointer is over {:?}, so this is a second hovering step",
        raised.app.view.hovered(),
    );
    bench.step("record-still", 0.0, || raised.frame());
}

/// Frames with a control easing between states under the pointer.
///
/// **The one step that measures the overlay rather than the drawing.** A chip
/// eases its fill and its ink toward what it should be wearing, and palantir
/// keeps that against the widget's id — a row created when the target moves and
/// drained when it arrives. Walking between two chips keeps a row live on every
/// frame, which is the case that could reach the heap and the case no other step
/// here touches.
fn lifting(bench: &mut AllocBench) {
    let mut raised = Raised::new();
    let (from, to) = (
        raised.at(internals::tool("Point")),
        raised.at(internals::tool("Circle")),
    );
    // Both ends walked before any of it is measured, so the rows and the typed
    // map palantir keeps them in are asked for outside the window.
    for at in [from, to, from] {
        raised.harness.move_to(at);
        raised.frame();
    }
    // The step reaches the frame it names, which a number alone cannot say: a
    // walk that landed between the chips would go on reporting zero about
    // frames with no control easing in them at all.
    assert!(
        raised.hovers(internals::tool("Point")),
        "the walk missed the chip at {from:?}, so nothing is lifting",
    );
    let mut step = 0usize;
    bench.step("record-lifting", 0.0, || {
        step += 1;
        raised
            .harness
            .move_to(if step.is_multiple_of(2) { from } else { to });
        raised.frame();
    });
}

/// Frames with the pointer walking across the drawing, which is what the app
/// does whenever someone is using it.
fn hovering(bench: &mut AllocBench) {
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
        if raised.app.view.hovered().is_some() {
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
    bench.step("record-hovering", 0.0, || {
        step += 1;
        raised.harness.move_to(middle + across(step % SWEEP));
        raised.frame();
    });
}

/// Frames with a point actually being taken somewhere.
///
/// The press is landed before the window opens, so what is measured is the
/// middle of a gesture rather than the start of one.
fn dragging(bench: &mut AllocBench) {
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
    bench.step("record-dragging", 0.0, || {
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
fn banding(bench: &mut AllocBench) {
    let mut raised = Raised::new();
    let line = raised.at(internals::tool("Line"));
    raised.harness.click_at(line);
    raised.frame();
    assert!(
        raised.app.session.tool().is(Tool::Line { from: None }),
        "the click at {line:?} left {:?} in hand, so it missed the chip",
        raised.app.session.tool(),
    );

    // Clear of the arm, so the first click starts a line rather than putting
    // the tool down on something already drawn.
    let empty = raised.app.wrist() + Vec3::new(0.0, 0.0, 3.0);
    let start = raised.cursor_on(empty);
    raised.harness.click_at(start);
    raised.frame();
    assert!(
        raised.app.session.tool().started().is_some(),
        "no line was begun, so there is no band here to measure",
    );

    let mut step = 0usize;
    bench.step("record-banding", 0.0, || {
        step += 1;
        raised
            .harness
            .move_to(start + Vec2::new(40.0 + (step % 16) as f32, 24.0));
        raised.frame();
    });
}
