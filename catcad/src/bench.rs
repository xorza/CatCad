//! Per-frame allocation gates for the application's record pass, driven by
//! `dhat`.
//!
//! One bench of two steps, both recording real frames through `UiHarness`:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `record-still` | a frame with the pointer parked | the status line it rebuilds |
//! | `record-hovering` | a frame with the pointer moving over the drawing | the above, at the length hovering gives it |
//!
//! Two, because the difference between them is the whole of what pointing at
//! the drawing costs — and because a regression in one and not the other says
//! immediately which half moved.
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

/// A frame with a parked pointer rebuilds the status line and nothing else.
///
/// Not zero, and the reason is the open finding in `.notes/ALLOCATIONS.md`:
/// `CatCad::status` formats a fresh `String` every frame out of a report that
/// only changes on a solve. Drop that and this becomes a strict zero — which
/// is the point of gating it at one rather than at three.
const RECORD_STILL_MAX: f64 = 1.0;

/// Hovering lengthens the status line enough that formatting it grows past
/// what the literal reserved — and that is now the whole of the difference.
/// Aiming at the drawing costs `aperture` nothing: `Scene::nearest` answers
/// without building a list, and the renderer's batches are refilled in place.
const RECORD_HOVERING_MAX: f64 = 2.0;

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("catcad", "frame");

    // Frames with the pointer parked off the drawing.
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    harness.move_to(PARKED);
    bench.step("record-still", RECORD_STILL_MAX, || {
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    });

    // Frames with the pointer walking across the drawing, which is what the
    // app does whenever someone is using it. The sweep is deliberately wide
    // enough to cross geometry and empty space both, so the number is what
    // hovering averages rather than the best or worst case of it.
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    let mut frame = 0usize;
    bench.step("record-hovering", RECORD_HOVERING_MAX, || {
        frame += 1;
        harness.move_to(Vec2::new(700.0 + (frame % 40) as f32, 520.0));
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    });

    bench.finish();
}
