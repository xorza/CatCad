use super::*;
use crate::sketch::constraint::Constraint;
use crate::sketch::solver::freedoms::Freedom;
use glam::DVec2;

/// Tight enough that a wrong answer can't hide behind it: the solver's own
/// tolerance is on the residual, and these check the geometry that follows.
const EPSILON: f64 = 1e-9;

#[test]
fn distance_moves_a_point_along_its_own_direction() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: free,
        distance: 5.0,
    });

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);

    assert!(outcome.report().converged, "{:?}", outcome.report());
    // The residual's gradient points along b - a, so a point starting on the
    // +x axis can only travel along it: (1,0) becomes exactly (5,0).
    assert!((sketch.point(free).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(
        sketch.point(anchor).position,
        DVec2::ZERO,
        "fixed point moved"
    );
    // One equation against two free parameters: the point may still slide
    // around the circle of radius 5.
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);
}

/// A removal leaves a sketch the solver can still work on, and gives back
/// exactly the freedom it took away.
///
/// The end-to-end check on the hole a removal leaves: the parameters a removed
/// point occupied stay in the vector, and what keeps the solver off them is
/// that they read as unfree. So a solve after a removal has to reach the same
/// geometry as one before it, and the degrees of freedom have to fall by the
/// two the point was carrying rather than by nothing or by everything.
#[test]
fn a_sketch_solves_the_same_once_geometry_and_constraints_are_removed() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let end = sketch.add_point(DVec2::new(1.0, 0.2));
    // Constrained by nothing, so the two freedoms it carries are its own and
    // go when it does.
    let spare = sketch.add_point(DVec2::new(4.0, 4.0));
    sketch.fix(anchor);
    let level = sketch.add_constraint(Constraint::Horizontal { a: anchor, b: end });
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: end,
        distance: 5.0,
    });

    let mut solver = Solver::default();
    let mut outcome = Outcome::default();

    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert!((sketch.point(end).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    // Six parameters, two of them the anchor's and pinned: four free against a
    // rank of two, so the spare point's pair is what is left.
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 2);

    sketch.remove_point(spare);
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());
    // The vector is as wide as it was — the hole keeps its two positions — so
    // this is the same solve reaching the same place with nothing left over.
    assert_eq!(sketch.params().count(), 6);
    assert!((sketch.point(end).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 0);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);

    // Taking the level away hands `end` back the freedom it was spending: it
    // keeps its distance from the anchor and may swing about it.
    sketch.remove_constraint(level);
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);
    assert!((sketch.point(end).position.length() - 5.0).abs() < EPSILON);
}

#[test]
fn the_requested_distance_is_what_changes_the_answer() {
    let solve_for = |distance: f64| {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.0));
        sketch.fix(anchor);
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance,
        });
        let mut outcome = Outcome::default();
        Solver::default().solve(&mut sketch, &mut outcome);
        assert!(outcome.report().converged);
        sketch.point(free).position.x
    };
    assert!((solve_for(5.0) - 5.0).abs() < EPSILON);
    assert!((solve_for(7.0) - 7.0).abs() < EPSILON);
    assert!(solve_for(5.0) != solve_for(7.0));
}

#[test]
fn three_distances_make_a_right_triangle() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(3.2, 0.1));
    let c = sketch.add_point(DVec2::new(0.1, 3.9));
    sketch.fix(a);
    for (first, second, distance) in [(a, b, 3.0), (a, c, 4.0), (b, c, 5.0)] {
        sketch.add_constraint(Constraint::Distance {
            a: first,
            b: second,
            distance,
        });
    }

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());

    let (pa, pb, pc) = (
        sketch.point(a).position,
        sketch.point(b).position,
        sketch.point(c).position,
    );
    assert!(((pb - pa).length() - 3.0).abs() < EPSILON);
    assert!(((pc - pa).length() - 4.0).abs() < EPSILON);
    assert!(((pc - pb).length() - 5.0).abs() < EPSILON);

    // Nothing asked for a right angle — 3² + 4² = 5² produces one, so the
    // corner at `a` squaring up is evidence the solve is genuine.
    assert!((pb - pa).dot(pc - pa).abs() < 1e-8, "{pa:?} {pb:?} {pc:?}");

    // Four free parameters against three independent distances: the triangle
    // can still rotate about the fixed corner.
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);
}

