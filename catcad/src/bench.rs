//! Per-frame allocation gates for the application's record pass, driven by
//! `dhat`.
//!
//! One bench of two steps, both recording real frames through `UiHarness`:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `record-still` | a frame with the pointer parked | strict zero |
//! | `record-hovering` | a frame with the pointer moving over the drawing | strict zero |
//!
//! Both are zero, and between them that is the whole of a frame: recording is
//! all this crate does per frame, and none of it reaches the heap. The status
//! line is formatted into the record pass's own text arena rather than a
//! `String`; `Scene::nearest` answers a hover without building a list; and the
//! renderer's batches are refilled in place.
//!
//! Two steps rather than one, because the difference between them is what
//! pointing at the drawing costs — and a regression in one and not the other
//! says immediately which half moved.
//!
//! No GPU: `Ui` records and lays out without one, which is the half of a frame
//! this crate owns. What the renderer does with the result is gated in
//! `aperture`'s own bench, and what palantir does beneath both is gated in
//! palantir's.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

use common::AllocBench;
use glam::{UVec2, Vec2};
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

    bench.finish();
}
