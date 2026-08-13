//! Per-frame allocation gates for the application's record pass, driven by
//! `dhat`.
//!
//! One bench of two steps, both recording real frames through `UiHarness`:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `record-still` | a frame with the pointer parked | the status line it rebuilds |
//! | `record-hovering` | a frame with the pointer moving over the drawing | the above, plus the pick's answer |
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

use glam::{UVec2, Vec2};
use palantir::internals::UiHarness;
use palantir::{App, WindowToken};
use std::hint::black_box;

use crate::CatCad;

/// Frames per measured window. Enough that an allocation happening on one
/// frame in ten — a `Vec` doubling, say — is not lost between two snapshots.
const MEASURE: usize = 256;

/// Frames recorded before the window opens, so one-time growth in palantir's
/// caches and our own is not charged to the steady state.
const WARMUP: usize = 16;

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

/// Hovering adds the pick's answer, and lengthens the status line enough that
/// formatting it grows past what the literal reserved.
const RECORD_HOVERING_MAX: f64 = 4.0;

fn profiler(dump: bool) -> dhat::Profiler {
    if dump {
        dhat::Profiler::new_heap()
    } else {
        dhat::Profiler::builder().testing().build()
    }
}

/// One step's measured window.
#[derive(Debug, Clone, Copy)]
struct Step {
    name: &'static str,
    blocks: u64,
    bytes: u64,
    max: f64,
}

impl Step {
    /// Warm up, then count what `MEASURE` frames allocate.
    ///
    /// Too short a warmup errs in the safe direction: leftover growth lands
    /// inside the window and trips the gate rather than hiding under it.
    fn measure(name: &'static str, max: f64, mut frame: impl FnMut(usize)) -> Self {
        for i in 0..WARMUP {
            frame(i);
        }
        let before = dhat::HeapStats::get();
        for i in 0..MEASURE {
            frame(i);
        }
        let after = dhat::HeapStats::get();
        Self {
            name,
            blocks: after.total_blocks - before.total_blocks,
            bytes: after.total_bytes - before.total_bytes,
            max,
        }
    }

    fn blocks_each(&self) -> f64 {
        self.blocks as f64 / MEASURE as f64
    }

    /// Blocks alone — `dhat` only ever adds to `total_bytes` alongside
    /// `total_blocks`, so a byte check could never fire on its own.
    fn over(&self) -> bool {
        self.blocks_each() > self.max
    }

    fn report(&self) {
        println!(
            "  {:<18} {:6} blocks  {:10} bytes  ({:6.2}/frame, limit <= {})",
            self.name,
            self.blocks,
            self.bytes,
            self.blocks_each(),
            self.max,
        );
    }
}

/// Frames with the pointer parked off the drawing.
fn record_still() -> Step {
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    harness.move_to(PARKED);
    Step::measure("record-still", RECORD_STILL_MAX, |_| {
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    })
}

/// Frames with the pointer walking across the drawing, which is what the app
/// does whenever someone is using it.
///
/// The sweep is deliberately wide enough to cross geometry and empty space
/// both, so the number is what hovering actually averages rather than the
/// best or worst case of it.
fn record_hovering() -> Step {
    let mut app = CatCad::build();
    let mut harness = UiHarness::new(SURFACE);
    Step::measure("record-hovering", RECORD_HOVERING_MAX, |frame| {
        harness.move_to(Vec2::new(700.0 + (frame % 40) as f32, 520.0));
        black_box(harness.frame(|ui| app.record(WindowToken(0), ui)));
    })
}

/// The allocation bench: every step, one profiler, one verdict.
///
/// Steps run to completion even when an earlier one is over — two numbers
/// localize a regression where one plus an early exit does not.
pub fn alloc_bench(dump: bool) {
    let profiler = profiler(dump);

    println!("catcad alloc: measure={MEASURE} frames/step");
    let steps = [record_still(), record_hovering()];
    for step in &steps {
        step.report();
    }

    // Before any exit: `process::exit` skips `Drop`, and dropping is what
    // writes `dhat-heap.json` under `--dump`.
    drop(profiler);

    let over: Vec<&Step> = steps.iter().filter(|step| step.over()).collect();
    if over.is_empty() {
        println!("PASS: every allocation gate held.");
        return;
    }
    eprintln!();
    for step in over {
        eprintln!(
            "FAIL: {} allocates {:.2} blocks/frame, over its limit of {}.",
            step.name,
            step.blocks_each(),
            step.max,
        );
    }
    eprintln!();
    eprintln!("Inspect call sites with:");
    eprintln!("  cargo bench -p catcad --bench alloc --features bench -- --dump");
    eprintln!("  open dhat-heap.json at https://nnethercote.github.io/dh_view/");
    std::process::exit(1);
}
