//! Per-frame allocation gates for the application's record pass, driven by
//! `dhat`.
//!
//! One bench of three steps, all recording real frames through `UiHarness`:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `record-still` | a frame with the pointer parked | strict zero |
//! | `record-hovering` | a frame with the pointer moving over the drawing | strict zero |
//! | `record-dragging` | a frame taking a point somewhere | strict zero |
//!
//! All three are zero, and between them that is the whole of a frame:
//! recording is all this crate does per frame, and none of it reaches the heap.
//! The status line is formatted into the record pass's own text arena rather
//! than a `String`; `Scene::nearest` answers a hover without building a list;
//! the drawing is laid out over the primitives the renderer already holds
//! rather than into fresh ones; and the sketch snapshots a dragged frame takes
//! — the solver's, to put back a step the constraints refuse, and the history's
//! two ends of what it is recording — all refill buffers that have the room.
//!
//! Three steps rather than one, because what separates them is what pointing at
//! the drawing costs and what editing it costs — and a regression in one and
//! not the others says immediately which part moved. Dragging is the only one
//! of the three that writes the document, and so the only one that solves,
//! snapshots, and lays the drawing out again.
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
use palantir::{App, WindowToken};
use std::hint::black_box;

use crate::CatCad;

/// The surface every step records at. Large enough that layout does real work
/// rather than collapsing everything to nothing.
const SURFACE: UVec2 = UVec2::new(1600, 1000);

/// Where the pointer sits for the still step — off the drawing, so nothing is
/// hovered and the status line stays at its shortest.
const PARKED: Vec2 = Vec2::new(12.0, 960.0);

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("catcad", "frame");

    // Frames with the pointer parked off the drawing.
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    harness.move_to(PARKED);
    bench.step("record-still", 0.0, || {
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    });

    // Frames with the pointer walking across the drawing, which is what the
    // app does whenever someone is using it. The sweep is deliberately wide
    // enough to cross geometry and empty space both, so the number is what
    // hovering averages rather than the best or worst case of it.
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    let mut frame = 0usize;
    bench.step("record-hovering", 0.0, || {
        frame += 1;
        harness.move_to(Vec2::new(700.0 + (frame % 40) as f32, 520.0));
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    });

    // Frames with a point actually being taken somewhere. The press is landed
    // before the window opens, so what is measured is the middle of a gesture
    // rather than the start of one.
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    harness.frame(|ui| app.record(WindowToken(0), ui));
    let wrist = wrist(&app);
    let grabbed = cursor_on(&mut app, wrist);
    harness.press_at(grabbed);
    harness.frame(|ui| app.record(WindowToken(0), ui));
    let mut frame = 0usize;
    bench.step("record-dragging", 0.0, || {
        frame += 1;
        // Well past palantir's four-pixel latch and never twice in the same
        // place, so the drag stays live and the geometry keeps moving — a drag
        // asked for where it already is settles to a no-op, which is not the
        // frame this is meant to be measuring.
        harness.drag_to(grabbed + Vec2::new(40.0 + (frame % 16) as f32, 24.0));
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    });

    bench.finish();
}

/// The far end of the demo's arm, which is the freest thing it draws and so the
/// one worth dragging. The arm's points are added last, so it is drawn last.
fn wrist(app: &CatCad) -> Vec3 {
    app.renderer()
        .borrow()
        .scene()
        .points
        .last()
        .expect("the demo draws markers")
        .position
}

/// The cursor that aims at `world`, through the camera the app is looking with.
fn cursor_on(app: &mut CatCad, world: Vec3) -> Vec2 {
    let viewport = Viewport::new(SURFACE);
    let clip = app.camera_mut().view_proj(viewport.aspect()) * world.extend(1.0);
    viewport.pixel_from_clip(clip)
}