#[test]
fn a_rectangle_is_fully_constrained() {
    let mut sketch = Sketch::default();
    let p0 = sketch.add_point(DVec2::ZERO);
    let p1 = sketch.add_point(DVec2::new(5.1, 0.2));
    let p2 = sketch.add_point(DVec2::new(4.9, 3.1));
    let p3 = sketch.add_point(DVec2::new(0.1, 2.9));
    sketch.fix(p0);
    let bottom = sketch.add_segment(p0, p1);
    let right = sketch.add_segment(p1, p2);
    let top = sketch.add_segment(p2, p3);

    sketch.add_constraint(Constraint::Horizontal { a: p0, b: p1 });
    sketch.add_constraint(Constraint::Distance {
        a: p0,
        b: p1,
        distance: 5.0,
    });
    sketch.add_constraint(Constraint::Perpendicular {
        first: bottom,
        second: right,
    });
    sketch.add_constraint(Constraint::Distance {
        a: p1,
        b: p2,
        distance: 3.0,
    });
    sketch.add_constraint(Constraint::Parallel {
        first: bottom,
        second: top,
    });
    sketch.add_constraint(Constraint::Vertical { a: p0, b: p3 });

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());

    // Six independent equations against six free parameters pin every corner
    // of a 5x3 rectangle with its lower-left at the fixed origin.
    assert!((sketch.point(p1).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert!((sketch.point(p2).position - DVec2::new(5.0, 3.0)).length() < EPSILON);
    assert!((sketch.point(p3).position - DVec2::new(0.0, 3.0)).length() < EPSILON);
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 0);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);
}

/// A coincidence pins both axes, which is the whole of what makes it worth two
/// equations rather than one.
///
/// It is the only constraint the assembler expands — into a `Vertical` and a
/// `Horizontal` — so this is where that expansion has to prove itself. The
/// report is the sharp end: an expansion that dropped an equation would leave
/// a degree of freedom, and one that emitted the same axis twice would leave a
/// degree of freedom *and* a redundancy, while the point below still slid onto
/// the anchor either way.
#[test]
fn a_coincidence_pins_both_axes_and_counts_as_two_equations() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::new(2.0, -1.0));
    // Off in both axes, so neither is satisfied to begin with.
    let free = sketch.add_point(DVec2::new(-3.5, 4.25));
    sketch.fix(anchor);
    sketch.add_constraint(Constraint::Coincident { a: anchor, b: free });

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);

    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert!((sketch.point(free).position - sketch.point(anchor).position).length() < EPSILON);
    // Two free parameters against two independent equations.
    assert_eq!(
        outcome.freedoms().degrees_of_freedom(),
        0,
        "{:?}",
        outcome.report()
    );
    assert_eq!(
        outcome.freedoms().redundant_equations(),
        0,
        "{:?}",
        outcome.report()
    );
}

/// Holding a point pins it where the caller put it and moves the rest of the
/// sketch to suit — which is the whole of what dragging is.
///
/// Without it the solver treats the held point as free and pulls it straight
/// back toward the constraint, so a drag would slip out from under the cursor.
/// The second half of this is that failure, measured.
///
/// The last of it is the drag taken back. What has to come back is not only
/// what the edit touched but what the *solve* moved to accommodate it, and a
/// snapshot is the one thing that holds both — which is why an undo is built on
/// putting a value back rather than on dragging the other way.
#[test]
fn a_held_point_stays_put_and_the_rest_of_the_sketch_follows() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let held = sketch.add_point(DVec2::new(5.0, 0.0));
    let trailing = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(anchor);
    // A chain: anchor — held — trailing, each pair five apart. Holding the
    // middle one leaves the last with somewhere to go.
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: held,
        distance: 5.0,
    });
    sketch.add_constraint(Constraint::Distance {
        a: held,
        b: trailing,
        distance: 5.0,
    });
    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert_eq!(
        outcome.settled(),
        Settled::Freely,
        "a plain solve holds nothing"
    );

    // Drag the middle point straight up. It has to stay exactly there.
    let dragged = DVec2::new(3.0, 4.0);
    sketch.set_point(held, dragged);
    Solver::default().solve_holding(&mut sketch, &[held], &mut outcome);

    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert_eq!(outcome.settled(), Settled::Holding);
    assert_eq!(
        sketch.point(held).position,
        dragged,
        "the held point was moved"
    );
    // 3-4-5 again: the anchor constraint is satisfied where the caller put it,
    // which is why this drag is possible at all.
    assert!((sketch.point(held).position.length() - 5.0).abs() < EPSILON);
    // And the trailing point followed, staying its own five away.
    let span = sketch.point(trailing).position - sketch.point(held).position;
    assert!((span.length() - 5.0).abs() < EPSILON, "{span:?}");

    // The same drag without holding: the solver counts the dragged point as
    // free and satisfies the distance by moving it, so it does not stay.
    sketch.set_point(held, DVec2::new(3.0, 9.0));
    let mut solver = Solver::default();
    solver.solve(&mut sketch, &mut Outcome::default());
    assert_ne!(
        sketch.point(held).position,
        DVec2::new(3.0, 9.0),
        "a free point slides back onto the constraint"
    );

    // And once more through `edit_holding`, which is the call a drag makes.
    // The 3-4-5 the other way round, so the request is reachable — and a
    // reachable one has to be kept.
    let mut before = Snapshot::default();
    sketch.snapshot_into(&mut before);
    let (was_held, was_trailing) = (sketch.point(held).position, sketch.point(trailing).position);
    let sent = DVec2::new(-3.0, 4.0);
    solver.edit_holding(&mut sketch, &[held], &mut outcome, |sketch| {
        sketch.set_point(held, sent)
    });
    assert!(outcome.report().converged, "{:?}", outcome.report());
    // Held throughout: the request was reachable, so the first attempt took it.
    assert_eq!(outcome.settled(), Settled::Holding);
    assert_eq!(
        sketch.point(held).position,
        sent,
        "a reachable edit was undone"
    );
    let span = sketch.point(trailing).position - sketch.point(held).position;
    assert!((span.length() - 5.0).abs() < EPSILON, "{span:?}");

    // An edit that took is an edit there is something to take back, and the
    // snapshot says so. Restoring it returns the trailing point too — nothing
    // asked that one to move, the solve moved it, and an undo that left it
    // where the drag put it would be an undo of half the drag.
    let mut after = Snapshot::default();
    sketch.snapshot_into(&mut after);
    assert_ne!(
        after, before,
        "an edit that moved the sketch snapshots as it was"
    );
    sketch.restore(&before);
    assert_eq!(sketch.point(held).position, was_held);
    assert_eq!(sketch.point(trailing).position, was_trailing);
}

