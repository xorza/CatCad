//! What the app decides before anything is drawn.

use glam::DVec2;
use silverpoint::{SolveReport, Solver};

use crate::demo::Demo;
use crate::named::Named;
use crate::{CatCad, Status};

/// The demo is a fixture, so what it solves to is a fact the rest of the suite
/// leans on — the frames below all draw this rectangle — and the report has to
/// agree about what is determined and what is not.
#[test]
fn the_demo_sketch_solves_to_a_determined_rectangle_and_a_free_linkage() {
    let mut sketch = Demo::sketch();
    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    // Three: the linkage is two free points against one distance between
    // them, and it is there so the drawing has somewhere it can be dragged.
    // The rectangle itself is determined, which the corners below show.
    assert_eq!(report.degrees_of_freedom, 3, "{report:?}");
    assert_eq!(report.redundant_equations, 0, "{report:?}");

    // Only the rectangle and the circle's hub: the linkage's two points come
    // after these, and `zip` stops before reaching them.
    let corners: Vec<DVec2> = sketch.points().map(|(_, position)| position).collect();
    let expected = [
        DVec2::ZERO,
        DVec2::new(8.0, 0.0),
        DVec2::new(8.0, 5.0),
        DVec2::new(0.0, 5.0),
        // The circle's centre: mid-width, mid-height.
        DVec2::new(4.0, 2.5),
    ];
    for (found, want) in corners.iter().zip(expected) {
        assert!((*found - want).length() < 1e-9, "{found:?} vs {want:?}");
    }
    assert_eq!(sketch.circles().next().unwrap().1.radius, 1.5);
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
        "solved · 3 dof · 0 redundant · 4 iterations",
        "the demo opens with its linkage free and the rest determined"
    );
}
