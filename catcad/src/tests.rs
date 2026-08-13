//! What the app decides before anything is drawn.

use glam::DVec2;
use silverpoint::Solver;

use crate::demo::Demo;

/// The demo is a fixture, so what it solves to is a fact the rest of the suite
/// leans on — the frames below all draw this rectangle — and the report has to
/// agree that nothing is left free.
#[test]
fn the_demo_sketch_solves_to_a_determined_rectangle() {
    let mut sketch = Demo::sketch();
    let report = Solver::default().solve(&mut sketch);

    assert!(report.converged, "{report:?}");
    assert_eq!(report.degrees_of_freedom, 0, "{report:?}");
    assert_eq!(report.redundant_equations, 0, "{report:?}");

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