/// Holding a point of a fully-determined sketch asks for a motion its
/// constraints forbid, and the report says so rather than pretending — and the
/// same request made as an edit leaves the sketch exactly as it found it.
///
/// The compromise the first half measures is exactly what an edit must never
/// keep: it satisfies nothing, and it stands up only while the point that
/// caused it is held, so the next solve that holds something else lets go of it
/// and the sketch springs back to where it always had to be.
#[test]
fn holding_a_point_a_determined_sketch_cannot_move_reports_unsolved() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let pinned = sketch.add_point(DVec2::new(5.0, 0.0));
    sketch.fix(anchor);
    sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: pinned,
        distance: 5.0,
    });
    sketch.add_constraint(Constraint::Horizontal {
        a: anchor,
        b: pinned,
    });
    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert!(
        outcome.report().converged && outcome.freedoms().degrees_of_freedom() == 0,
        "{:?}",
        outcome.report()
    );

    // Somewhere the constraints cannot reach with that point held. The anchor
    // is fixed and this one is held, so every column is zeroed and nothing can
    // move at all: the distance is satisfied exactly where it was put — 3-4-5
    // — and the horizontal is out by the whole of its four.
    sketch.set_point(pinned, DVec2::new(3.0, 4.0));
    Solver::default().solve_holding(&mut sketch, &[pinned], &mut outcome);
    assert!(!outcome.report().converged, "{:?}", outcome.report());
    assert_eq!(outcome.settled(), Settled::Holding, "a solve never refuses");
    assert_eq!(outcome.report().max_residual, 4.0, "{:?}", outcome.report());
    assert_eq!(
        sketch.point(pinned).position,
        DVec2::new(3.0, 4.0),
        "{:?}",
        outcome.report()
    );

    // Back to rest, and then the same request as an edit. Nothing of it
    // survives: not the geometry, and not the report either.
    let mut solver = Solver::default();
    solver.solve(&mut sketch, &mut Outcome::default());
    let mut was = Snapshot::default();
    sketch.snapshot_into(&mut was);
    solver.edit_holding(&mut sketch, &[pinned], &mut outcome, |sketch| {
        sketch.set_point(pinned, DVec2::new(3.0, 4.0))
    });

    // Compared as a snapshot rather than as a list of points, because that is
    // the comparison an undo stack makes: an edit that came to nothing has to
    // read as the nothing it was, or it would be recorded as a step to take
    // back.
    //
    // Nothing is what it comes to. Held, the request is impossible; asked
    // again holding nothing, the constraints answer it by putting the point
    // back where it always had to be — which is exactly where it started, so
    // that attempt is not kept either and the sketch is restored.
    let mut now = Snapshot::default();
    sketch.snapshot_into(&mut now);
    assert_eq!(now, was, "an impossible edit moved the sketch");
    // And here is what the report alone cannot say. The sketch it describes is
    // the one that was always there, which is a satisfied sketch — so a
    // refusal reads exactly like a success, down to the iteration count.
    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert_eq!(outcome.report().iterations, 0);
    assert_eq!(
        outcome.settled(),
        Settled::Refused,
        "a refusal read as a solve"
    );
    assert_eq!(
        outcome.freedoms().degrees_of_freedom(),
        0,
        "{:?}",
        outcome.report()
    );
}

