//! What a drag through a drawing costs the heap, a solve at a time.
//!
//! Every gate here runs through one [`Solver`] held across its window, which is
//! the shape a drag has: the constraints are satisfied again on every frame a
//! pointer is down, and the workspace the solver keeps means none of those
//! solves touch the heap.
//!
//! The two drag gates matter most. They are what the application runs every
//! frame a pointer is down, and the only path that walks the parameter vector
//! out and back — twice per call, so that a drag which moved nothing can hand
//! back what it was given. Both walks fill buffers the solver keeps.

use common::AllocTester;
use glam::DVec2;
use silverpoint::{Constraint, Dimension, Drive, Outcome, PointId, Sketch, Snapshot, Solver};
use std::hint::black_box;

/// A rectangle anchored at the origin with a circle at its centre, its
/// coordinates deliberately off the answer.
///
/// The same shape the application opens with, restated here rather than
/// borrowed: a fixture that moves whenever the demo drawing is redecorated is a
/// gate that moves for reasons that are not regressions.
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
    sketch.add_constraint(Constraint::apart(corner[0], corner[1], WIDTH));
    sketch.add_constraint(Constraint::apart(corner[1], corner[2], HEIGHT));

    let hub = sketch.add_point(DVec2::new(3.6, 2.1));
    let hole = sketch.add_circle(hub, 0.9);
    let to_centre = (WIDTH * WIDTH + HEIGHT * HEIGHT).sqrt() * 0.5;
    sketch.add_constraint(Constraint::apart(corner[0], hub, to_centre));
    sketch.add_constraint(Constraint::apart(corner[1], hub, to_centre));
    sketch.add_constraint(Constraint::Radius {
        circle: hole,
        dimension: Dimension::new(1.5),
    });
    sketch
}

/// Where the sketch stands, which is what says whether a drag was taken: one
/// that was moves geometry, and one with nowhere to go leaves every last bit of
/// it alone.
fn taken_down(sketch: &Sketch) -> Snapshot {
    let mut at = Snapshot::default();
    sketch.snapshot_into(&mut at);
    at
}

/// A chain and the end of it, which is what a drag takes hold of.
#[derive(Debug)]
struct Chain {
    sketch: Sketch,
    wrist: PointId,
}

/// A fixed anchor, an elbow five away, a wrist five beyond that.
///
/// Its own fixture because [`fixture`] is determined and no drag on one of its
/// points can move anything — a gate over the drag that *is* taken has to have
/// somewhere to go.
fn chain() -> Chain {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let elbow = sketch.add_point(DVec2::new(5.0, 0.0));
    let wrist = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(anchor);
    sketch.add_segment(anchor, elbow);
    sketch.add_segment(elbow, wrist);
    sketch.add_constraint(Constraint::apart(anchor, elbow, 5.0));
    sketch.add_constraint(Constraint::apart(elbow, wrist, 5.0));
    Chain { sketch, wrist }
}

/// Solving from the fixture's own guesses, over and over, through one solver —
/// which is the only shape the workspace pays for.
///
/// The sketch is rewound from a snapshot rather than cloned: a clone allocates,
/// and it would land inside the window and be counted as the solver's.
#[test]
fn a_solve_from_a_guess_allocates_nothing() {
    let mut sketch = fixture();
    let guess = taken_down(&sketch);
    let mut solver = Solver::default();
    // Kept outside the window with the solver: a solve fills it rather than
    // handing one back, so it is the caller's buffer and pays for itself once.
    let mut outcome = Outcome::default();
    AllocTester::new().run(|| {
        sketch.restore(&guess);
        solver.solve(&mut sketch, &mut outcome);
        black_box(&outcome);
    });
}

/// Re-solving a sketch already at its answer, which is what most frames of a
/// drag actually are: the geometry has barely moved since the last one.
#[test]
fn a_solve_of_settled_geometry_allocates_nothing() {
    let mut sketch = fixture();
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    let settled = taken_down(&sketch);
    AllocTester::new().run(|| {
        sketch.restore(&settled);
        solver.solve(&mut sketch, &mut outcome);
        black_box(&outcome);
    });
}

/// A drag the constraints take: the wrist stays exactly where the cursor put it
/// and the elbow swings to suit.
///
/// Rewound to the settled pose each run, so every one of them is the same drag
/// rather than a chain walking away.
#[test]
fn a_drag_that_is_taken_allocates_nothing() {
    let Chain { mut sketch, wrist } = chain();
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    let settled = taken_down(&sketch);
    // Within reach: √72 from the anchor against an arm that extends to ten.
    let reachable = DVec2::new(6.0, 6.0);
    solver.drag(
        &mut sketch,
        &[Drive::Point(wrist, reachable)],
        &[],
        &mut outcome,
    );
    assert_ne!(
        taken_down(&sketch),
        settled,
        "this gate stopped measuring a drag that is taken"
    );
    AllocTester::new().run(|| {
        sketch.restore(&settled);
        solver.drag(
            &mut sketch,
            &[Drive::Point(wrist, reachable)],
            &[],
            &mut outcome,
        );
        black_box(&outcome);
    });
}

/// A drag with nowhere to go.
///
/// Every point of the fixture is pinned down by its constraints, so the pull
/// finds nothing to yield and the parameter vector is handed back exactly as it
/// arrived — the pull run, the settle after it, and the two walks of the vector
/// that say so.
#[test]
fn a_drag_with_nowhere_to_go_allocates_nothing() {
    let mut sketch = fixture();
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    let corner = sketch
        .points()
        .nth(1)
        .map(|(id, _)| id)
        .expect("the fixture has four corners");
    let nowhere = sketch.point(corner).position + DVec2::splat(0.25);
    let before = taken_down(&sketch);
    solver.drag(
        &mut sketch,
        &[Drive::Point(corner, nowhere)],
        &[],
        &mut outcome,
    );
    assert_eq!(
        taken_down(&sketch),
        before,
        "this gate stopped measuring a drag with nowhere to go"
    );
    // No rewind: a drag with nowhere to go leaves the sketch where it found it.
    AllocTester::new().run(|| {
        solver.drag(
            &mut sketch,
            &[Drive::Point(corner, nowhere)],
            &[],
            &mut outcome,
        );
        black_box(&outcome);
    });
}

/// Reading a sketch nothing has moved, which is what a drawing does after an
/// edit it did not make itself — an undo, or a step replayed.
#[test]
fn measuring_a_settled_sketch_allocates_nothing() {
    let mut sketch = fixture();
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    AllocTester::new().run(|| {
        solver.measure(&sketch, &mut outcome);
        black_box(&outcome);
    });
}
