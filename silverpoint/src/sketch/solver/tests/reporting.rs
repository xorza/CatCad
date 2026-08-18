//! What a solve says about itself: the freedoms left, the constraints spare,
//! and the work it did.

use crate::sketch::constraint::Constraint;
use crate::sketch::solver::freedom::Freedom;
use crate::sketch::solver::tests::fixtures::*;
use crate::sketch::solver::*;
use glam::DVec2;

/// A solver keeps the buffers a solve works in, so nothing one solve leaves
/// behind may be visible to the next.
///
/// Sizes are the sharp case. Solving a large sketch and then a small one
/// leaves every buffer longer than the small one needs, so a missed `clear` or
/// a length taken from the buffer instead of the sketch shows up here and
/// nowhere else — and the large one again afterwards covers the grow back.
#[test]
fn a_reused_solver_answers_exactly_as_a_fresh_one_would() {
    // What each looks like to a solver that has never seen anything.
    let mut fresh_rectangle = Rectangle::new().sketch;
    let rectangle_outcome = fresh_rectangle.solved();
    let mut fresh_pair = Apart::new().sketch;
    let pair_outcome = fresh_pair.solved();

    // The same two through one solver, largest first. The whole outcome is
    // compared rather than the report alone: the freedoms are keyed by slot and
    // sized from the sketch, so a length taken from a buffer rather than from
    // the geometry shows up there and in nothing else.
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    let mut large = Rectangle::new().sketch;
    solver.solve(&mut large, &mut outcome);
    assert_eq!(outcome, rectangle_outcome);
    // Four parameters against one equation, where the rectangle has six
    // against four.
    let mut small = Apart::new().sketch;
    solver.solve(&mut small, &mut outcome);
    assert_eq!(outcome, pair_outcome, "a smaller sketch after a larger one");
    let mut large_again = Rectangle::new().sketch;
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
    let Doubled {
        mut sketch,
        free,
        stated: [first, second],
    } = Doubled::new();

    let outcome = sketch.solved();

    // Consistent, so it still solves — but two equations share one row of
    // rank, and the extra one is reported rather than silently absorbed.
    assert!(outcome.converged(), "{:?}", outcome);
    assert!((sketch.point(free).position - DVec2::new(5.0, 0.0)).length() < EPSILON);
    assert_eq!(outcome.redundant_constraints(), 1);
    assert_eq!(outcome.degrees_of_freedom(), 1);

    // And the count is broken down to the constraint that carries it. Exactly
    // one of the pair, never both: they are redundant *together*, and the one
    // named is whichever the elimination did not pivot on — so which of the two
    // it is, is not something to assert.
    assert_ne!(
        outcome.is_redundant(first),
        outcome.is_redundant(second),
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
    let apart = sketch.add_constraint(Constraint::apart(anchor, other, 2.0));

    let outcome = sketch.solved();
    assert!(outcome.converged(), "{:?}", outcome);

    // Five equations, rank three: the coincidence pair spends two and the
    // distance one, leaving two rows over. Both of those rows belong to one
    // coincidence, so what the system could do without is *one* constraint, not
    // two equations' worth — and one is what the count says, which is what lets
    // a drawing light a mark per flagged constraint and have the two agree.
    assert_eq!(outcome.redundant_constraints(), 1);
    assert_ne!(outcome.is_redundant(first), outcome.is_redundant(second));
    assert!(
        !outcome.is_redundant(apart),
        "the one constraint holding a point down was called redundant"
    );
    // It is holding it down, which is what says the distance was load-bearing.
    assert!((sketch.point(other).position.length() - 2.0).abs() < EPSILON);
    assert!(
        sketch.point(free).position.length() < EPSILON,
        "the coincidence broke"
    );
    // `other` still slides around its circle; `free` is pinned to the anchor.
    assert_eq!(outcome.degrees_of_freedom(), 1);
}

#[test]
fn conflicting_distances_settle_at_the_least_squares_compromise() {
    let Conflicting { mut sketch, free } = Conflicting::new();

    let outcome = sketch.solved();

    // Minimising (L-1)² + (L-2)² puts L at 1.5, leaving each equation half a
    // unit out. The solve reports failure rather than pretending.
    assert!(!outcome.converged(), "{:?}", outcome);
    assert!((sketch.point(free).position.length() - 1.5).abs() < 1e-8);
    // Two steps: the first takes the point to the compromise, the second finds
    // nothing left to gain there and stops. That second test is the only one
    // that can stop this solve at all — the residual it is told to drive to
    // zero never reaches any tolerance, so without it the iteration grinds on
    // against an answer it already had until the damping gives out, which on
    // this sketch is twenty-four steps for the same result.
    assert_eq!(outcome.iterations(), 2);
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
    let mut outcome = sketch.solved();
    assert!(outcome.converged());
    assert_eq!(outcome.iterations(), 0);
    assert_eq!(outcome.degrees_of_freedom(), 0);
    assert_eq!(outcome.redundant_constraints(), 0);

    // Five parameters, of which the pinned point's two never move: two
    // coordinates and one radius are left, and nothing states a thing about any
    // of them.
    let anchor = sketch.add_point(DVec2::ZERO);
    sketch.fix(anchor);
    let loose = sketch.add_point(DVec2::new(1.0, 2.0));
    let ring = sketch.add_circle(loose, 0.5);
    let mut solver = Solver::default();
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.converged());
    assert_eq!(outcome.iterations(), 0);
    assert_eq!(outcome.degrees_of_freedom(), 3);
    assert_eq!(outcome.redundant_constraints(), 0);

    // Measuring asks the constraints nothing and moves nothing, so it reads the
    // same as the solve that found the sketch already at its answer.
    solver.measure(&sketch, &mut outcome);
    assert_eq!(outcome.degrees_of_freedom(), 3);
    assert_eq!(outcome.point(anchor), Freedom::Determined);
    assert_eq!(outcome.point(loose), Freedom::Free);
    assert_eq!(outcome.circle(ring), Freedom::Free);
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
    let Rectangle {
        sketch: mut rectangle,
        corner,
    } = Rectangle::new();
    solver.solve(&mut rectangle, &mut outcome);
    assert!(outcome.converged());
    for (index, point) in corner.iter().enumerate() {
        assert_eq!(
            outcome.point(*point),
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
    assert!(outcome.converged());
    assert_eq!(outcome.point(loose[0]), Freedom::Determined, "the anchor");
    // Its y is the anchor's and its x is anyone's guess.
    assert_eq!(outcome.point(loose[1]), Freedom::Partly);

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
    assert!(outcome.converged());
    assert_eq!(outcome.point(upright[0]), Freedom::Determined, "the anchor");
    // Its x is the anchor's and its y is anyone's guess.
    assert_eq!(outcome.point(upright[1]), Freedom::Partly);
    assert_eq!(outcome.degrees_of_freedom(), 1);

    // The same one freedom, spent on a curve instead of a line. Both of its
    // coordinates change as it goes round, and it is no freer for that — a
    // cursor that leaves the circle is still asking for the impossible.
    let Orbit {
        sketch: mut orbit,
        rider,
        ring,
    } = Orbit::new();
    solver.solve(&mut orbit, &mut outcome);
    assert!(outcome.converged(), "{:?}", outcome);
    assert_eq!(outcome.degrees_of_freedom(), 1, "the same one freedom");
    assert_eq!(outcome.point(rider), Freedom::Partly);
    // And the radius the constraint named is decided, unlike the rider on it.
    assert_eq!(outcome.circle(ring), Freedom::Determined);

    // A circle nothing sized keeps its rim to be dragged.
    let mut loose_ring = Sketch::default();
    let centre = loose_ring.add_point(DVec2::ZERO);
    loose_ring.fix(centre);
    let free_ring = loose_ring.add_circle(centre, 1.0);
    solver.solve(&mut loose_ring, &mut outcome);
    assert!(outcome.converged());
    assert_eq!(outcome.circle(free_ring), Freedom::Free);
    assert_eq!(outcome.point(centre), Freedom::Determined);
}

/// The freedoms have to agree with the count they break down, entity by entity.
///
/// Holding a point and asking again is the other way to learn the same thing:
/// pinning a coordinate that was already decided costs the sketch nothing,
/// where pinning one it was free to choose spends a degree of freedom.
///
/// Two calculations rather than one asked twice. Both reductions come out of the
/// same elimination, but under different masks: a label is a Gram determinant
/// over the point's two null-space rows, and what pinning it costs is the rank
/// of the whole system with those two columns struck out. So where they agree
/// across sketches of every shape, both are doing what they claim.
#[test]
fn the_freedoms_agree_with_what_holding_each_point_costs() {
    let mut outcome = Outcome::default();
    let mut solver = Solver::default();

    for (name, mut sketch) in [
        ("a determined pair", determined_pair().sketch),
        ("a point on its own circle", Orbit::new().sketch),
        ("a duplicated constraint", Doubled::new().sketch),
        ("conflicting distances", Conflicting::new().sketch),
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
                outcome.point(id),
                expected,
                "{name}: holding this point spent {spent} of the sketch's freedoms"
            );
        }
    }
}