/// A point the constraints leave somewhere to go is taken as near what was
/// asked for as it may go, rather than not moving at all.
///
/// The other side of holding. Held, the grabbed point may not move at all, so a
/// point tied to an edge could never be dragged: a cursor is never *exactly* on
/// the line, and the edge cannot come to meet it. Asked again holding nothing,
/// the same request slides the point along the edge to the nearest place the
/// constraints allow — which is the foot of the perpendicular from where the
/// cursor asked.
#[test]
fn a_drag_the_constraints_cannot_take_exactly_lands_as_near_as_they_allow() {
    let mut sketch = Sketch::default();
    let left = sketch.add_point(DVec2::ZERO);
    let right = sketch.add_point(DVec2::new(10.0, 0.0));
    sketch.fix(left);
    sketch.fix(right);
    let edge = sketch.add_segment(left, right);
    let sliding = sketch.add_point(DVec2::new(3.0, 0.0));
    sketch.add_constraint(Constraint::PointOnSegment {
        point: sliding,
        segment: edge,
    });

    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged);
    assert_eq!(
        outcome.freedoms().degrees_of_freedom(),
        1,
        "the point should be free along the edge and nowhere else"
    );

    // Two and a half units off the line, which is where a cursor always is.
    // The edge runs along y = 0, so the nearest place on it is straight below.
    let asked = DVec2::new(7.0, 2.5);
    solver.edit_holding(&mut sketch, &[sliding], &mut outcome, |sketch| {
        sketch.set_point(sliding, asked)
    });
    assert!(outcome.report().converged, "{:?}", outcome.report());
    // Held, the cursor asked for somewhere off the edge and the constraints
    // refused; freed, they answered with the nearest place on it. That second
    // attempt is the one kept, and it is the one this names.
    assert_eq!(outcome.settled(), Settled::Freely);

    let landed = sketch.point(sliding).position;
    assert!(
        (landed - DVec2::new(7.0, 0.0)).length() < EPSILON,
        "asked for {asked:?} and landed {landed:?}, not the foot of the perpendicular"
    );
    // And the edge it slid along did not come to meet it: both ends are pinned,
    // so a solve that had moved them would be answering a different question.
    assert_eq!(sketch.point(left).position, DVec2::ZERO);
    assert_eq!(sketch.point(right).position, DVec2::new(10.0, 0.0));
}

/// Holding nothing is solving, exactly — the general entry point cannot drift
/// from the one every caller uses.
#[test]
fn holding_nothing_is_the_same_solve() {
    let build = || {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.5));
        sketch.fix(anchor);
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance: 5.0,
        });
        sketch
    };
    let mut plain = build();
    let mut empty_hold = build();
    let mut plainly = Outcome::default();
    Solver::default().solve(&mut plain, &mut plainly);
    let mut holding_nothing = Outcome::default();
    Solver::default().solve_holding(&mut empty_hold, &[], &mut holding_nothing);
    assert_eq!(plainly, holding_nothing);
    let moved: Vec<DVec2> = plain.points().map(|(_, at)| at.position).collect();
    let same: Vec<DVec2> = empty_hold.points().map(|(_, at)| at.position).collect();
    assert_eq!(moved, same);
}

