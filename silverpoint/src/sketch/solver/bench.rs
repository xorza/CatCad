//! Per-solve allocation gates, driven by `dhat`.
//!
//! One bench of two steps, both over the same fixture — a rectangle with a
//! circle at its centre, eleven parameters against eleven equations — and both
//! through one solver kept alive across the window:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `solve-from-guess` | a full solve from coordinates deliberately off the answer | strict zero |
//! | `solve-converged` | re-solving geometry already at the answer | strict zero |
//!
//! One solver, many solves, is the shape a drag has, and the workspace it
//! keeps means none of them touch the heap. A solver thrown away each time has
//! no workspace to reuse and allocates accordingly — which is a fact about
//! that caller, not about the solver, so there is nothing here to gate.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

use crate::sketch::constraint::Constraint;
use crate::sketch::solver::Solver;
use crate::sketch::{PointId, Sketch};
use common::AllocBench;
use glam::DVec2;
use std::hint::black_box;

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

/// The allocation bench: every step, one profiler, one verdict.
pub fn alloc_bench() {
    let mut bench = AllocBench::start("silverpoint", "solve");

    // Solving from the fixture's own guesses, over and over, through one
    // solver — which is what a drag is, and the only shape the workspace pays
    // for. The sketch is rewound with `set_params` rather than cloned: a clone
    // allocates, and it would land inside the window and be counted as the
    // solver's.
    let mut sketch = fixture();
    let mut guess = Vec::new();
    sketch.write_params(&mut guess);
    let mut solver = Solver::default();
    bench.step("solve-from-guess", 0.0, || {
        sketch.set_params(&guess);
        black_box(solver.solve(&mut sketch));
    });

    // Re-solving a sketch already at its answer, which is what most frames of
    // a drag actually are: the geometry has barely moved since the last one.
    let mut sketch = fixture();
    let mut solver = Solver::default();
    solver.solve(&mut sketch);
    let mut solved = Vec::new();
    sketch.write_params(&mut solved);
    bench.step("solve-converged", 0.0, || {
        sketch.set_params(&solved);
        black_box(solver.solve(&mut sketch));
    });

    bench.finish();
}
