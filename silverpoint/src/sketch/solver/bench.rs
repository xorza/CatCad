//! Per-solve allocation gates, driven by `dhat`.
//!
//! One bench of two steps, both over the same fixture — a rectangle with a
//! circle at its centre, eleven parameters against eleven equations:
//!
//! | step | measures | why |
//! |---|---|---|
//! | `solve-from-guess` | a full solve from coordinates deliberately off the answer | what a first solve costs |
//! | `solve-converged` | re-solving geometry already at the answer | what a *drag* costs, where the sketch has barely moved since the last frame |
//!
//! The second is the one that matters for a frame budget. Nothing re-solves
//! per frame yet, so neither number is a per-frame cost today — but dragging
//! a point is what the solver exists for, and when that lands this is the
//! allocation it will pay on every frame of the drag.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

use crate::sketch::constraint::Constraint;
use crate::sketch::solver::Solver;
use crate::sketch::{PointId, Sketch};
use glam::DVec2;
use std::hint::black_box;

/// Solves per measured window. Enough that an allocation happening on one
/// iteration in ten — a `Vec` doubling, say — is not lost between two
/// snapshots.
const MEASURE: usize = 256;

/// Solves run before the window opens, so one-time growth in any retained
/// scratch is not charged to the steady state.
const WARMUP: usize = 16;

/// Most blocks one solve from a cold guess may allocate.
///
/// A budget rather than zero: the solver builds a dense system per call and
/// nothing in the API lets it keep those buffers between calls. Measured at
/// 28 on the fixture below, so this catches drift rather than presence — and
/// halving it is the open finding in `.notes/ALLOCATIONS.md`.
const FROM_GUESS_MAX: f64 = 32.0;

/// Most blocks re-solving an already-converged sketch may allocate.
///
/// Lower than the above because the iteration loop never runs: what is left
/// is the fixed cost of setting up, which is what a drag would pay per frame.
const CONVERGED_MAX: f64 = 20.0;

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
    /// Warm up, then count what `MEASURE` runs of `body` allocate.
    ///
    /// Too short a warmup errs in the safe direction: leftover growth lands
    /// inside the window and trips the gate rather than hiding under it.
    fn measure(name: &'static str, max: f64, mut body: impl FnMut()) -> Self {
        for _ in 0..WARMUP {
            body();
        }
        let before = dhat::HeapStats::get();
        for _ in 0..MEASURE {
            body();
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
            "  {:<18} {:6} blocks  {:10} bytes  ({:6.2}/solve, limit <= {})",
            self.name,
            self.blocks,
            self.bytes,
            self.blocks_each(),
            self.max,
        );
    }
}

/// A rectangle anchored at the origin with a circle at its centre, its
/// coordinates deliberately off the answer.
///
/// The same shape the application opens with, restated here rather than
/// borrowed: a bench fixture that moves whenever the demo drawing is
/// redecorated is a gate that moves for reasons that are not regressions.
fn fixture() -> Sketch {
    const WIDTH: f64 = 8.0;
    const HEIGHT: f64 = 5.0;

    let mut sketch = Sketch::default();
    let corner: [PointId; 4] = [
        sketch.add_point(DVec2::ZERO),
        sketch.add_point(DVec2::new(7.4, 0.6)),
        sketch.add_point(DVec2::new(8.6, 4.2)),
        sketch.add_point(DVec2::new(-0.5, 5.3)),
    ];
    sketch.fix(corner[0]);
    for pair in [[0, 1], [1, 2], [2, 3], [3, 0]] {
        sketch.add_segment(corner[pair[0]], corner[pair[1]]);
    }
    sketch.add_constraint(Constraint::Horizontal {
        a: corner[0],
        b: corner[1],
    });
    sketch.add_constraint(Constraint::Vertical {
        a: corner[1],
        b: corner[2],
    });
    sketch.add_constraint(Constraint::Horizontal {
        a: corner[2],
        b: corner[3],
    });
    sketch.add_constraint(Constraint::Vertical {
        a: corner[3],
        b: corner[0],
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[0],
        b: corner[1],
        distance: WIDTH,
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[1],
        b: corner[2],
        distance: HEIGHT,
    });

    let hub = sketch.add_point(DVec2::new(3.6, 2.1));
    let hole = sketch.add_circle(hub, 0.9);
    let to_centre = (WIDTH * WIDTH + HEIGHT * HEIGHT).sqrt() * 0.5;
    sketch.add_constraint(Constraint::Distance {
        a: corner[0],
        b: hub,
        distance: to_centre,
    });
    sketch.add_constraint(Constraint::Distance {
        a: corner[1],
        b: hub,
        distance: to_centre,
    });
    sketch.add_constraint(Constraint::Radius {
        circle: hole,
        radius: 1.5,
    });
    sketch
}

/// Solve from the fixture's own guesses, over and over.
///
/// The sketch is rewound with `set_params` rather than cloned: a clone
/// allocates, and it would land inside the window and be counted as the
/// solver's.
fn from_guess() -> Step {
    let mut sketch = fixture();
    let guess = sketch.params();
    Step::measure("solve-from-guess", FROM_GUESS_MAX, || {
        sketch.set_params(&guess);
        black_box(Solver::default().solve(&mut sketch));
    })
}

/// Re-solve a sketch already at its answer — the drag case.
fn converged() -> Step {
    let mut sketch = fixture();
    Solver::default().solve(&mut sketch);
    let solved = sketch.params();
    Step::measure("solve-converged", CONVERGED_MAX, || {
        sketch.set_params(&solved);
        black_box(Solver::default().solve(&mut sketch));
    })
}

/// The allocation bench: every step, one profiler, one verdict.
///
/// Steps run to completion even when an earlier one is over — two numbers
/// localize a regression where one plus an early exit does not.
pub fn alloc_bench(dump: bool) {
    let profiler = profiler(dump);

    println!("silverpoint alloc: measure={MEASURE} solves/step");
    let steps = [from_guess(), converged()];
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
            "FAIL: {} allocates {:.2} blocks/solve, over its limit of {}.",
            step.name,
            step.blocks_each(),
            step.max,
        );
    }
    eprintln!();
    eprintln!("Inspect call sites with:");
    eprintln!("  cargo bench -p silverpoint --bench alloc --features bench -- --dump");
    eprintln!("  open dhat-heap.json at https://nnethercote.github.io/dh_view/");
    std::process::exit(1);
}