/// A solver keeps the buffers a solve works in, so nothing one solve leaves
/// behind may be visible to the next.
///
/// Sizes are the sharp case. Solving a large sketch and then a small one
/// leaves every buffer longer than the small one needs, so a missed `clear` or
/// a length taken from the buffer instead of the sketch shows up here and
/// nowhere else — and the large one again afterwards covers the grow back.
#[test]
fn a_reused_solver_answers_exactly_as_a_fresh_one_would() {
    fn rectangle() -> Sketch {
        let mut sketch = Sketch::default();
        let corner = [
            sketch.add_point(DVec2::ZERO),
            sketch.add_point(DVec2::new(5.1, 0.2)),
            sketch.add_point(DVec2::new(4.9, 3.1)),
        ];
        sketch.fix(corner[0]);
        sketch.add_constraint(Constraint::Horizontal {
            a: corner[0],
            b: corner[1],
        });
        sketch.add_constraint(Constraint::Distance {
            a: corner[0],
            b: corner[1],
            distance: 5.0,
        });
        sketch.add_constraint(Constraint::Vertical {
            a: corner[1],
            b: corner[2],
        });
        sketch.add_constraint(Constraint::Distance {
            a: corner[1],
            b: corner[2],
            distance: 3.0,
        });
        sketch
    }
    // Four parameters against one equation, where the rectangle has six
    // against four.
    fn pair() -> Sketch {
        let mut sketch = Sketch::default();
        let anchor = sketch.add_point(DVec2::ZERO);
        let free = sketch.add_point(DVec2::new(1.0, 0.0));
        sketch.fix(anchor);
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance: 5.0,
        });
        sketch
    }

    // What each looks like to a solver that has never seen anything.
    let mut fresh_rectangle = rectangle();
    let mut rectangle_outcome = Outcome::default();
    Solver::default().solve(&mut fresh_rectangle, &mut rectangle_outcome);
    let mut fresh_pair = pair();
    let mut pair_outcome = Outcome::default();
    Solver::default().solve(&mut fresh_pair, &mut pair_outcome);

    // The same two through one solver, largest first. The whole outcome is
    // compared rather than the report alone: the freedoms are keyed by slot and
    // sized from the sketch, so a length taken from a buffer rather than from
    // the geometry shows up there and in nothing else.
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    let mut large = rectangle();
    solver.solve(&mut large, &mut outcome);
    assert_eq!(outcome, rectangle_outcome);
    let mut small = pair();
    solver.solve(&mut small, &mut outcome);
    assert_eq!(outcome, pair_outcome, "a smaller sketch after a larger one");
    let mut large_again = rectangle();
    solver.solve(&mut large_again, &mut outcome);
    assert_eq!(outcome, rectangle_outcome, "and larger again after that");

    // The same reports, and the same geometry behind them.
    for (reused, fresh) in [
        (&large, &fresh_rectangle),
        (&small, &fresh_pair),
        (&large_again, &fresh_rectangle),
    ] {
        let moved: Vec<DVec2> = reused.points().map(|(_, at)| at.position).collect();
        let expected: Vec<DVec2> = fresh.points().map(|(_, at)| at.position).collect();
        assert_eq!(moved, expected);
    }
}

#[test]
fn a_duplicate_constraint_is_reported_as_redundant() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    let distance = Constraint::Distance {
        a: anchor,
        b: free,
        distance: 5.0,
    };
    let first = sketch.add_constraint(distance);
    let second = sketch.add_constraint(distance);

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);

    // Consistent, so it still solves — but two equations share one row of
    // rank, and the extra one is reported rather than silently absorbed.
    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert!((sketch.point(free).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(outcome.freedoms().redundant_equations(), 1);
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);

    // And the count is broken down to the constraint that carries it. Exactly
    // one of the pair, never both: they are redundant *together*, and the one
    // named is whichever the elimination did not pivot on — so which of the two
    // it is, is not something to assert.
    assert_ne!(
        outcome.freedoms().is_redundant(first),
        outcome.freedoms().is_redundant(second),
        "a duplicated pair should name one of its members, not both or neither"
    );
}

/// Redundancy is reported per constraint rather than per equation, and only
/// where the system really has a spare.
///
/// Two coincidences over one pair of points make four equations against a rank
/// of two, so *two* rows die — and both belong to whichever coincidence lost, so
/// it is named once rather than twice. The distance beside them is needed, and
/// is left alone.
///
/// The sharp half is the distance. Its row is the last one assembled and the
/// elimination swaps it forward to pivot on it, so a run that did not carry the
/// row permutation alongside the swap would read the dead rows off the wrong
/// equations and blame the one constraint here that is doing work.
#[test]
fn redundancy_names_constraints_and_leaves_the_needed_ones_alone() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.5));
    let other = sketch.add_point(DVec2::new(3.0, 0.0));
    sketch.fix(anchor);
    let together = Constraint::Coincident { a: anchor, b: free };
    let first = sketch.add_constraint(together);
    let second = sketch.add_constraint(together);
    let apart = sketch.add_constraint(Constraint::Distance {
        a: anchor,
        b: other,
        distance: 2.0,
    });

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());

    // Five equations, rank three: the coincidence pair spends two and the
    // distance one, leaving two rows over.
    assert_eq!(outcome.freedoms().redundant_equations(), 2);
    // Both of those rows belong to one coincidence, which is named once.
    assert_ne!(
        outcome.freedoms().is_redundant(first),
        outcome.freedoms().is_redundant(second)
    );
    assert!(
        !outcome.freedoms().is_redundant(apart),
        "the one constraint holding a point down was called redundant"
    );
    // It is holding it down, which is what says the distance was load-bearing.
    assert!((sketch.point(other).position.length() - 2.0).abs() < EPSILON);
    assert!(
        sketch.point(free).position.length() < EPSILON,
        "the coincidence broke"
    );
    // `other` still slides around its circle; `free` is pinned to the anchor.
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);
}

