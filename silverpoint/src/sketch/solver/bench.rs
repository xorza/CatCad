//! Per-solve allocation gates, driven by `dhat`.
//!
//! One bench of two steps, both over the same fixture — a rectangle with a
//! circle at its centre, eleven parameters against eleven equations:
//!
//! | step | measures | limit |
//! |---|---|---|
//! | `solve-from-guess` | a full solve from coordinates deliberately off the answer, through a solver kept alive | strict zero |
//! | `solve-converged` | re-solving geometry already at the answer, through the same | strict zero |
//! | `solve-cold` | the same solve through a solver thrown away each time | a budget |
//!
//! The first two are the shape a drag has: one solver, many solves, and the
//! workspace it keeps means none of them touch the heap. The third is the
//! shape every caller has today — nothing re-solves per frame yet — and it
//! allocates, which is exactly the contrast worth keeping visible.
//!
//! Counts, never times: `dhat::Alloc` taxes every allocation 10-30x, so a
//! duration measured under it says nothing.

use crate::sketch::constraint::Constraint;
use crate::sketch::solver::Solver;
use crate::sketch::{PointId, Sketch};
use common::AllocBench;
use glam::DVec2;
use std::hint::black_box;

/// Nothing: the solver keeps the buffers a solve works in, so the second and
/// every later solve of a sketch refills them rather than rebuilding them.
const FROM_GUESS_MAX: f64 = 0.0;

/// Nothing, for the same reason — and this is the one a drag would pay on
/// every frame, so it is the number that matters.
const CONVERGED_MAX: f64 = 0.0;

/// What a solve costs a caller who does not keep the solver: the workspace is
/// born empty and grows from nothing. A budget rather than zero, and the
/// contrast with the two above is the whole point of the step.
const SOLVE_COLD_MAX: f64 = 32.0;

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
    bench.step("solve-from-guess", FROM_GUESS_MAX, || {
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
    bench.step("solve-converged", CONVERGED_MAX, || {
        sketch.set_params(&solved);
        black_box(solver.solve(&mut sketch));
    });

    // And the same solve through a solver thrown away each time, which is what
    // every caller does today. Kept so the cost the workspace avoids stays
    // visible rather than looking like it never existed.
    let mut sketch = fixture();
    bench.step("solve-cold", SOLVE_COLD_MAX, || {
        sketch.set_params(&guess);
        black_box(Solver::default().solve(&mut sketch));
    });

    bench.finish();
}
