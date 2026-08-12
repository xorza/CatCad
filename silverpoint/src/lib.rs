//! Geometry for CAD. Everything 2D lives under `sketch`: the [`Sketch`]
//! container, the [`Constraint`]s over it, and the [`Solver`] that satisfies
//! them. Three-dimensional work belongs beside it as a sibling module rather
//! than as an extension of it — the two share a crate, not a coordinate space.
//!
//! A [`Sketch`] holds points, segments, and circles together with the
//! [`Constraint`]s relating them. [`Solver`] moves the unfixed geometry until
//! every constraint is satisfied, and reports how well determined the result
//! was:
//!
//! ```
//! # use glam::DVec2;
//! # use silverpoint::{Constraint, Sketch, Solver};
//! let mut sketch = Sketch::default();
//! let origin = sketch.add_point(DVec2::ZERO);
//! let end = sketch.add_point(DVec2::new(1.0, 0.2));
//! sketch.fix(origin);
//! sketch.add_constraint(Constraint::Horizontal { a: origin, b: end });
//! sketch.add_constraint(Constraint::Distance { a: origin, b: end, distance: 5.0 });
//!
//! let report = Solver::default().solve(&mut sketch);
//! assert!(report.converged);
//! assert_eq!(report.degrees_of_freedom, 0);
//! assert!((sketch.point(end) - DVec2::new(5.0, 0.0)).length() < 1e-9);
//! ```
//!
//! Solving is Levenberg-Marquardt over the analytic Jacobian of the constraint
//! residuals, on dense matrices — sketches are small, and the parameter count
//! is twice the point count plus one per circle. Everything is `f64`: the
//! residuals of a nearly-degenerate sketch do not survive `f32`.

pub(crate) mod sketch;

pub use sketch::constraint::Constraint;
pub use sketch::solver::{SolveReport, Solver};
pub use sketch::{Circle, CircleId, PointId, Segment, SegmentId, Sketch};