#[test]
fn conflicting_distances_settle_at_the_least_squares_compromise() {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    for distance in [1.0, 2.0] {
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance,
        });
    }

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);

    // Minimising (L-1)² + (L-2)² puts L at 1.5, leaving each equation half a
    // unit out. The solve reports failure rather than pretending.
    assert!(!outcome.report().converged, "{:?}", outcome.report());
    assert!((sketch.point(free).position.length() - 1.5).abs() < 1e-8);
    // Two steps: the first takes the point to the compromise, the second finds
    // nothing left to gain there and stops. That second test is the only one
    // that can stop this solve at all — the residual it is told to drive to
    // zero never reaches any tolerance, so without it the iteration grinds on
    // against an answer it already had until the damping gives out, which on
    // this sketch is twenty-four steps for the same result.
    assert_eq!(outcome.report().iterations, 2);
    assert!(
        (outcome.report().max_residual - 0.5).abs() < 1e-8,
        "{:?}",
        outcome.report()
    );
}

#[test]
fn a_circle_solves_its_radius_and_the_point_on_it_together() {
    let mut sketch = Sketch::default();
    let center = sketch.add_point(DVec2::ZERO);
    let rim = sketch.add_point(DVec2::new(3.0, 0.5));
    sketch.fix(center);
    let circle = sketch.add_circle(center, 1.0);
    sketch.add_constraint(Constraint::Radius {
        circle,
        radius: 2.0,
    });
    sketch.add_constraint(Constraint::PointOnCircle { point: rim, circle });

    let start = sketch.point(rim).position;
    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());

    assert!((sketch.circle(circle).radius - 2.0).abs() < EPSILON);
    assert!((sketch.point(rim).position.length() - 2.0).abs() < EPSILON);
    // The point is only ever pushed radially, so it lands on its own ray.
    assert!((sketch.point(rim).position.normalize() - start.normalize()).length() < EPSILON);
    // Three free parameters (the point, the radius) against two equations:
    // the point can still travel around the circle.
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);
}

#[test]
fn point_on_segment_slides_onto_the_line_without_moving_it() {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(4.0, 0.0));
    let stray = sketch.add_point(DVec2::new(2.0, 1.5));
    sketch.fix(a);
    sketch.fix(b);
    let segment = sketch.add_segment(a, b);
    sketch.add_constraint(Constraint::PointOnSegment {
        point: stray,
        segment,
    });

    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());

    // Both endpoints are fixed, so the line can't come to the point: the
    // point drops straight down onto y = 0, keeping its x.
    assert!((sketch.point(stray).position - DVec2::new(2.0, 0.0)).length() < EPSILON);
    assert_eq!(sketch.point(b).position, DVec2::new(4.0, 0.0));
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);
}

/// A sketch nothing has been said about, with and without geometry in it.
///
/// The half with geometry is the one that bites: there is no Jacobian to
/// eliminate, so the freedoms cannot be read off a rank at all and every
/// parameter a solve could move has to come back free. Answering it the other
/// way — nothing to eliminate, therefore nothing undecided — is exactly
/// backwards, and reads as a fully constrained drawing that refuses to be
/// dragged.
#[test]
fn a_sketch_with_no_constraints_is_solved_and_everything_it_can_move_is_free() {
    let mut sketch = Sketch::default();
    let mut outcome = Outcome::default();
    Solver::default().solve(&mut sketch, &mut outcome);
    assert_eq!(
        outcome.report(),
        SolveReport {
            converged: true,
            iterations: 0,
            max_residual: 0.0,
        }
    );
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 0);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);

    // Five parameters, of which the pinned point's two never move: two
    // coordinates and one radius are left, and nothing states a thing about any
    // of them.
    let anchor = sketch.add_point(DVec2::ZERO);
    sketch.fix(anchor);
    let loose = sketch.add_point(DVec2::new(1.0, 2.0));
    let ring = sketch.add_circle(loose, 0.5);
    let mut solver = Solver::default();
    solver.solve(&mut sketch, &mut outcome);
    assert_eq!(
        outcome.report(),
        SolveReport {
            converged: true,
            iterations: 0,
            max_residual: 0.0,
        }
    );
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 3);
    assert_eq!(outcome.freedoms().redundant_equations(), 0);
    assert_eq!(outcome.settled(), Settled::Freely);

    // Measuring asks the constraints nothing, so it settles nothing — and says
    // so, where a report of the same sketch would read the same either way.
    solver.measure(&sketch, &mut outcome);
    assert_eq!(outcome.settled(), Settled::AtRest);
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 3);
    assert_eq!(outcome.freedoms().point(anchor), Freedom::Determined);
    assert_eq!(outcome.freedoms().point(loose), Freedom::Free);
    assert_eq!(outcome.freedoms().radius(ring), Freedom::Free);
    // And nothing moved: a solve with no equations has no step to take.
    assert_eq!(sketch.point(loose).position, DVec2::new(1.0, 2.0));
    assert_eq!(sketch.circle(ring).radius, 0.5);
}

