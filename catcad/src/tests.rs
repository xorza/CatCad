//! What the app decides before anything is drawn.

use glam::DVec2;
use silverpoint::{PointId, SolveReport, Solver};

use crate::demo::Demo;
use crate::named::Named;
use crate::{CatCad, Status};

/// The demo is a fixture, so what it solves to is a fact the rest of the suite
/// leans on — the frames below all draw this drawing — and the report has to
/// agree about what is determined and what is not.
///
/// The rectangle and the hub are checked against hand-worked coordinates,
/// because their constraints admit one answer. The arm's are not: it is free to
/// travel, so where it settles is wherever the solve reached from the guess,
/// and what it owes is its *relations* — which is what the constraints actually
/// say and what a wrong Jacobian would break.
#[test]
fn the_demo_sketch_solves_to_a_rigid_frame_and_an_arm_that_can_move() {
    let mut sketch = Demo::sketch();
    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    // Eighteen free parameters — nine unpinned points and two radii — against
    // thirteen equations. Six of those determine the rectangle and two the hub,
    // leaving the five that make the drawing worth dragging: the arm's three
    // (it can travel and turn as one piece), the rail's one (it stretches), and
    // the unconstrained radius of the circle.
    assert_eq!(report.degrees_of_freedom, 5, "{report:?}");
    assert_eq!(report.redundant_equations, 0, "{report:?}");

    let at: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
    let expected = [
        DVec2::ZERO,
        DVec2::new(8.0, 0.0),
        DVec2::new(8.0, 5.0),
        DVec2::new(0.0, 5.0),
        // The circle's centre: mid-width, mid-height.
        DVec2::new(4.0, 2.5),
    ];
    for (found, want) in at.iter().zip(expected) {
        assert!((*found - want).length() < 1e-9, "{found:?} vs {want:?}");
    }
    // Nothing asked the circle for a size, so it kept the one it was made with
    // — which is what leaves its rim free to be dragged.
    assert_eq!(sketch.circles().next().unwrap().1.radius, 1.5);

    // The rail runs along the rectangle's base, and the arm holds its two
    // lengths at a right angle. Points 5..9 are rail end, shoulder, elbow,
    // wrist, in the order they were added.
    let [rail_end, shoulder, elbow, wrist] = [at[5], at[6], at[7], at[8]];
    assert!((shoulder.y - rail_end.y).abs() < 1e-9, "{rail_end:?}");
    assert!(((elbow - shoulder).length() - 2.0).abs() < 1e-9);
    assert!(((wrist - elbow).length() - 1.4).abs() < 1e-9);
    assert!((elbow - shoulder).dot(wrist - elbow).abs() < 1e-9);
    // And the eye at the end kept the size it was given, unlike the circle.
    assert_eq!(sketch.circles().nth(1).unwrap().1.radius, 0.45);

    // The freedoms are there to be used, so the last of this takes hold of
    // them. A count of five says only that the drawing has somewhere to go; a
    // constraint added carelessly could leave the count reading five and still
    // freeze the arm, and only a drag can tell the two apart.
    let id: Vec<PointId> = sketch.points().map(|(point, _)| point).collect();
    let sent = wrist + DVec2::new(1.2, -0.4);
    let report = Solver::default().edit_holding(&mut sketch, &[id[8]], |sketch| {
        sketch.set_point(id[8], sent)
    });
    assert!(report.converged, "{report:?}");

    let now: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
    assert_eq!(now[8], sent, "the arm would not go where it was sent");
    // It went as one rigid piece: both bars their own length, the elbow still
    // square, and the rail still along the base it is parallel to.
    assert!(((now[7] - now[6]).length() - 2.0).abs() < 1e-9, "{now:?}");
    assert!(((now[8] - now[7]).length() - 1.4).abs() < 1e-9, "{now:?}");
    assert!(
        (now[7] - now[6]).dot(now[8] - now[7]).abs() < 1e-9,
        "{now:?}"
    );
    assert!((now[6].y - now[5].y).abs() < 1e-9, "{now:?}");
    // And the frame it hangs beneath did not follow it anywhere.
    for (index, was) in at.iter().enumerate().take(5) {
        assert!((now[index] - *was).length() < 1e-9, "corner {index} moved");
    }

    // The circle is the other kind of freedom: no radius of its own, so its rim
    // keeps whatever a drag gives it instead of being pulled back.
    let hole = sketch.circles().next().unwrap().0;
    let report = Solver::default()
        .edit_holding(&mut sketch, &[id[4]], |sketch| sketch.set_radius(hole, 2.2));
    assert!(report.converged, "{report:?}");
    assert_eq!(
        sketch.circle(hole).radius,
        2.2,
        "the rim would not be driven"
    );
}

/// What the status line reads, in every shape it takes.
///
/// Pinned here rather than left to the visual suite: the line is drawn into
/// the golden frames, but it covers far under the one percent of pixels those
/// tolerate, so the whole of it can change without a golden noticing. Measured
/// — swapping a separator in it leaves all ten passing.
#[test]
fn the_status_line_reads_the_report_and_what_is_under_the_pointer() {
    let solved = SolveReport {
        converged: true,
        iterations: 4,
        max_residual: 0.0,
        degrees_of_freedom: 0,
        redundant_equations: 0,
    };
    assert_eq!(
        Status {
            report: solved,
            hovered: None,
        }
        .to_string(),
        "solved · 0 dof · 0 redundant · 4 iterations"
    );

    // A sketch entity under the pointer adds itself to the end, in the word a
    // draughtsman would use rather than the solver's.
    let sketch = Demo::sketch();
    let point = sketch.points().next().unwrap().0;
    let segment = sketch.segments().next().unwrap().0;
    let circle = sketch.circles().next().unwrap().0;
    for (hovered, tail) in [
        (Named::Point(point), " · point"),
        (Named::Segment(segment), " · edge"),
        (Named::Circle(circle), " · circle"),
    ] {
        assert_eq!(
            Status {
                report: solved,
                hovered: Some(hovered),
            }
            .to_string(),
            format!("solved · 0 dof · 0 redundant · 4 iterations{tail}")
        );
    }

    // An unsolved sketch says so, and says what it left free.
    let stuck = SolveReport {
        converged: false,
        iterations: 100,
        max_residual: 0.5,
        degrees_of_freedom: 3,
        redundant_equations: 2,
    };
    assert_eq!(
        Status {
            report: stuck,
            hovered: None,
        }
        .to_string(),
        "unsolved · 3 dof · 2 redundant · 100 iterations"
    );

    // And the app's own opening state agrees with the demo's solve.
    let app = CatCad::build();
    assert_eq!(
        app.status().to_string(),
        "solved · 5 dof · 0 redundant · 4 iterations",
        "the demo opens with its arm free and its frame determined"
    );
}
