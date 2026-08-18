//! What a drag does — what it holds still, what it carries, and what it
//! refuses.

use crate::Snapshot;
use crate::sketch::constraint::Constraint;
use crate::sketch::solver::tests::fixtures::*;
use crate::sketch::solver::*;
use glam::DVec2;

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
    sketch.add_constraint(Constraint::apart(anchor, held, 5.0));
    sketch.add_constraint(Constraint::apart(held, trailing, 5.0));
    let mut outcome = sketch.solved();
    assert!(outcome.converged(), "{outcome:?}");

    // Drag the middle point straight up. It has to stay exactly there.
    let dragged = DVec2::new(3.0, 4.0);
    Solver::default().drag(
        &mut sketch,
        &[Drive::Point(held, dragged)],
        &[],
        &mut outcome,
    );

    assert!(outcome.converged(), "{:?}", outcome);
    // Reached rather than written. A drag pulls toward the cursor *through* the
    // constraints, so where it arrives is the solver's answer and carries the
    // solver's tolerance — where writing the position and pinning it landed on
    // the bit and left the constraints to be checked afterwards.
    assert!(
        (sketch.point(held).position - dragged).length() < EPSILON,
        "the held point was moved: {:?}",
        sketch.point(held).position
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

    // And once more through `drag`, which is the call a gesture makes.
    // The 3-4-5 the other way round, so the request is reachable — and a
    // reachable one has to be kept.
    let mut before = Snapshot::default();
    sketch.snapshot_into(&mut before);
    let (was_held, was_trailing) = (sketch.point(held).position, sketch.point(trailing).position);
    let sent = DVec2::new(-3.0, 4.0);
    solver.drag(&mut sketch, &[Drive::Point(held, sent)], &[], &mut outcome);
    assert!(outcome.converged(), "{:?}", outcome);
    // Reached, because the request was one the constraints allow: there the
    // pull and the constraints go to zero together and the drag lands on what
    // was asked for, to the solver's tolerance.
    assert!(
        (sketch.point(held).position - sent).length() < EPSILON,
        "a reachable edit was undone: {:?}",
        sketch.point(held).position
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

/// A drag the constraints leave nowhere to go moves nothing — not the point it
/// has hold of, and not anything hanging off it.
///
/// The whole of what pulling buys over writing. Writing the point where the
/// cursor asked put the sketch somewhere its constraints were badly broken and
/// then asked least squares to tidy up: the tidying split the correction between
/// the grabbed point and whatever it was tied to, dragged the grabbed point all
/// the way home again — its own constraints admitted nothing else — and left the
/// neighbour holding its share, with nothing left to pull that back. Every
/// frame, a fresh way. Pulling never leaves the constraints in the first place,
/// so there is no correction to split and nothing to leave behind.
///
/// Two fixtures, because the fault needs somewhere to leak *to*. The first is
/// pinned down to its last parameter and could only ever have shown the point
/// itself staying put; the second hangs a free arm off that point, which is
/// where the leak went.
#[test]
fn a_drag_the_constraints_leave_nowhere_to_go_moves_nothing() {
    // There is one place this point can be, and the cursor gets no say in it.
    let Apart {
        mut sketch,
        free: pinned,
        ..
    } = determined_pair();
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();
    solver.solve(&mut sketch, &mut outcome);
    assert!(
        outcome.converged() && outcome.degrees_of_freedom() == 0,
        "{outcome:?}"
    );

    // Somewhere it cannot reach: the distance would be satisfied at 3-4-5 and
    // the level broken by the whole of its four.
    solver.drag(
        &mut sketch,
        &[Drive::Point(pinned, DVec2::new(3.0, 4.0))],
        &[],
        &mut outcome,
    );
    let at = sketch.point(pinned).position;
    assert!(
        (at - DVec2::new(5.0, 0.0)).length() < EPSILON,
        "a drag with nowhere to go moved the point: {at:?}"
    );
    // Within the solve's own tolerance rather than to the bit, and that is the
    // contract: a drag is answered by pulling through the constraints, so where
    // it leaves the sketch is a solved position and carries a solved position's
    // precision. Nothing is put back, because nothing was broken.
    assert!(outcome.converged(), "{outcome:?}");
    assert_eq!(outcome.degrees_of_freedom(), 0, "{outcome:?}");
    // And refused without a step being taken, which is the difference between
    // asking whether there is anywhere to go and finding out by going. A run
    // that has to discover this creeps toward the cursor by less than a drag is
    // judged by, keeps step after step, and factorises the normal equations
    // once per step to arrive back where it started.
    assert_eq!(
        outcome.iterations(),
        0,
        "a drag with nowhere to go ran anyway: {outcome:?}"
    );

    // And now the half the fixture above cannot show. The same immovable point,
    // with an arm hung off it at a stated length — one freedom, free to swing.
    let mut sketch = Sketch::default();
    let anchor = sketch.add_point(DVec2::ZERO);
    sketch.fix(anchor);
    let pinned = sketch.add_point(DVec2::ZERO);
    let swinging = sketch.add_point(DVec2::new(3.0, 0.0));
    sketch.add_segment(pinned, swinging);
    sketch.add_constraint(Constraint::Coincident {
        a: anchor,
        b: pinned,
    });
    sketch.add_constraint(Constraint::apart(pinned, swinging, 3.0));
    solver.solve(&mut sketch, &mut outcome);
    assert!(outcome.converged(), "{outcome:?}");
    // The arm's swing, and it is the whole reason this fixture says anything the
    // one above does not.
    assert_eq!(outcome.degrees_of_freedom(), 1, "{outcome:?}");

    // A pointer sweeping a circle about the point it has hold of, which cannot
    // follow it anywhere. The arm must not so much as tremble — and swept rather
    // than asked once, because that is the shape the fault took: a pointer held
    // over a point that cannot move shook the drawing for as long as it was held
    // there.
    let settled = sketch.point(swinging).position;
    for step in 0..12 {
        let angle = step as f64 * 0.5;
        let to = DVec2::new(2.0 * angle.cos(), 2.0 * angle.sin());
        solver.drag(&mut sketch, &[Drive::Point(pinned, to)], &[], &mut outcome);
        assert!(
            sketch.point(pinned).position.length() < EPSILON,
            "the cursor at {to:?} moved a point welded to a fixed anchor"
        );
        assert!(
            (sketch.point(swinging).position - settled).length() < DRAGGED,
            "the cursor at {to:?} swung an arm nothing asked to move: {:?}",
            sketch.point(swinging).position
        );
        // Refused on what the drag *drives* rather than on what the sketch can
        // do: this one has a freedom and the point being driven is no part of
        // it, so there is still nowhere for the pull to go.
        assert_eq!(
            outcome.iterations(),
            0,
            "the cursor at {to:?} ran a solve to conclude nothing: {outcome:?}"
        );
    }
}

/// What a drag holds still decides the answer, not just what it drives.
///
/// The two arguments do different jobs and nothing else in this file tells them
/// apart, because the drags it makes have nothing worth holding. Here the same
/// drive is asked twice over the same sketch and answers differently, which is
/// the only way to say that holding is doing anything at all.
///
/// A circle through a fixed point: its centre must stand off that point by
/// exactly its radius. Growing it is therefore two things at once — the rim
/// moves out and the centre walks after it — and holding the centre is what
/// separates them. Which is precisely what a rim drag wants: growing a circle
/// should not carry it across the drawing.
#[test]
fn holding_the_centre_is_what_grows_a_circle_instead_of_walking_it() {
    let mut solver = Solver::default();
    let mut outcome = Outcome::default();

    // Holding nothing, the centre is free to travel and the constraint makes it:
    // a radius of five puts the centre five from the origin.
    let Pegged {
        mut sketch,
        centre,
        circle,
    } = Pegged::new();
    solver.solve(&mut sketch, &mut outcome);
    solver.drag(
        &mut sketch,
        &[Drive::Radius(circle, 5.0)],
        &[],
        &mut outcome,
    );
    assert!(
        (sketch.circle(circle).radius - 5.0).abs() < DRAGGED,
        "the rim would not be driven: {}",
        sketch.circle(circle).radius
    );
    let walked = sketch.point(centre).position;
    assert!(
        (walked - DVec2::new(5.0, 0.0)).length() < DRAGGED,
        "the centre did not follow the rim it is tied to: {walked:?}"
    );

    // Holding the centre, the same drive has nowhere to go: the standoff is
    // fixed at three, so the radius is too, and the sketch comes back untouched.
    let Pegged {
        mut sketch,
        centre,
        circle,
    } = Pegged::new();
    solver.solve(&mut sketch, &mut outcome);
    solver.drag(
        &mut sketch,
        &[Drive::Radius(circle, 5.0)],
        &[centre],
        &mut outcome,
    );
    assert_eq!(
        sketch.circle(circle).radius,
        3.0,
        "a held centre let the radius go anyway"
    );
    assert_eq!(sketch.point(centre).position, DVec2::new(3.0, 0.0));
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
    assert!(outcome.converged());
    assert_eq!(
        outcome.degrees_of_freedom(),
        1,
        "the point should be free along the edge and nowhere else"
    );

    // Two and a half units off the line, which is where a cursor always is.
    // The edge runs along y = 0, so the nearest place on it is straight below.
    let asked = DVec2::new(7.0, 2.5);
    solver.drag(
        &mut sketch,
        &[Drive::Point(sliding, asked)],
        &[],
        &mut outcome,
    );
    assert!(outcome.converged(), "{:?}", outcome);
    // And it ran to get there, which is the other side of the refusal next
    // door: a pull the constraints cannot take *exactly* is not a pull they
    // pin, and a drag turned away for reaching past what it can have would
    // leave every point on an edge unable to slide along it.
    assert!(
        outcome.iterations() > 0,
        "the drag was refused rather than answered: {outcome:?}"
    );
    // Held, the cursor asked for somewhere off the edge and the constraints
    // refused; freed, they answered with the nearest place on it. That second
    // attempt is the one kept, and it is the one this names.

    let landed = sketch.point(sliding).position;
    assert!(
        (landed - DVec2::new(7.0, 0.0)).length() < DRAGGED,
        "asked for {asked:?} and landed {landed:?}, not the foot of the perpendicular"
    );
    // And the edge it slid along did not come to meet it: both ends are pinned,
    // so a solve that had moved them would be answering a different question.
    assert_eq!(sketch.point(left).position, DVec2::ZERO);
    assert_eq!(sketch.point(right).position, DVec2::new(10.0, 0.0));
}