/// Which geometry the constraints pin down, and which they leave something to
/// decide.
///
/// The pair that matters is the last two, and what matters is that they agree.
/// A point riding a horizontal line and a point riding a circle are equally
/// constrained — each has one way to go — and both read `Partly`, though the
/// first keeps a coordinate still and the second changes both as it travels.
/// Counting coordinates rather than directions would separate them, and would
/// then have to call a point on a diagonal free.
#[test]
fn freedoms_name_which_geometry_the_constraints_leave_undecided() {
    let mut outcome = Outcome::default();
    let mut solver = Solver::default();

    // Six independent equations against six free parameters: every corner has
    // exactly one place it can be, and the anchor never had a choice.
    let mut rectangle = Sketch::default();
    let corner = [
        rectangle.add_point(DVec2::ZERO),
        rectangle.add_point(DVec2::new(5.1, 0.2)),
        rectangle.add_point(DVec2::new(4.9, 3.1)),
    ];
    rectangle.fix(corner[0]);
    rectangle.add_constraint(Constraint::Horizontal {
        a: corner[0],
        b: corner[1],
    });
    rectangle.add_constraint(Constraint::Distance {
        a: corner[0],
        b: corner[1],
        distance: 5.0,
    });
    rectangle.add_constraint(Constraint::Vertical {
        a: corner[1],
        b: corner[2],
    });
    rectangle.add_constraint(Constraint::Distance {
        a: corner[1],
        b: corner[2],
        distance: 3.0,
    });
    solver.solve(&mut rectangle, &mut outcome);
    assert!(outcome.report().converged);
    for (index, point) in corner.iter().enumerate() {
        assert_eq!(
            outcome.freedoms().point(*point),
            Freedom::Determined,
            "corner {index} was left something to decide"
        );
    }

    // The same rectangle with its width released: two of its corners can now
    // travel, and the third still cannot.
    let mut stretchy = Sketch::default();
    let loose = [
        stretchy.add_point(DVec2::ZERO),
        stretchy.add_point(DVec2::new(5.0, 0.0)),
    ];
    stretchy.fix(loose[0]);
    stretchy.add_constraint(Constraint::Horizontal {
        a: loose[0],
        b: loose[1],
    });
    solver.solve(&mut stretchy, &mut outcome);
    assert!(outcome.report().converged);
    assert_eq!(
        outcome.freedoms().point(loose[0]),
        Freedom::Determined,
        "the anchor"
    );
    // Its y is the anchor's and its x is anyone's guess.
    assert_eq!(outcome.freedoms().point(loose[1]), Freedom::Partly);

    // The same one freedom released the other way about. Not a mirror of the
    // above as far as the elimination is concerned: there the free coordinate
    // is the column the single row pivots on last, and here the row pivots
    // first and the free coordinate is what is left over once every row has.
    // Both have to come back as one direction to go in.
    let mut tall = Sketch::default();
    let upright = [
        tall.add_point(DVec2::ZERO),
        tall.add_point(DVec2::new(0.0, 5.0)),
    ];
    tall.fix(upright[0]);
    tall.add_constraint(Constraint::Vertical {
        a: upright[0],
        b: upright[1],
    });
    solver.solve(&mut tall, &mut outcome);
    assert!(outcome.report().converged);
    assert_eq!(
        outcome.freedoms().point(upright[0]),
        Freedom::Determined,
        "the anchor"
    );
    // Its x is the anchor's and its y is anyone's guess.
    assert_eq!(outcome.freedoms().point(upright[1]), Freedom::Partly);
    assert_eq!(outcome.freedoms().degrees_of_freedom(), 1);

    // The same one freedom, spent on a curve instead of a line. Both of its
    // coordinates change as it goes round, and it is no freer for that — a
    // cursor that leaves the circle is still asking for the impossible.
    let mut orbit = Sketch::default();
    let hub = orbit.add_point(DVec2::ZERO);
    let rider = orbit.add_point(DVec2::new(2.0, 0.5));
    orbit.fix(hub);
    let ring = orbit.add_circle(hub, 2.0);
    orbit.add_constraint(Constraint::Radius {
        circle: ring,
        radius: 2.0,
    });
    orbit.add_constraint(Constraint::PointOnCircle {
        point: rider,
        circle: ring,
    });
    solver.solve(&mut orbit, &mut outcome);
    assert!(outcome.report().converged, "{:?}", outcome.report());
    assert_eq!(
        outcome.freedoms().degrees_of_freedom(),
        1,
        "the same one freedom"
    );
    assert_eq!(outcome.freedoms().point(rider), Freedom::Partly);
    // And the radius the constraint named is decided, unlike the rider on it.
    assert_eq!(outcome.freedoms().radius(ring), Freedom::Determined);

    // A circle nothing sized keeps its rim to be dragged.
    let mut loose_ring = Sketch::default();
    let centre = loose_ring.add_point(DVec2::ZERO);
    loose_ring.fix(centre);
    let free_ring = loose_ring.add_circle(centre, 1.0);
    solver.solve(&mut loose_ring, &mut outcome);
    assert!(outcome.report().converged);
    assert_eq!(outcome.freedoms().radius(free_ring), Freedom::Free);
    assert_eq!(outcome.freedoms().point(centre), Freedom::Determined);
}

