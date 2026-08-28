//! Geometry for CAD. Everything 2D lives under `sketch`: the [`Sketch`]
//! container, the [`Constraint`]s over it, and the [`Solver`] that satisfies
//! them. Three-dimensional work belongs beside it as a sibling module rather
//! than as an extension of it — the two share a crate, not a coordinate space.
//!
//! A [`Sketch`] holds points, segments, and circles together with the
//! [`Constraint`]s relating them, and [`Plane`] carries the whole of one into
//! the world it is modelled in. [`Solver`] moves the unfixed geometry until
//! every constraint is satisfied, reporting how the run went and filling in
//! what the sketch it settled on can still do:
//!
//! ```
//! # use glam::DVec2;
//! # use silverpoint::{Along, Constraint, Dimension, Outcome, Sketch, Solver};
//! let mut sketch = Sketch::default();
//! let origin = sketch.add_point(DVec2::ZERO);
//! let end = sketch.add_point(DVec2::new(1.0, 0.2));
//! sketch.fix(origin);
//! sketch.add_constraint(Constraint::Horizontal { a: origin, b: end });
//! sketch.add_constraint(Constraint::Distance {
//!     a: origin,
//!     b: end,
//!     along: Along::Shortest,
//!     dimension: Dimension::new(5.0),
//! });
//!
//! let mut outcome = Outcome::default();
//! Solver::default().solve(&mut sketch, &mut outcome);
//! assert!(outcome.converged());
//! assert_eq!(outcome.degrees_of_freedom(), 0);
//! assert!((sketch.point(end).position - DVec2::new(5.0, 0.0)).length() < 1e-9);
//! ```
//!
//! Solving is Levenberg-Marquardt over the analytic Jacobian of the constraint
//! residuals, on dense matrices — sketches are small, and the parameter count
//! is twice the point count plus one per circle. Everything is `f64`: the
//! residuals of a nearly-degenerate sketch do not survive `f32`.
pub(crate) mod arena;
pub(crate) mod inline;
pub(crate) mod loops;
pub(crate) mod math;
pub(crate) mod number;
pub(crate) mod sided;
pub(crate) mod sketch;
pub(crate) mod solid;

pub use arena::Id;
pub use math::plane::Plane;
pub use math::triangulate::Fill;
pub use sketch::arrangement::Arrangement;
pub use sketch::arrangement::bound::Bound;
pub use sketch::arrangement::face::Face;
pub use sketch::arrangement::filler::Filler;
pub use sketch::constraint::{Along, Constraint, ConstraintId, Dimension};
pub use sketch::entity::Entity;
pub use sketch::measurement::{Frame, Measurement};
pub use sketch::snapshot::Snapshot;
pub use sketch::solver::freedom::Freedom;
pub use sketch::solver::outcome::Outcome;
pub use sketch::solver::{Drive, Solver};
pub use sketch::{Circle, CircleId, Point, PointId, Removed, Segment, SegmentId, Sketch};
pub use solid::boolean::Boolean;
pub use solid::boolean::operation::Operation;
pub use solid::build::builder::{Builder, Extrusion};
pub use solid::build::revolving::{Revolution, Sector};
pub use solid::grown::Grown;
pub use solid::mesh::{Mesher, Patch};
pub use solid::named::{Named, Step};
pub use solid::topology::body::Body;