/// The freedoms have to agree with the count they break down, entity by entity.
///
/// Holding a point and asking again is the other way to learn the same thing:
/// pinning a coordinate that was already decided costs the sketch nothing,
/// where pinning one it was free to choose spends a degree of freedom. That is
/// a different route through the solver — the count comes from `free_params`
/// against the rank, the labels from the reduced elimination — so where the two
/// agree across sketches of every shape, both are doing what they claim.
#[test]
fn the_freedoms_agree_with_what_holding_each_point_costs() {
    let mut outcome = Outcome::default();
    let mut solver = Solver::default();

    for (name, mut sketch) in [
        ("a determined rectangle", determined_rectangle()),
        ("a point on its own circle", point_on_a_circle()),
        ("a duplicated constraint", duplicated_distance()),
        ("conflicting distances", conflicting_distances()),
    ] {
        solver.solve(&mut sketch, &mut outcome);
        let at_rest = solver.freedom_holding(&sketch, &[]);

        for (id, _) in sketch.points().collect::<Vec<_>>() {
            let spent = at_rest - solver.freedom_holding(&sketch, &[id]);
            let expected = match spent {
                0 => Freedom::Determined,
                1 => Freedom::Partly,
                _ => Freedom::Free,
            };
            assert_eq!(
                outcome.freedoms().point(id),
                expected,
                "{name}: holding this point spent {spent} of the sketch's freedoms"
            );
        }
    }
}

fn determined_rectangle() -> Sketch {
    let mut sketch = Sketch::default();
    let a = sketch.add_point(DVec2::ZERO);
    let b = sketch.add_point(DVec2::new(5.1, 0.2));
    sketch.fix(a);
    sketch.add_constraint(Constraint::Horizontal { a, b });
    sketch.add_constraint(Constraint::Distance {
        a,
        b,
        distance: 5.0,
    });
    sketch
}

fn point_on_a_circle() -> Sketch {
    let mut sketch = Sketch::default();
    let hub = sketch.add_point(DVec2::ZERO);
    let rider = sketch.add_point(DVec2::new(2.0, 0.5));
    sketch.fix(hub);
    let ring = sketch.add_circle(hub, 2.0);
    sketch.add_constraint(Constraint::Radius {
        circle: ring,
        radius: 2.0,
    });
    sketch.add_constraint(Constraint::PointOnCircle {
        point: rider,
        circle: ring,
    });
    sketch
}

fn duplicated_distance() -> Sketch {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    let distance = Constraint::Distance {
        a: anchor,
        b: free,
        distance: 5.0,
    };
    sketch.add_constraint(distance);
    sketch.add_constraint(distance);
    sketch
}

fn conflicting_distances() -> Sketch {
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    let free = sketch.add_point(DVec2::new(1.0, 0.0));
    sketch.fix(anchor);
    for distance in [1.0, 2.0] {
        sketch.add_constraint(Constraint::Distance {
            a: anchor,
            b: free,
            distance,
        });
    }
    sketch
}
